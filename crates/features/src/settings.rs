//! App-wide preferences that outlive a session (`Core::Settings`), kept as
//! JSON next to the TDLib database.

use std::path::{Path, PathBuf};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use style::Palette;
use ui::Theme;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Values {
    night_mode: bool,
    /// The archive row shrinks to a bar (`archiveCollapsed`).
    archive_collapsed: bool,
    /// The archive leaves the list for the main menu (`archiveInMainMenu`).
    archive_in_main_menu: bool,
}

#[derive(Debug, Default)]
pub struct Settings {
    /// `None` keeps changes in memory only, as in snapshots.
    path: Option<PathBuf>,
    values: Values,
}

impl Global for Settings {}

/// Reads the settings at `path` and applies them. A missing or unreadable
/// file gives the defaults.
pub fn load(path: PathBuf, cx: &mut App) {
    let values = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            tracing::warn!(%error, "settings are unreadable, using the defaults");
            Values::default()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Values::default(),
        Err(error) => {
            tracing::warn!(%error, "cannot read the settings, using the defaults");
            Values::default()
        }
    };
    apply_theme(values.night_mode, cx);
    cx.set_global(Settings {
        path: Some(path),
        values,
    });
}

pub(crate) fn is_night_mode(cx: &App) -> bool {
    values(cx).is_some_and(|values| values.night_mode)
}

/// Switches between the day and night palettes and remembers the choice.
pub(crate) fn set_night_mode(night_mode: bool, cx: &mut App) {
    change(cx, |values| values.night_mode = night_mode);
    apply_theme(night_mode, cx);
    cx.refresh_windows();
}

pub(crate) fn is_archive_collapsed(cx: &App) -> bool {
    values(cx).is_some_and(|values| values.archive_collapsed)
}

pub(crate) fn set_archive_collapsed(collapsed: bool, cx: &mut App) {
    change(cx, |values| values.archive_collapsed = collapsed);
    cx.refresh_windows();
}

pub(crate) fn is_archive_in_main_menu(cx: &App) -> bool {
    values(cx).is_some_and(|values| values.archive_in_main_menu)
}

pub(crate) fn set_archive_in_main_menu(in_menu: bool, cx: &mut App) {
    change(cx, |values| values.archive_in_main_menu = in_menu);
    cx.refresh_windows();
}

fn values(cx: &App) -> Option<&Values> {
    cx.try_global::<Settings>().map(|settings| &settings.values)
}

/// Applies `edit` and saves the result.
fn change(cx: &mut App, edit: impl FnOnce(&mut Values)) {
    if !cx.has_global::<Settings>() {
        cx.set_global(Settings::default());
    }
    let settings = cx.global_mut::<Settings>();
    edit(&mut settings.values);
    if let Some(path) = &settings.path
        && let Err(error) = save(path, &settings.values)
    {
        tracing::warn!(%error, "cannot save the settings");
    }
}

fn apply_theme(night_mode: bool, cx: &mut App) {
    let palette = if night_mode {
        Palette::NIGHT
    } else {
        Palette::DEFAULT
    };
    cx.set_global(Theme { palette });
}

/// Writes through a temporary file so a crash leaves the old settings.
fn save(path: &Path, values: &Values) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(values).map_err(std::io::Error::other)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, json)?;
    std::fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_reads_back_the_values() {
        let dir = std::env::temp_dir().join(format!("rgram-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temp dir is writable");
        let path = dir.join("settings.json");
        let values = Values {
            night_mode: true,
            archive_collapsed: true,
            archive_in_main_menu: false,
        };
        save(&path, &values).expect("the temp dir is writable");
        let read: Values =
            serde_json::from_slice(&std::fs::read(&path).expect("just written")).expect("valid");
        assert_eq!(read, values);
        // Unknown and missing keys fall back to the defaults.
        let partial: Values = serde_json::from_str(r#"{"future_key": 1}"#).expect("valid");
        assert_eq!(partial, Values::default());
        std::fs::remove_dir_all(&dir).ok();
    }
}
