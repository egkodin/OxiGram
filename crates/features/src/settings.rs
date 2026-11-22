//! App-wide preferences that outlive a session (`Core::Settings`), kept as
//! JSON next to the TDLib database.

use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use gpui::{App, Global};
use media::devices::Direction;
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

/// The built-in themes (`Window::Theme::EmbeddedType`); the night mode
/// switch moves between the last day and the last night one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddedTheme {
    #[default]
    Classic,
    Day,
    /// `night.tdesktop-theme`.
    Tinted,
    /// `night-green.tdesktop-theme`.
    Night,
}

impl EmbeddedTheme {
    #[must_use]
    pub fn palette(self) -> Palette {
        match self {
            Self::Classic => Palette::DEFAULT,
            Self::Day => Palette::DAY_BLUE,
            Self::Tinted => Palette::NIGHT,
            Self::Night => Palette::NIGHT_GREEN,
        }
    }

    #[must_use]
    pub fn is_night(self) -> bool {
        matches!(self, Self::Tinted | Self::Night)
    }
}

/// What a desktop notification shows (`Core::Settings::NotifyView`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyView {
    #[default]
    NameAndPreview,
    NameOnly,
    Nothing,
}

/// The app's own notification switches (`Core::Settings`), next to the
/// ones Telegram keeps per kind of chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notify {
    /// Desktop notifications (`desktopNotify`).
    pub desktop: bool,
    /// The Dock bounces (`flashBounceNotify`).
    pub flash_bounce: bool,
    /// Sounds play (`soundNotify`).
    pub sound: bool,
    pub view: NotifyView,
    /// Muted chats count in the badge (`includeMutedCounter`).
    pub include_muted: bool,
    /// The badge counts messages, not chats (`countUnreadMessages`).
    pub count_messages: bool,
    /// New pinned messages notify (`notifyAboutPinned`).
    pub about_pinned: bool,
}

impl Default for Notify {
    fn default() -> Self {
        Self {
            desktop: true,
            flash_bounce: true,
            sound: true,
            view: NotifyView::default(),
            include_muted: true,
            count_messages: true,
            about_pinned: true,
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
    /// Settings search results opened, newest first
    /// (`Settings::RecentSearches`).
    // shortcut: app-wide; tdesktop keeps them per account, which matters once several accounts search settings.
    recent_settings_searches: Vec<String>,
    notify: Notify,
    /// The name this device shows in the sessions list; empty is the
    /// system's (`customDeviceModel`).
    custom_device_model: String,
    /// The theme night mode off shows (`Classic` or `Day`).
    day_theme: EmbeddedTheme,
    /// The theme night mode on shows (`Tinted` or `Night`).
    night_theme: EmbeddedTheme,
    /// Night mode follows the system (`systemDarkModeEnabled`).
    system_night_mode: bool,
    /// One to three emoji alone show big (`largeEmoji`).
    large_emoji: bool,
    /// Scrolling past a channel's end offers the next unread one
    /// (`pullToNextChannel`).
    pull_to_next_channel: bool,
    /// The speakers' cpal id; empty is the system's (`playbackDeviceId`).
    playback_device: String,
    /// The microphone's cpal id; empty is the system's (`captureDeviceId`).
    capture_device: String,
    /// The animations turned off, in tdesktop's bits (`powerSaving`).
    power_saving: u32,
    /// The interface scale in percent; 0 is the default (`kScaleAuto`).
    interface_scale: u32,
    /// Voices and rounds (`voicePlaybackSpeed`).
    voice_speed: crate::speed::Speed,
    /// Songs (`audioPlaybackSpeed`).
    audio_speed: crate::speed::Speed,
    song_repeat: crate::player::Repeat,
    song_order: crate::player::Order,
    song_volume: u8,
    unmuted_song_volume: u8,
    /// Videos in the viewer (`videoPlaybackSpeed`).
    video_speed: crate::speed::Speed,
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
            recent_settings_searches: Vec::new(),
            notify: Notify::default(),
            custom_device_model: String::new(),
            day_theme: EmbeddedTheme::Classic,
            night_theme: EmbeddedTheme::Tinted,
            system_night_mode: true,
            large_emoji: true,
            pull_to_next_channel: true,
            playback_device: String::new(),
            capture_device: String::new(),
            power_saving: 0,
            interface_scale: 0,
            voice_speed: crate::speed::Speed::default(),
            audio_speed: crate::speed::Speed::default(),
            song_repeat: crate::player::Repeat::default(),
            song_order: crate::player::Order::default(),
            song_volume: 100,
            unmuted_song_volume: 100,
            video_speed: crate::speed::Speed::default(),
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

const MAX_SETTINGS_BYTES: u64 = 4 << 20;

fn check_size(bytes: &[u8]) -> io::Result<()> {
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "settings exceed 4 MiB",
        ));
    }
    Ok(())
}

fn read_values(path: &Path) -> io::Result<Values> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)?;
    check_size(&bytes)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid settings at line {}, column {}",
                error.line(),
                error.column()
            ),
        )
    })
}

/// Reads the settings at `path` and applies them. A missing or unreadable
/// file gives the defaults.
pub fn load(path: PathBuf, cx: &mut App) {
    let (values, path) = match read_values(&path) {
        Ok(values) => (values, Some(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (Values::default(), Some(path)),
        Err(error) => {
            tracing::warn!(%error, "cannot read settings; using defaults in memory to preserve the file");
            (Values::default(), None)
        }
    };
    apply_theme(&values, cx);
    apply_devices(&values);
    crate::power_saving::apply(crate::power_saving::Saving(values.power_saving));
    cx.set_global(Settings { path, values });
}

pub(crate) fn is_night_mode(cx: &App) -> bool {
    values(cx).is_some_and(|values| values.night_mode)
}

/// Switches between the day and night themes and remembers the choice.
pub(crate) fn set_night_mode(night_mode: bool, cx: &mut App) {
    change(cx, |values| values.night_mode = night_mode);
    reapply_theme(cx);
}

/// The theme shown now.
pub(crate) fn theme(cx: &App) -> EmbeddedTheme {
    values(cx).map_or_else(EmbeddedTheme::default, |values| {
        if values.night_mode {
            values.night_theme
        } else {
            values.day_theme
        }
    })
}

/// Shows `theme`, turning night mode on or off with it.
pub(crate) fn set_theme(theme: EmbeddedTheme, cx: &mut App) {
    change(cx, |values| {
        values.night_mode = theme.is_night();
        if theme.is_night() {
            values.night_theme = theme;
        } else {
            values.day_theme = theme;
        }
    });
    reapply_theme(cx);
}

fn reapply_theme(cx: &mut App) {
    if let Some(values) = values(cx).cloned() {
        apply_theme(&values, cx);
    }
    cx.refresh_windows();
}

pub(crate) fn is_system_night_mode(cx: &App) -> bool {
    values(cx).is_some_and(|values| values.system_night_mode)
}

/// Following the system applies its appearance at once; snapshots turn it
/// off to stay the same on any Mac.
pub fn set_system_night_mode(enabled: bool, cx: &mut App) {
    change(cx, |values| values.system_night_mode = enabled);
    if enabled {
        follow_system_appearance(cx);
    }
}

/// Night mode as the system's appearance says, when it is followed.
pub(crate) fn follow_system_appearance(cx: &mut App) {
    if !is_system_night_mode(cx) {
        return;
    }
    let is_dark = matches!(
        cx.window_appearance(),
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
    );
    if is_dark != is_night_mode(cx) {
        set_night_mode(is_dark, cx);
    }
}

pub(crate) fn speed(kind: crate::speed::Kind, cx: &App) -> crate::speed::Speed {
    values(cx).map_or_else(crate::speed::Speed::default, |values| match kind {
        crate::speed::Kind::Voice => values.voice_speed,
        crate::speed::Kind::Audio => values.audio_speed,
        crate::speed::Kind::Video => values.video_speed,
    })
}

pub(crate) fn set_speed(kind: crate::speed::Kind, speed: crate::speed::Speed, cx: &mut App) {
    change(cx, |values| match kind {
        crate::speed::Kind::Voice => values.voice_speed = speed,
        crate::speed::Kind::Audio => values.audio_speed = speed,
        crate::speed::Kind::Video => values.video_speed = speed,
    });
}

pub(crate) fn is_large_emoji(cx: &App) -> bool {
    values(cx).is_none_or(|values| values.large_emoji)
}

pub(crate) fn song_repeat(cx: &App) -> crate::player::Repeat {
    values(cx).map_or_else(crate::player::Repeat::default, |values| values.song_repeat)
}

pub(crate) fn cycle_song_repeat(cx: &mut App) {
    change(cx, |values| values.song_repeat = values.song_repeat.cycle());
}

pub(crate) fn song_order(cx: &App) -> crate::player::Order {
    values(cx).map_or_else(crate::player::Order::default, |values| values.song_order)
}

pub(crate) fn set_song_order(cx: &mut App, order: crate::player::Order) {
    change(cx, |values| values.song_order = order);
}

pub(crate) fn song_volume(cx: &App) -> u8 {
    values(cx).map_or(100, |values| values.song_volume.min(100))
}

pub(crate) fn toggled_song_volume(cx: &App) -> u8 {
    if song_volume(cx) == 0 {
        values(cx).map_or(100, |values| values.unmuted_song_volume.clamp(1, 100))
    } else {
        0
    }
}

pub(crate) fn set_song_volume(cx: &mut App, volume: u8) {
    let volume = volume.min(100);
    change(cx, |values| {
        values.song_volume = volume;
        if volume > 0 {
            values.unmuted_song_volume = volume;
        }
    });
}

pub(crate) fn set_large_emoji(large: bool, cx: &mut App) {
    change(cx, |values| values.large_emoji = large);
    cx.refresh_windows();
}

pub(crate) fn is_pull_to_next_channel(cx: &App) -> bool {
    values(cx).is_none_or(|values| values.pull_to_next_channel)
}

pub(crate) fn set_pull_to_next_channel(enabled: bool, cx: &mut App) {
    change(cx, |values| values.pull_to_next_channel = enabled);
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

pub(crate) fn notify(cx: &App) -> Notify {
    values(cx).map_or_else(Notify::default, |values| values.notify)
}

pub(crate) fn set_notify(notify: Notify, cx: &mut App) {
    change(cx, |values| values.notify = notify);
}

/// Power Usage's switches as tdesktop's bits, for the snapshots' soak.
#[doc(hidden)]
pub fn preset_power_saving(bits: u32, cx: &mut App) {
    set_power_saving(crate::power_saving::Saving(bits), cx);
}

pub(crate) fn power_saving(cx: &App) -> crate::power_saving::Saving {
    crate::power_saving::Saving(values(cx).map_or(0, |values| values.power_saving))
}

pub(crate) fn set_power_saving(saving: crate::power_saving::Saving, cx: &mut App) {
    crate::power_saving::apply(saving);
    change(cx, |values| values.power_saving = saving.0);
}

/// The interface scale in percent; 0 is the default, 100 % (`cConfigScale`).
#[must_use]
pub fn interface_scale(cx: &App) -> u32 {
    values(cx).map_or(0, |values| values.interface_scale)
}

/// How many points a pixel of the interface takes, read once at start.
#[must_use]
pub fn interface_zoom(cx: &App) -> f32 {
    match interface_scale(cx) {
        0 => 1.0,
        #[expect(clippy::cast_precision_loss, reason = "a few hundred percent")]
        scale => scale as f32 / 100.0,
    }
}

/// Takes effect at the next start, as in tdesktop.
pub(crate) fn set_interface_scale(scale: u32, cx: &mut App) {
    change(cx, |values| values.interface_scale = scale);
}

fn apply_devices(values: &Values) {
    let id = |id: &str| Some(id.to_owned()).filter(|id| !id.is_empty());
    media::devices::choose(Direction::Output, id(&values.playback_device));
    media::devices::choose(Direction::Input, id(&values.capture_device));
}

/// The chosen speakers or microphone; None is the system's.
pub(crate) fn audio_device(direction: Direction, cx: &App) -> Option<String> {
    let id = values(cx).map(|values| match direction {
        Direction::Output => values.playback_device.clone(),
        Direction::Input => values.capture_device.clone(),
    });
    id.filter(|id| !id.is_empty())
}

/// Sound goes through `id` from now on, also after a restart.
pub(crate) fn set_audio_device(direction: Direction, id: Option<String>, cx: &mut App) {
    media::devices::choose(direction, id.clone());
    let id = id.unwrap_or_default();
    change(cx, |values| match direction {
        Direction::Output => values.playback_device = id,
        Direction::Input => values.capture_device = id,
    });
}

/// The name TDLib reports for this device, if the owner renamed it.
#[must_use]
pub fn custom_device_model(cx: &App) -> Option<String> {
    values(cx)
        .map(|values| values.custom_device_model.clone())
        .filter(|name| !name.is_empty())
}

/// Takes effect at the next start: TDLib reads the name once.
pub(crate) fn set_custom_device_model(name: String, cx: &mut App) {
    change(cx, |values| values.custom_device_model = name);
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

/// How many recent settings searches are kept (`RecentSearches::kLimit`).
const RECENT_SETTINGS_SEARCHES: usize = 32;

pub(crate) fn recent_settings_searches(cx: &App) -> Vec<String> {
    values(cx).map_or_else(Vec::new, |values| values.recent_settings_searches.clone())
}

/// Moves `id` to the front of the recent settings searches.
pub(crate) fn bump_settings_search(id: &str, cx: &mut App) {
    change(cx, |values| {
        let list = &mut values.recent_settings_searches;
        list.retain(|known| known != id);
        list.insert(0, id.to_owned());
        list.truncate(RECENT_SETTINGS_SEARCHES);
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
fn known_accounts(stored: Vec<u32>, active: u32) -> (Vec<u32>, u32) {
    let mut list = Vec::with_capacity(crate::accounts::PREMIUM_MAX_ACCOUNTS);
    for &id in &stored {
        if !list.contains(&id) {
            list.push(id);
            if list.len() == crate::accounts::PREMIUM_MAX_ACCOUNTS {
                break;
            }
        }
    }
    if list.len() != stored.len() {
        tracing::warn!("ignoring duplicate or excess saved account slots");
        if !list.contains(&active)
            && stored.contains(&active)
            && let Some(last) = list.last_mut()
        {
            *last = active;
        }
    }
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

fn apply_theme(values: &Values, cx: &mut App) {
    let theme = if values.night_mode {
        values.night_theme
    } else {
        values.day_theme
    };
    cx.set_global(Theme {
        palette: theme.palette(),
    });
}

/// Writes through a temporary file so a crash leaves the old settings.
fn save(path: &Path, values: &Values) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(values).map_err(std::io::Error::other)?;
    check_size(&json)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut temporary = tempfile::NamedTempFile::new_in(dir)?;
    temporary.write_all(&json)?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
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
    fn oversized_settings_and_invalid_values_are_safe_to_report() {
        let path =
            std::env::temp_dir().join(format!("oxigram-settings-limits-{}", std::process::id()));
        let mut bytes = b"{}".to_vec();
        bytes.resize(usize::try_from(MAX_SETTINGS_BYTES).unwrap() + 1, b' ');
        std::fs::write(&path, bytes).unwrap();
        let oversized = read_values(&path);
        std::fs::write(&path, br#"{"night_mode":"private device name"}"#).unwrap();
        let invalid = read_values(&path).unwrap_err();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            oversized.unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        assert!(!invalid.to_string().contains("private"));
    }

    #[test]
    fn restoring_accounts_bounds_clients_and_keeps_the_active_account() {
        let (ids, active) = known_accounts(vec![2, 2, 4, 5, 6, 7, 8, 9], 9);
        assert_eq!(ids, [2, 4, 5, 6, 7, 9]);
        assert_eq!(active, 9);
    }

    #[test]
    fn saves_and_reads_back_the_values() {
        let dir = std::env::temp_dir().join(format!("oxigram-settings-{}", std::process::id()));
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
            recent_settings_searches: vec!["main/language".to_owned()],
            notify: Notify {
                view: NotifyView::NameOnly,
                ..Notify::default()
            },
            custom_device_model: "Work Mac".to_owned(),
            day_theme: EmbeddedTheme::Day,
            night_theme: EmbeddedTheme::Night,
            system_night_mode: false,
            large_emoji: false,
            pull_to_next_channel: false,
            playback_device: "coreaudio:speakers".to_owned(),
            capture_device: "coreaudio:mic".to_owned(),
            power_saving: 5,
            interface_scale: 150,
            voice_speed: crate::speed::Speed::default().chosen(1.5),
            audio_speed: crate::speed::Speed::default(),
            song_repeat: crate::player::Repeat::All,
            song_order: crate::player::Order::Shuffle,
            song_volume: 0,
            unmuted_song_volume: 50,
            video_speed: crate::speed::Speed::default().toggled(),
        };
        let sibling = path.with_extension("json.tmp");
        std::fs::write(&sibling, b"other writer").unwrap();
        save(&path, &values).expect("the temp dir is writable");
        assert_eq!(std::fs::read(&sibling).unwrap(), b"other writer");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
        let read = read_values(&path).expect("just written");
        assert_eq!(read, values);
        // Unknown and missing keys fall back to the defaults.
        let partial: Values = serde_json::from_str(r#"{"future_key": 1}"#).expect("valid");
        assert_eq!(partial, Values::default());
        std::fs::remove_dir_all(&dir).ok();
    }
}
