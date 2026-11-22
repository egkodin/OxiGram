//! Stages replacements beside the running package so installation can rename
//! the application on the same volume after shutdown.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context as _, ensure};
use semver::Version;
use tempfile::TempDir;

use super::{archive, quiet_command, releases::Update};

const MARKER_MAGIC: &str = "OxiGram package 1";
const MAX_MARKER: u64 = 256_000;

#[derive(Clone, Copy)]
enum Layout {
    Mac,
    Linux,
    Windows,
}

impl Layout {
    fn current() -> Option<Self> {
        match std::env::consts::OS {
            "macos" => Some(Self::Mac),
            "linux" => Some(Self::Linux),
            "windows" if std::env::consts::ARCH == "x86_64" => Some(Self::Windows),
            _ => None,
        }
    }

    fn archive_root(self) -> &'static str {
        match self {
            Self::Mac => "OxiGram.app",
            Self::Linux | Self::Windows => "OxiGram",
        }
    }

    fn marker(self) -> &'static str {
        match self {
            Self::Mac => "Contents/Resources/OxiGram.package",
            Self::Linux | Self::Windows => "OxiGram.package",
        }
    }

    fn executable(self) -> &'static str {
        match self {
            Self::Mac => "Contents/MacOS/OxiGram",
            Self::Linux => "oxigram",
            Self::Windows => "oxigram.exe",
        }
    }

    fn package_root(self, exe: &Path) -> Option<&Path> {
        let root = exe
            .ancestors()
            .nth(if matches!(self, Self::Mac) { 3 } else { 1 })?;
        (exe == root.join(self.executable())).then_some(root)
    }
}

#[derive(Clone)]
pub(super) struct Package {
    root: PathBuf,
    layout: Layout,
    pub(super) version: Version,
}

impl Package {
    pub(super) fn current() -> Option<Self> {
        let layout = Layout::current()?;
        let exe = std::env::current_exe().ok()?;
        let root = layout.package_root(&exe)?;
        if root.parent().is_none() || (matches!(layout, Layout::Mac) && !root.is_dir()) {
            return None;
        }
        let version = Version::parse(env!("CARGO_PKG_VERSION")).ok()?;
        let marker = read_marker(layout, root).ok()?;
        (marker.lines().nth(1)? == version.to_string()).then_some(Self {
            root: root.to_owned(),
            layout,
            version,
        })
    }

    pub(super) fn suffix(&self) -> String {
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            arch => arch,
        };
        match self.layout {
            Layout::Mac => format!("macos-{arch}.zip"),
            Layout::Linux => format!("linux-{arch}.tar.gz"),
            Layout::Windows => format!("windows-{arch}.zip"),
        }
    }

    pub(super) fn prepare(
        &self,
        update: &Update,
        profile: Option<&Path>,
    ) -> anyhow::Result<Prepared> {
        validate(self.layout, &self.root, &self.version)?;
        if let Some(profile) = profile {
            let profile = profile
                .canonicalize()
                .unwrap_or_else(|_| profile.to_owned());
            ensure!(
                !profile.starts_with(self.root.canonicalize()?),
                "the account profile is inside the application folder"
            );
        }
        let parent = self
            .root
            .parent()
            .context("the installation has no parent directory")?;
        let temporary = tempfile::Builder::new()
            .prefix(".oxigram-update-")
            .tempdir_in(parent)
            .context("creating a staging folder beside OxiGram")?;
        let staging = temporary.path();
        let archive_path = staging.join("download");
        let compressed = update.download(&archive_path)?;
        archive::unpack(
            compressed,
            staging,
            self.layout.archive_root(),
            !matches!(self.layout, Layout::Linux),
        )?;
        fs::remove_file(archive_path)?;
        let incoming = staging.join(self.layout.archive_root());
        validate(self.layout, &incoming, &update.version)?;

        #[cfg(target_os = "macos")]
        ensure!(
            quiet_command("/usr/bin/codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(&incoming)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?
                .success(),
            "the downloaded app failed its code-signature check"
        );

        ensure!(
            quiet_command(incoming.join(self.layout.executable()))
                .current_dir(&incoming)
                .env_remove("OXIGRAM_API_ID")
                .env_remove("OXIGRAM_API_HASH")
                .env_remove("DYLD_LIBRARY_PATH")
                .env_remove("LD_LIBRARY_PATH")
                .arg("--check-release-configuration")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?
                .success(),
            "the downloaded app failed its launch check"
        );
        Ok(Prepared {
            temporary,
            incoming,
            package: self.clone(),
        })
    }
}

pub(super) struct Prepared {
    temporary: TempDir,
    incoming: PathBuf,
    package: Package,
}

impl Prepared {
    pub(super) fn launch(self) -> anyhow::Result<()> {
        let stage = self.temporary.path();
        let root = &self.package.root;
        let mut installer = installer_command();
        installer
            .current_dir(root.parent().context("the installation has no parent")?)
            .env("OXIGRAM_UPDATE_PID", std::process::id().to_string())
            .env("OXIGRAM_UPDATE_ROOT", root)
            .env("OXIGRAM_UPDATE_INCOMING", &self.incoming)
            .env("OXIGRAM_UPDATE_STAGE", stage)
            .env("OXIGRAM_UPDATE_MARKER", self.package.layout.marker())
            .env("OXIGRAM_UPDATE_FROM", self.package.version.to_string())
            .env("OXIGRAM_UPDATE_EXE", self.package.layout.executable())
            .env("OXIGRAM_UPDATE_WORKDIR", std::env::current_dir()?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("starting the installer")?;
        // The installer owns the stage once it has started.
        let _ = self.temporary.keep();
        Ok(())
    }
}

#[cfg(unix)]
fn installer_command() -> std::process::Command {
    let mut command = quiet_command("/bin/sh");
    command.args(["-c", include_str!("install.sh")]);
    command
}

#[cfg(windows)]
fn installer_command() -> std::process::Command {
    use base64::Engine as _;

    let text: Vec<u8> = include_str!("install.ps1")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut command = quiet_command("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand"])
        .arg(base64::engine::general_purpose::STANDARD.encode(text));
    command
}

fn read_marker(layout: Layout, root: &Path) -> anyhow::Result<String> {
    let marker = root.join(layout.marker());
    let md = fs::symlink_metadata(&marker)?;
    ensure!(
        md.is_file() && md.len() <= MAX_MARKER,
        "invalid package marker"
    );
    let text = fs::read_to_string(marker)?;
    ensure!(
        text.lines().next() == Some(MARKER_MAGIC),
        "unexpected package marker"
    );
    Ok(text)
}

fn validate(layout: Layout, root: &Path, expected: &Version) -> anyhow::Result<()> {
    ensure!(
        fs::symlink_metadata(root)?.is_dir(),
        "installation is not a real directory"
    );
    let marker = read_marker(layout, root)?;
    let mut lines = marker.lines().skip(1);
    let version = expected.to_string();
    ensure!(
        lines.next() == Some(version.as_str()),
        "unexpected packaged version"
    );
    ensure!(
        fs::symlink_metadata(root.join(layout.executable()))?.is_file(),
        "package executable is missing"
    );
    if !matches!(layout, Layout::Mac) {
        let expected_files: BTreeSet<&str> = lines.collect();
        ensure!(!expected_files.is_empty(), "package inventory is missing");
        let mut actual = BTreeSet::new();
        inventory(root, root, &mut actual)?;
        ensure!(
            actual.iter().map(String::as_str).eq(expected_files),
            "the portable folder contains modified or additional files"
        );
    }
    Ok(())
}

fn inventory(root: &Path, dir: &Path, paths: &mut BTreeSet<String>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        ensure!(
            file_type.is_file() || file_type.is_dir(),
            "unexpected package entry"
        );
        ensure!(
            paths.len() < archive::MAX_ENTRIES,
            "too many entries in the package"
        );
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .context("invalid local path")?;
        paths.insert(format!("./{}", relative.replace('\\', "/")));
        if file_type.is_dir() {
            inventory(root, &path, paths)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_portable_package_is_only_replaced_if_its_inventory_matches() {
        let temp = tempfile::tempdir().unwrap();
        let version = Version::new(1, 2, 3);
        fs::write(temp.path().join("oxigram"), "").unwrap();
        fs::write(
            temp.path().join("OxiGram.package"),
            format!("{MARKER_MAGIC}\n{version}\n./OxiGram.package\n./oxigram\n"),
        )
        .unwrap();
        assert!(validate(Layout::Linux, temp.path(), &version).is_ok());
        fs::write(temp.path().join("personal.txt"), "preserve me").unwrap();
        assert!(validate(Layout::Linux, temp.path(), &version).is_err());
        assert!(temp.path().join("personal.txt").is_file());
    }
}
