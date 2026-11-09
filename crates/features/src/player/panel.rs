//! The playlist under the player bar (`Media::Player::Panel`): the chat's
//! songs around the one playing, newest first. It opens while the pointer
//! rests on the bar's name and time, and closes a moment after the pointer
//! leaves both; a click on a song plays or pauses it.

use std::time::Duration;

use gpui::{
    AnyElement, App, BoxShadow, Context, Entity, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Pixels, ScrollHandle, StatefulInteractiveElement as _, Styled as _, Task,
    anchored, deferred, div, hsla, point, px,
};
use model::MessageId;
use style::basic::NORMAL_FONT;
use style::info::INFO_MEDIA_MARGIN;
use style::media_player::{
    MEDIA_PLAYER_FILE_LAYOUT, MEDIA_PLAYER_HEIGHT, MEDIA_PLAYER_LIST_HEIGHT_MAX,
    MEDIA_PLAYER_LIST_MARGIN_BOTTOM, MEDIA_PLAYER_PANEL_MARGIN_BOTTOM,
    MEDIA_PLAYER_PANEL_MARGIN_LEFT, MEDIA_PLAYER_PANEL_WIDTH,
};
use style::widgets::DEFAULT_POPUP_MENU;
use style::{ColorId, ColorRef};
use ui::{color, font, font_height};

use super::{Player, Playing, Song, Track};
use crate::history::duration_text;
use crate::px_of;

/// `Panel::showFromOther` and the leave timers.
const TOGGLE_DELAY: Duration = Duration::from_millis(300);

/// Whether the pointer is over the bar's labels or the panel, and whether
/// the panel shows.
#[derive(Default)]
pub(crate) struct Hover {
    over_labels: bool,
    over_panel: bool,
    is_shown: bool,
    /// The panel's left edge from the bar's.
    left: Pixels,
    scroll: ScrollHandle,
    toggle: Option<Task<()>>,
}

impl Player {
    pub(super) fn is_over_labels(&self) -> bool {
        self.hover.over_labels
    }

    /// The pointer came onto or left the name and time, `x` from the bar's
    /// left (`togglePlaylistRequests`).
    pub(super) fn hover_labels(&mut self, is_over: bool, x: Pixels, cx: &mut Context<Self>) {
        if is_over && !self.hover.is_shown {
            self.hover.left = panel_left(x);
        }
        self.hover.over_labels = is_over;
        self.schedule_toggle(cx);
    }

    fn hover_panel(&mut self, is_over: bool, cx: &mut Context<Self>) {
        self.hover.over_panel = is_over;
        self.schedule_toggle(cx);
    }

    /// Shows or hides the panel after the delay; a newer hover replaces it.
    fn schedule_toggle(&mut self, cx: &mut Context<Self>) {
        self.hover.toggle = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOGGLE_DELAY).await;
            this.update(cx, |this, cx| this.toggle_panel(cx)).ok();
        }));
    }

    fn toggle_panel(&mut self, cx: &mut Context<Self>) {
        let is_shown = self.hover.over_labels || self.hover.over_panel;
        if self.hover.is_shown == is_shown {
            return;
        }
        self.hover.is_shown = is_shown;
        // `scrollPlaylistToCurrentTrack`.
        let current = self
            .playing
            .as_ref()
            .and_then(|playing| current_row(&playing.playlist.as_ref()?.songs, playing.message));
        if let (true, Some(row)) = (is_shown, current) {
            self.hover.scroll.scroll_to_top_of_item(row);
        }
        cx.notify();
    }

    pub(super) fn reset_hover(&mut self) {
        self.hover = Hover::default();
    }
}

/// The row of `message` in the panel, which lists `songs` newest first.
fn current_row(songs: &[Song], message: MessageId) -> Option<usize> {
    let index = songs.iter().position(|song| song.message == message)?;
    Some(songs.len() - 1 - index)
}

/// `bestPositionFor`: the first song's circle under the pointer.
fn panel_left(x: Pixels) -> Pixels {
    let layout = &MEDIA_PLAYER_FILE_LAYOUT;
    x - px_of(MEDIA_PLAYER_PANEL_MARGIN_LEFT)
        - px_of(layout.song_padding.left)
        - px_of(layout.song_thumb_size) / 2.0
}

/// The open panel, under the bar and kept inside the window
/// (`updateMediaPlaylistPosition`); none while it is closed or empty.
// shortcut: no month headers, no download icon or progress for a song not on disk yet, no fade; with the shared media grid (row "Shared media").
pub(super) fn panel(player: &Entity<Player>, cx: &App) -> Option<AnyElement> {
    let state = player.read(cx);
    if !state.hover.is_shown {
        return None;
    }
    let playing = state.current()?;
    let songs = &playing.playlist.as_ref()?.songs;
    if songs.is_empty() {
        return None;
    }
    let rows = songs
        .iter()
        .rev()
        .enumerate()
        .map(|(index, song)| row(player, index, song, playing, cx));
    let list = div()
        .id("media-player-list")
        .max_h(px_of(MEDIA_PLAYER_LIST_HEIGHT_MAX))
        .overflow_y_scroll()
        .track_scroll(&state.hover.scroll)
        .pt(px_of(INFO_MEDIA_MARGIN.top))
        .pb(px_of(INFO_MEDIA_MARGIN.bottom))
        .children(rows);
    let popup = &DEFAULT_POPUP_MENU;
    #[expect(clippy::cast_possible_truncation, reason = "an opacity in 0..1")]
    let shadow = hsla(0.0, 0.0, 0.0, popup.shadow.opacity as f32);
    let frame = div()
        .w(px_of(MEDIA_PLAYER_PANEL_WIDTH))
        .pb(px_of(MEDIA_PLAYER_LIST_MARGIN_BOTTOM))
        .rounded(px_of(popup.radius))
        .overflow_hidden()
        .bg(color(cx, ColorRef::Palette(ColorId::MenuBg)))
        .shadow(vec![BoxShadow {
            color: shadow,
            offset: point(px_of(popup.shadow.offset.x), px_of(popup.shadow.offset.y)),
            blur_radius: px_of(popup.shadow.blur_radius),
            spread_radius: px(0.0),
            inset: false,
        }])
        .child(list);
    // The margins around the frame count as the panel, as tdesktop's.
    let margin = px_of(MEDIA_PLAYER_PANEL_MARGIN_LEFT);
    let hovered = player.clone();
    let panel = div()
        .id("media-player-panel")
        .occlude()
        .pt(margin)
        .px(margin)
        .pb(px_of(MEDIA_PLAYER_PANEL_MARGIN_BOTTOM))
        .on_hover(move |is_over, _, cx| {
            hovered.update(cx, |player, cx| player.hover_panel(*is_over, cx));
        })
        .child(frame);
    let placed = anchored()
        .offset(point(state.hover.left, px(0.0)))
        .snap_to_window()
        .child(panel);
    Some(
        div()
            .absolute()
            .left_0()
            .top(px_of(MEDIA_PLAYER_HEIGHT))
            .child(deferred(placed))
            .into_any_element(),
    )
}

/// `Overview::Layout::Document` for a song: the circle with play or pause,
/// the name, and the length, or the time played of the one playing.
fn row(
    player: &Entity<Player>,
    index: usize,
    song: &Song,
    playing: &Playing,
    cx: &App,
) -> AnyElement {
    let layout = &MEDIA_PLAYER_FILE_LAYOUT;
    let padding = layout.song_padding;
    let thumb = px_of(layout.song_thumb_size);
    let is_current = song.message == playing.message;
    let icon = if is_current && !playing.sound.is_paused() {
        layout.song_pause
    } else {
        layout.song_play
    };
    let circle = div()
        .absolute()
        .left(px_of(padding.left))
        .top(px_of(padding.top))
        .size(thumb)
        .rounded_full()
        .bg(color(cx, layout.song_icon_bg))
        .flex()
        .items_center()
        .justify_center()
        .child(ui::icon(icon));
    let name_left = px_of(padding.left) + thumb + px_of(padding.right);
    let (performer, title) = match &song.track {
        Track::Song { performer, title } => (performer.as_str(), title.as_str()),
        Track::Voice { name, date } => (name.as_str(), date.as_str()),
    };
    let fg = color(cx, ColorRef::Palette(ColorId::HistoryFileNameInFg));
    let name = div()
        .absolute()
        .left(name_left)
        .right(px_of(padding.left))
        .top(px_of(layout.song_name_top))
        .flex()
        .child(super::bar::name(performer, title, fg));
    let status = div()
        .absolute()
        .left(name_left)
        .top(px_of(layout.song_status_top))
        .font(font(NORMAL_FONT))
        .text_size(px_of(NORMAL_FONT.size))
        .line_height(font_height(NORMAL_FONT))
        .text_color(color(cx, ColorRef::Palette(ColorId::MediaInFg)))
        .child(status_text(song, is_current.then_some(playing)));
    let chosen = song.clone();
    let player = player.clone();
    div()
        .id(("media-player-song", index))
        .relative()
        .h(px_of(padding.top) + thumb + px_of(padding.bottom))
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            player.update(cx, |player, cx| player.choose(chosen.clone(), cx));
        })
        .child(circle)
        .child(name)
        .child(status)
        .into_any_element()
}

/// The length, or "played / length" for the song playing.
fn status_text(song: &Song, playing: Option<&Playing>) -> String {
    let length = duration_text(song.duration);
    match playing {
        Some(playing) => {
            let played = i32::try_from(playing.sound.position().as_secs()).unwrap_or(i32::MAX);
            format!("{} / {length}", duration_text(played))
        }
        None => length,
    }
}

#[cfg(test)]
mod tests {
    use model::FileId;

    use super::*;

    #[test]
    fn the_newest_song_is_the_first_row() {
        let songs: Vec<Song> = [10, 20, 30]
            .into_iter()
            .map(|id| Song {
                message: MessageId(id),
                file: FileId(1),
                track: Track::Voice {
                    name: String::new(),
                    date: String::new(),
                },
                duration: 0,
            })
            .collect();
        assert_eq!(current_row(&songs, MessageId(30)), Some(0));
        assert_eq!(current_row(&songs, MessageId(10)), Some(2));
        assert_eq!(current_row(&songs, MessageId(15)), None);
    }
}
