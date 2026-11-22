//! Selects a compatible release from OxiGram's public GitHub feed.

use std::fs::File;
use std::io::{self, Read as _, Seek as _};
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context as _, ensure};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

const ORIGIN: &str = "https://github.com/egkodin/OxiGram/releases";
const RELEASES: &str = "https://api.github.com/repos/egkodin/OxiGram/releases?per_page=12";
const MAX_FEED: u64 = 512_000;
const MAX_ARCHIVE: u64 = 200_000_000;
const USER_AGENT: &str = concat!("OxiGram/", env!("CARGO_PKG_VERSION"));

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

pub(super) struct Update {
    pub(super) version: Version,
    pub(super) page: String,
    url: String,
    size: u64,
    sha256: [u8; 32],
}

pub(super) fn check(suffix: &str, current: &Version) -> anyhow::Result<Option<Update>> {
    let feed = tempfile::NamedTempFile::new()?;
    download(RELEASES, feed.path(), MAX_FEED, 15)?;
    let mut bytes = Vec::new();
    File::open(feed.path())?
        .take(MAX_FEED + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len())? <= MAX_FEED,
        "release feed exceeded its size limit"
    );
    let releases = serde_json::from_slice(&bytes).context("reading the GitHub release feed")?;
    Ok(select(releases, suffix, current))
}

fn select(releases: Vec<Release>, suffix: &str, current: &Version) -> Option<Update> {
    releases
        .into_iter()
        .filter(|release| !release.draft && (!release.prerelease || !current.pre.is_empty()))
        .filter_map(|release| {
            let version = Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
            if version.cmp_precedence(current).is_le()
                || (current.pre.is_empty() && !version.pre.is_empty())
            {
                return None;
            }
            let filename = format!("OxiGram-{version}-{suffix}");
            let asset = release
                .assets
                .into_iter()
                .find(|asset| asset.name == filename)?;
            let url = format!("{ORIGIN}/download/{}/{filename}", release.tag_name);
            if asset.browser_download_url != url || !(1..=MAX_ARCHIVE).contains(&asset.size) {
                return None;
            }
            Some(Update {
                version,
                page: format!("{ORIGIN}/tag/{}", release.tag_name),
                url,
                size: asset.size,
                sha256: digest(asset.digest?.strip_prefix("sha256:")?)?,
            })
        })
        .max_by(|a, b| a.version.cmp_precedence(&b.version))
}

impl Update {
    pub(super) fn download(&self, path: &Path) -> anyhow::Result<File> {
        download(&self.url, path, self.size, 300)?;
        let mut file = File::open(path)?;
        ensure!(
            file.metadata()?.len() == self.size,
            "incomplete update download"
        );
        let mut hash = Sha256::new();
        io::copy(&mut file, &mut hash)?;
        ensure!(
            hash.finalize()[..] == self.sha256,
            "update checksum mismatch"
        );
        file.rewind()?;
        Ok(file)
    }
}

fn digest(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * index..2 * index + 2], 16).ok()?;
    }
    Some(bytes)
}

fn download(url: &str, path: &Path, limit: u64, seconds: u64) -> anyhow::Result<()> {
    let mut curl = if cfg!(windows) {
        super::quiet_command("curl.exe")
    } else {
        super::quiet_command("curl")
    };
    let status = curl
        .args([
            "-q",
            "--fail",
            "--silent",
            "--location",
            "--max-redirs",
            "4",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "8",
            "--user-agent",
            USER_AGENT,
            "--max-filesize",
        ])
        .arg(limit.to_string())
        .arg("--max-time")
        .arg(seconds.to_string())
        .arg("--output")
        .arg(path)
        .args(["--", url])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .context("starting the system HTTPS client")?;
    ensure!(status.success(), "the HTTPS transfer failed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUFFIX: &str = "linux-x86_64.tar.gz";

    fn release(tag: &str, draft: bool, prerelease: bool) -> Release {
        let version = tag.strip_prefix('v').expect("test tag");
        let name = format!("OxiGram-{version}-{SUFFIX}");
        Release {
            tag_name: tag.to_owned(),
            draft,
            prerelease,
            assets: vec![Asset {
                browser_download_url: format!("{ORIGIN}/download/{tag}/{name}"),
                name,
                size: 128,
                digest: Some(format!("sha256:{}", "ab".repeat(32))),
            }],
        }
    }

    #[test]
    fn finds_the_latest_compatible_stable_release() {
        let mut wrong_url = release("v1.9.0", false, false);
        wrong_url.assets[0].browser_download_url = "https://example.org/archive".into();
        let mut no_digest = release("v1.8.0", false, false);
        no_digest.assets[0].digest = None;
        let releases = vec![
            release("v2.0.0", true, false),
            release("v1.6.0-preview.1", false, true),
            wrong_url,
            no_digest,
            release("v1.2.0", false, false),
            release("v1.3.0", false, false),
            release("v0.9.0", false, false),
        ];
        let found = select(releases, SUFFIX, &Version::new(1, 0, 0)).expect("update found");
        assert_eq!(found.version, Version::new(1, 3, 0));
    }

    #[test]
    fn previews_can_update_to_newer_previews_and_stable_builds() {
        let current = Version::parse("1.0.0-preview.1").expect("valid version");
        let versions = [release("v1.0.0-preview.2", false, true)];
        assert!(select(Vec::from(versions), SUFFIX, &current).is_some());
        assert!(select(vec![release("v1.0.0", false, false)], SUFFIX, &current).is_some());
        assert!(select(vec![release("v1.0.0", false, false)], "other", &current).is_none());
    }

    #[test]
    fn compares_precedence_without_build_metadata_and_rejects_invalid_hashes() {
        let current = Version::new(1, 0, 0);
        assert!(select(vec![release("v1.0.0+test", false, false)], SUFFIX, &current).is_none());
        assert!(digest(&"gg".repeat(32)).is_none());
    }
}
