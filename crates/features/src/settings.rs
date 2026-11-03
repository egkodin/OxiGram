//! App-wide preferences that outlive a session (`Core::Settings`), kept as
//! JSON next to the TDLib database.

use std::path::{Path, PathBuf};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use style::Palette;
use ui::Theme;

/// Where the folders show (`chatFiltersHorizontal`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoldersPlacement {
    /// A column left of the chat list, when the window is wide enough.
    #[default]
    Sidebar,
    /// Tabs above the chat list.
    Top,
}

/// Which Enter sends a message (`sendSubmitWay`); the other one starts a
/// new line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SendBy {
    #[default]
    Enter,
    CtrlEnter,
}

impl SendBy {
    #[must_use]
    pub fn submit_key(self) -> ui::SubmitKey {
        match self {
            Self::Enter => ui::SubmitKey::Enter,
            Self::CtrlEnter => ui::SubmitKey::CtrlEnter,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Values {
    night_mode: bool,
    /// The archive row shrinks to a bar (`archiveCollapsed`).
    archive_collapsed: bool,
    /// The archive leaves the list for the main menu (`archiveInMainMenu`).
    archive_in_main_menu: bool,
    /// The chat list's share of the window; zero is the narrow list
    /// (`dialogsWidthRatio`).
    // shortcut: one ratio; tdesktop keeps another for when no chat is open, which matters once chats open in M3.
    dialogs_width_ratio: f32,
    folders_placement: FoldersPlacement,
    /// Hashtags searched for, most used first (`cRecentSearchHashtags`).
    // shortcut: app-wide; tdesktop keeps them per account, which matters once accounts switch in M8.
    recent_search_hashtags: crate::hashtags::Recent,
    send_by: SendBy,
    /// Emoji sent from the panel, most used first (`recentEmoji`).
    recent_emoji: crate::ranked::Ranked,
    /// The accounts' data directories in the menu's order; empty is the
    /// first account alone (`Main::Domain`'s list).
    accounts: Vec<u32>,
    /// The account shown at start (`activeAccount`).
    active_account: u32,
    /// The main menu's account list is open (`mainMenuAccountsShown`).
    main_menu_accounts_shown: bool,
}

impl Default for Values {
    fn default() -> Self {
        Self {
            night_mode: false,
            archive_collapsed: false,
            archive_in_main_menu: false,
            dialogs_width_ratio: crate::columns::DEFAULT_RATIO,
            folders_placement: FoldersPlacement::Sidebar,
            recent_search_hashtags: Vec::new(),
            send_by: SendBy::Enter,
            recent_emoji: Vec::new(),
            accounts: Vec::new(),
            active_account: 0,
            main_menu_accounts_shown: false,
        }
    }
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

#[must_use]
pub fn folders_placement(cx: &App) -> FoldersPlacement {
    values(cx).map_or_else(FoldersPlacement::default, |values| values.folders_placement)
}

// shortcut: no switch in the UI until the folders settings page (M6); snapshots set it.
pub fn set_folders_placement(placement: FoldersPlacement, cx: &mut App) {
    change(cx, |values| values.folders_placement = placement);
    cx.refresh_windows();
}

#[must_use]
pub fn send_by(cx: &App) -> SendBy {
    values(cx).map_or_else(SendBy::default, |values| values.send_by)
}

/// Open fields follow it (they observe [`Settings`]).
pub fn set_send_by(send_by: SendBy, cx: &mut App) {
    change(cx, |values| values.send_by = send_by);
}

pub(crate) fn recent_search_hashtags(cx: &App) -> crate::hashtags::Recent {
    values(cx).map_or_else(Vec::new, |values| values.recent_search_hashtags.clone())
}

/// Counts a use of each hashtag in `text` (`saveRecentSearchHashtags`).
pub(crate) fn save_search_hashtags(text: &str, cx: &mut App) {
    let tags = crate::hashtags::tags_in(text);
    if tags.is_empty() {
        return;
    }
    change(cx, |values| {
        for tag in &tags {
            crate::hashtags::increment(&mut values.recent_search_hashtags, tag);
        }
    });
}

pub(crate) fn remove_search_hashtag(tag: &str, cx: &mut App) {
    change(cx, |values| {
        values
            .recent_search_hashtags
            .retain(|(known, _)| known != tag);
    });
}

pub(crate) fn recent_emoji(cx: &App) -> crate::ranked::Ranked {
    values(cx).map_or_else(Vec::new, |values| values.recent_emoji.clone())
}

/// Counts a use of `emoji` (`incrementRecentEmoji`).
pub(crate) fn count_emoji_use(emoji: &str, limits: crate::ranked::Limits, cx: &mut App) {
    change(cx, |values| {
        crate::ranked::increment(&mut values.recent_emoji, emoji, limits);
    });
}

pub(crate) fn dialogs_width_ratio(cx: &App) -> f32 {
    values(cx).map_or(crate::columns::DEFAULT_RATIO, |values| {
        values.dialogs_width_ratio
    })
}

/// Follows a column drag; `save` writes it once the drag ends.
pub(crate) fn set_dialogs_width_ratio(ratio: f32, cx: &mut App) {
    if !cx.has_global::<Settings>() {
        cx.set_global(Settings::default());
    }
    cx.global_mut::<Settings>().values.dialogs_width_ratio = ratio;
}

/// The accounts in the menu's order and the active one, by data directory.
#[must_use]
pub fn accounts(cx: &App) -> (Vec<u32>, u32) {
    let (list, active) = values(cx).map_or((Vec::new(), 0), |values| {
        (values.accounts.clone(), values.active_account)
    });
    known_accounts(list, active)
}

/// The first account alone when none are listed, and a listed active one.
fn known_accounts(mut list: Vec<u32>, active: u32) -> (Vec<u32>, u32) {
    if list.is_empty() {
        list.push(0);
    }
    let active = if list.contains(&active) {
        active
    } else {
        list[0]
    };
    (list, active)
}

pub(crate) fn set_accounts(list: Vec<u32>, active: u32, cx: &mut App) {
    change(cx, |values| {
        values.accounts = list;
        values.active_account = active;
    });
}

pub(crate) fn is_main_menu_accounts_shown(cx: &App) -> bool {
    values(cx).is_some_and(|values| values.main_menu_accounts_shown)
}

pub(crate) fn set_main_menu_accounts_shown(shown: bool, cx: &mut App) {
    change(cx, |values| values.main_menu_accounts_shown = shown);
}

/// Writes the settings as they are now.
pub(crate) fn save_now(cx: &mut App) {
    change(cx, |_| {});
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
    fn accounts_fall_back_to_the_first_one() {
        assert_eq!(known_accounts(Vec::new(), 3), (vec![0], 0));
        assert_eq!(known_accounts(vec![0, 2], 2), (vec![0, 2], 2));
        assert_eq!(known_accounts(vec![1, 2], 0), (vec![1, 2], 1));
    }

    #[test]
    fn saves_and_reads_back_the_values() {
        let dir = std::env::temp_dir().join(format!("rgram-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temp dir is writable");
        let path = dir.join("settings.json");
        let values = Values {
            night_mode: true,
            archive_collapsed: true,
            archive_in_main_menu: false,
            dialogs_width_ratio: 0.0,
            folders_placement: FoldersPlacement::Top,
            recent_search_hashtags: vec![("rust".to_owned(), 2)],
            send_by: SendBy::CtrlEnter,
            recent_emoji: vec![("😂".to_owned(), 3)],
            accounts: vec![0, 2],
            active_account: 2,
            main_menu_accounts_shown: true,
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
