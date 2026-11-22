//! Where OxiGram keeps its data, and the system language TDLib is told.

use std::env;
use std::path::{Path, PathBuf};

use model::Environment;

/// Overrides the data directory, for test profiles.
const DATA_DIR_VAR: &str = "OXIGRAM_DATA_DIR";
const APP_DIR: &str = "OxiGram";
/// The directory before the rename to OxiGram, moved on first start.
const LEGACY_APP_DIR: &str = "rgram";
/// Test servers keep separate accounts, so they get their own database.
const TEST_DIR: &str = "test-dc";
/// An account's TDLib database, files and language packs, in its directory.
pub(crate) const DATABASE_DIR: &str = "tdata";
pub(crate) const FILES_DIR: &str = "files";
pub(crate) const LANGUAGE_PACKS: &str = "langpack.sqlite";

/// The per-user data directory: Application Support on macOS, `%APPDATA%`
/// on Windows, `$XDG_DATA_HOME` elsewhere.
pub(crate) fn data_dir(environment: Environment) -> anyhow::Result<PathBuf> {
    let base = match env::var_os(DATA_DIR_VAR).filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => {
            let platform = platform_dir()?;
            adopt_legacy_dir(&platform.join(LEGACY_APP_DIR), &platform.join(APP_DIR))?;
            platform.join(APP_DIR)
        }
    };
    Ok(match environment {
        Environment::Production => base,
        Environment::Test => base.join(TEST_DIR),
    })
}

/// Moves a legacy profile once, when the current profile does not exist.
fn adopt_legacy_dir(legacy: &Path, current: &Path) -> anyhow::Result<()> {
    if current.exists() || !legacy.is_dir() {
        return Ok(());
    }
    std::fs::rename(legacy, current).map_err(|error| {
        anyhow::anyhow!(
            "moving {} to {}: {error}",
            legacy.display(),
            current.display()
        )
    })?;
    tracing::info!("moved the data directory to the OxiGram name");
    Ok(())
}

/// Account `id`'s directory: the data directory itself for the first one,
/// which had it before oxigram had accounts, `account-<id>` in it for others.
pub(crate) fn account_dir(data: &Path, id: u32) -> PathBuf {
    match id {
        0 => data.to_path_buf(),
        id => data.join(format!("account-{id}")),
    }
}

#[cfg(target_os = "macos")]
fn platform_dir() -> anyhow::Result<PathBuf> {
    Ok(home()?.join("Library").join("Application Support"))
}

#[cfg(target_os = "windows")]
fn platform_dir() -> anyhow::Result<PathBuf> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("APPDATA is not set"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_dir() -> anyhow::Result<PathBuf> {
    match env::var_os("XDG_DATA_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => Ok(home()?.join(".local").join("share")),
    }
}

#[cfg(not(target_os = "windows"))]
fn home() -> anyhow::Result<PathBuf> {
    env::var_os("HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set"))
}

/// The system language as an IETF tag (`en-US`), from the POSIX locale.
// shortcut: reads LC_ALL/LC_MESSAGES/LANG only, so apps started from the macOS Finder or on Windows fall back to "en"; read the OS locale API in M7 OS integration.
pub(crate) fn system_language() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|var| env::var(var).ok())
        .find_map(|locale| ietf_tag(&locale))
        .unwrap_or_else(|| "en".to_owned())
}

/// `de_DE.UTF-8` -> `de-DE`; `C` and `POSIX` are no language.
fn ietf_tag(locale: &str) -> Option<String> {
    let tag = locale.split(['.', '@']).next()?.replace('_', "-");
    let is_language = tag.len() >= 2 && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    (is_language && tag != "POSIX").then_some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_account_keeps_the_data_directory() {
        let data = Path::new("/data");
        assert_eq!(account_dir(data, 0), PathBuf::from("/data"));
        assert_eq!(account_dir(data, 2), PathBuf::from("/data/account-2"));
    }

    #[test]
    fn the_legacy_directory_moves_once_and_never_over_data() {
        let root = std::env::temp_dir().join(format!("oxigram-paths-{}", std::process::id()));
        let (legacy, current) = (root.join(LEGACY_APP_DIR), root.join("OxiGram"));
        std::fs::create_dir_all(legacy.join("tdata")).expect("temp dir is writable");
        adopt_legacy_dir(&legacy, &current).expect("moving within one temp dir works");
        assert!(current.join("tdata").is_dir() && !legacy.exists());
        std::fs::create_dir_all(&legacy).expect("temp dir is writable");
        adopt_legacy_dir(&legacy, &current).expect("an existing directory is kept");
        assert!(legacy.is_dir() && current.join("tdata").is_dir());
        std::fs::remove_dir_all(&root).expect("temp dir is removable");
    }

    #[test]
    fn locales_become_ietf_tags() {
        assert_eq!(ietf_tag("de_DE.UTF-8").as_deref(), Some("de-DE"));
        assert_eq!(ietf_tag("sr_RS@latin").as_deref(), Some("sr-RS"));
        assert_eq!(ietf_tag("C"), None);
        assert_eq!(ietf_tag("POSIX"), None);
        assert_eq!(ietf_tag(""), None);
    }
}
