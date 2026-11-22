//! Streams verified release archives into a private staging directory.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, ensure};
use flate2::read::GzDecoder;

pub(super) const MAX_ENTRIES: usize = 4096;
const MAX_EXPANDED: u64 = 400_000_000;

pub(super) fn unpack(
    archive: File,
    staging: &Path,
    root: &str,
    is_zip: bool,
) -> anyhow::Result<()> {
    let mut remaining = MAX_EXPANDED;
    if is_zip {
        let mut zip = zip::ZipArchive::new(archive).context("reading release ZIP")?;
        ensure!(zip.len() <= MAX_ENTRIES, "too many release entries");
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index)?;
            let path = entry.enclosed_name().context("unsafe ZIP path")?;
            let mode = entry.unix_mode().unwrap_or(0);
            ensure!(
                matches!(mode & 0o170_000, 0 | 0o040_000 | 0o100_000),
                "unsupported ZIP entry type"
            );
            let output = safe_path(staging, root, &path)?;
            if entry.is_dir() {
                fs::create_dir_all(output)?;
            } else {
                write_file(&output, &mut entry, mode, &mut remaining)?;
            }
        }
    } else {
        let mut tar = tar::Archive::new(GzDecoder::new(archive));
        for (index, entry) in tar.entries()?.enumerate() {
            ensure!(index < MAX_ENTRIES, "too many release entries");
            let mut entry = entry?;
            let output = safe_path(staging, root, &entry.path()?)?;
            let kind = entry.header().entry_type();
            if kind.is_dir() {
                fs::create_dir_all(output)?;
            } else {
                ensure!(kind.is_file(), "unsupported TAR entry type");
                let mode = entry.header().mode()?;
                write_file(&output, &mut entry, mode, &mut remaining)?;
            }
        }
    }
    Ok(())
}

fn safe_path(base: &Path, root: &str, name: &Path) -> anyhow::Result<PathBuf> {
    let mut parts = name.components();
    ensure!(
        parts.next() == Some(Component::Normal(root.as_ref())),
        "unexpected release root"
    );
    let mut output = base.join(root);
    for part in parts {
        let Component::Normal(part) = part else {
            anyhow::bail!("invalid release path component");
        };
        let text = part.to_str().context("non-UTF-8 release path")?;
        ensure!(
            !text.ends_with('.')
                && text
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
            "invalid release filename"
        );
        #[cfg(windows)]
        {
            let stem = text.split('.').next().unwrap_or("").to_ascii_uppercase();
            let reserved = matches!(stem.as_str(), "CON" | "NUL" | "AUX" | "PRN")
                || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.len() == 4
                    && stem.as_bytes()[3].is_ascii_digit());
            ensure!(!reserved, "reserved Windows filename");
        }
        output.push(part);
    }
    Ok(output)
}

fn write_file(
    output: &Path,
    mut input: impl io::Read,
    mode: u32,
    remaining: &mut u64,
) -> anyhow::Result<()> {
    fs::create_dir_all(output.parent().context("missing release directory")?)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let count = io::copy(&mut input.by_ref().take(*remaining + 1), &mut file)?;
    ensure!(
        count <= *remaining,
        "release exceeded its unpacked size limit"
    );
    *remaining -= count;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let permissions = if mode & 0o111 == 0 { 0o644 } else { 0o755 };
        file.set_permissions(fs::Permissions::from_mode(permissions))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricts_extraction_to_the_managed_root() {
        let base = Path::new("staged");
        let bad = [
            "../OxiGram/client",
            "OxiGram/../../client",
            "/OxiGram/client",
            "Different/client",
            "OxiGram/evil:stream",
            "OxiGram/evil\\client",
        ];
        for path in bad {
            if path.ends_with("evil\\client") && cfg!(windows) {
                // Windows treats backslashes as path separators. Both parts
                // remain normal path components inside the managed root.
                continue;
            }
            assert!(
                safe_path(base, "OxiGram", Path::new(path)).is_err(),
                "{path}"
            );
        }
        assert_eq!(
            safe_path(base, "OxiGram", Path::new("OxiGram/licenses/curl.txt")).unwrap(),
            base.join("OxiGram").join("licenses").join("curl.txt")
        );
    }

    #[test]
    fn extracts_the_same_file_from_deflated_zip_and_tgz() {
        use std::io::Write as _;

        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        let output = temp.path().join("output");
        let name = "OxiGram/oxigram";
        let bytes = b"offline executable";
        let mut zip = zip::ZipWriter::new(File::create(&input).unwrap());
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o755),
        )
        .unwrap();
        zip.write_all(bytes).unwrap();
        zip.finish().unwrap();
        unpack(File::open(&input).unwrap(), &output, "OxiGram", true).unwrap();
        assert_eq!(fs::read(output.join(name)).unwrap(), bytes);
        fs::remove_dir_all(&output).unwrap();

        let gzip = flate2::write::GzEncoder::new(
            File::create(&input).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(gzip);
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o755);
        header.set_size(u64::try_from(bytes.len()).unwrap());
        header.set_cksum();
        tar.append_data(&mut header, name, &bytes[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        unpack(File::open(&input).unwrap(), &output, "OxiGram", false).unwrap();
        assert_eq!(fs::read(output.join(name)).unwrap(), bytes);
    }

    #[test]
    fn refuses_duplicate_entries_and_bounds_expanded_size() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("client");
        let mut remaining = 5;
        write_file(&file, &b"abc"[..], 0o755, &mut remaining).unwrap();
        assert_eq!(remaining, 2);
        assert!(write_file(&file, &b"x"[..], 0o644, &mut remaining).is_err());
        assert!(
            write_file(
                &temp.path().join("too-big"),
                &b"abc"[..],
                0o644,
                &mut remaining
            )
            .is_err()
        );
        assert_eq!(fs::read(file).unwrap(), b"abc");
    }
}
