//! Where rgram keeps its data, and the system language TDLib is told.

use std::env;
use std::path::PathBuf;

use model::Environment;

/// Overrides the data directory, for test profiles.
const DATA_DIR_VAR: &str = "RGRAM_DATA_DIR";
const APP_DIR: &str = "rgram";
/// Test servers keep separate accounts, so they get their own database.
const TEST_DIR: &str = "test-dc";

/// The per-user data directory: Application Support on macOS, `%APPDATA%`
/// on Windows, `$XDG_DATA_HOME` elsewhere.
pub(crate) fn data_dir(environment: Environment) -> anyhow::Result<PathBuf> {
    let base = match env::var_os(DATA_DIR_VAR).filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => platform_dir()?.join(APP_DIR),
    };
    Ok(match environment {
        Environment::Production => base,
        Environment::Test => base.join(TEST_DIR),
    })
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
    fn locales_become_ietf_tags() {
        assert_eq!(ietf_tag("de_DE.UTF-8").as_deref(), Some("de-DE"));
        assert_eq!(ietf_tag("sr_RS@latin").as_deref(), Some("sr-RS"));
        assert_eq!(ietf_tag("C"), None);
        assert_eq!(ietf_tag("POSIX"), None);
        assert_eq!(ietf_tag(""), None);
    }
}
