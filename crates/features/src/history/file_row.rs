//! A file at the top of its bubble (`HistoryDocument`): a circle with what a
//! click does (download, cancel with the progress arc, open, play), the name
//! or a voice's waveform, and the size, length or how much moved.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Hsla, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, canvas, div, px,
};
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::chat::{
    MEDIA_UNREAD_SIZE, MEDIA_UNREAD_SKIP, MEDIA_UNREAD_TOP, MSG_FILE_LAYOUT, MSG_FILE_MIN_WIDTH,
    MSG_FILE_RADIAL_LINE, MSG_PADDING,
};
use style::chat_style::{
    HISTORY_FILE_IN_CANCEL, HISTORY_FILE_IN_CANCEL_SELECTED, HISTORY_FILE_IN_DOCUMENT,
    HISTORY_FILE_IN_DOCUMENT_SELECTED, HISTORY_FILE_IN_DOWNLOAD, HISTORY_FILE_IN_DOWNLOAD_SELECTED,
    HISTORY_FILE_IN_PAUSE, HISTORY_FILE_IN_PAUSE_SELECTED, HISTORY_FILE_IN_PLAY,
    HISTORY_FILE_IN_PLAY_SELECTED, HISTORY_FILE_OUT_CANCEL, HISTORY_FILE_OUT_CANCEL_SELECTED,
    HISTORY_FILE_OUT_DOCUMENT, HISTORY_FILE_OUT_DOCUMENT_SELECTED, HISTORY_FILE_OUT_DOWNLOAD,
    HISTORY_FILE_OUT_DOWNLOAD_SELECTED, HISTORY_FILE_OUT_PAUSE, HISTORY_FILE_OUT_PAUSE_SELECTED,
    HISTORY_FILE_OUT_PLAY, HISTORY_FILE_OUT_PLAY_SELECTED,
};
use style::{ColorId, ColorRef, Icon};
use ui::{RadialState, color, font, font_height, paint_arc};

use super::bubble::OnClick;
use crate::px_of;

/// A full turn in Qt's arc units, and twelve o'clock where the arc starts.
const FULL_ARC: f32 = 360.0 * 16.0;
const QUARTER_ARC: i64 = 90 * 16;
/// The arc never shrinks below this share, so a starting transfer shows.
const MIN_PROGRESS: f32 = 0.02;

/// What the circle shows, and so what a click on the row does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Thumb {
    /// Not on disk: a click downloads.
    Download,
    /// Moving, `progress` of it done: a click cancels.
    Cancel { progress: f32 },
    /// On disk: a click opens it.
    Document,
    /// A voice on disk, still or paused: a click plays it.
    Play,
    /// A voice playing: a click pauses it.
    Pause,
}

/// What goes beside the circle above the status.
pub(super) enum Title {
    Name(String),
    Waveform(super::waveform::Waveform),
}

pub(super) struct FileRow {
    pub title: Title,
    pub status: String,
    /// A voice nobody played yet: a dot after the status.
    pub is_unread: bool,
    pub thumb: Thumb,
    pub on_click: OnClick,
}

/// Where the row sits in its bubble.
pub(super) struct Place {
    pub id: u64,
    pub is_out: bool,
    pub is_selected: bool,
    pub has_header: bool,
    /// Room for the time after the status when nothing follows the row,
    /// which then reaches the bubble's bottom.
    pub time_room: Option<String>,
}

/// The share of `size` done, kept between the arc's ends.
#[expect(clippy::cast_precision_loss, reason = "a share needs no exact bytes")]
pub(super) fn progress(done: i64, size: i64) -> f32 {
    if size <= 0 {
        return MIN_PROGRESS;
    }
    (done as f32 / size as f32).clamp(MIN_PROGRESS, 1.0)
}

fn icon(thumb: Thumb, is_out: bool, is_selected: bool) -> Icon {
    match (thumb, is_out, is_selected) {
        (Thumb::Download, false, false) => HISTORY_FILE_IN_DOWNLOAD,
        (Thumb::Download, false, true) => HISTORY_FILE_IN_DOWNLOAD_SELECTED,
        (Thumb::Download, true, false) => HISTORY_FILE_OUT_DOWNLOAD,
        (Thumb::Download, true, true) => HISTORY_FILE_OUT_DOWNLOAD_SELECTED,
        (Thumb::Cancel { .. }, false, false) => HISTORY_FILE_IN_CANCEL,
        (Thumb::Cancel { .. }, false, true) => HISTORY_FILE_IN_CANCEL_SELECTED,
        (Thumb::Cancel { .. }, true, false) => HISTORY_FILE_OUT_CANCEL,
        (Thumb::Cancel { .. }, true, true) => HISTORY_FILE_OUT_CANCEL_SELECTED,
        (Thumb::Document, false, false) => HISTORY_FILE_IN_DOCUMENT,
        (Thumb::Document, false, true) => HISTORY_FILE_IN_DOCUMENT_SELECTED,
        (Thumb::Document, true, false) => HISTORY_FILE_OUT_DOCUMENT,
        (Thumb::Document, true, true) => HISTORY_FILE_OUT_DOCUMENT_SELECTED,
        (Thumb::Play, false, false) => HISTORY_FILE_IN_PLAY,
        (Thumb::Play, false, true) => HISTORY_FILE_IN_PLAY_SELECTED,
        (Thumb::Play, true, false) => HISTORY_FILE_OUT_PLAY,
        (Thumb::Play, true, true) => HISTORY_FILE_OUT_PLAY_SELECTED,
        (Thumb::Pause, false, false) => HISTORY_FILE_IN_PAUSE,
        (Thumb::Pause, false, true) => HISTORY_FILE_IN_PAUSE_SELECTED,
        (Thumb::Pause, true, false) => HISTORY_FILE_OUT_PAUSE,
        (Thumb::Pause, true, true) => HISTORY_FILE_OUT_PAUSE_SELECTED,
    }
}

/// The colors of the circle, the radial, the name and the status.
fn colors(is_out: bool, is_selected: bool) -> [ColorId; 4] {
    match (is_out, is_selected) {
        (false, false) => [
            ColorId::MsgFileInBg,
            ColorId::HistoryFileInRadialFg,
            ColorId::HistoryFileNameInFg,
            ColorId::MediaInFg,
        ],
        (false, true) => [
            ColorId::MsgFileInBgSelected,
            ColorId::HistoryFileInRadialFgSelected,
            ColorId::HistoryFileNameInFgSelected,
            ColorId::MediaInFgSelected,
        ],
        (true, false) => [
            ColorId::MsgFileOutBg,
            ColorId::HistoryFileOutRadialFg,
            ColorId::HistoryFileNameOutFg,
            ColorId::MediaOutFg,
        ],
        (true, true) => [
            ColorId::MsgFileOutBgSelected,
            ColorId::HistoryFileOutRadialFgSelected,
            ColorId::HistoryFileNameOutFgSelected,
            ColorId::MediaOutFgSelected,
        ],
    }
}

fn circle(file: &FileRow, place: &Place, bg: ColorId, radial: ColorId, cx: &App) -> AnyElement {
    let st = &MSG_FILE_LAYOUT;
    let arc = match file.thumb {
        Thumb::Cancel { progress } => Some(progress),
        Thumb::Download | Thumb::Document | Thumb::Play | Thumb::Pause => None,
    };
    let radial_fg = color(cx, ColorRef::Palette(radial));
    div()
        .relative()
        .flex_none()
        .size(px_of(st.thumb_size))
        .rounded_full()
        .bg(color(cx, ColorRef::Palette(bg)))
        .flex()
        .items_center()
        .justify_center()
        .child(ui::icon(icon(file.thumb, place.is_out, place.is_selected)))
        .children(arc.map(|progress| self::arc(progress, radial_fg)))
        .into_any_element()
}

/// The transfer's arc over a round button, `progress` of a turn from twelve
/// o'clock.
pub(super) fn arc(progress: f32, fg: Hsla) -> AnyElement {
    #[expect(clippy::cast_possible_truncation, reason = "within a full turn")]
    let state = RadialState {
        shown: 1.0,
        arc_from: QUARTER_ARC,
        arc_length: -(FULL_ARC * progress).round() as i64,
    };
    radial_arc(state, fg)
}

/// `state`'s arc inside a thumb circle.
pub(super) fn radial_arc(state: RadialState, fg: Hsla) -> AnyElement {
    let line_width = px_of(MSG_FILE_RADIAL_LINE);
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            // The arc runs inside the circle by its width (`msgFileRadialLine`).
            let arc = bounds.dilate(-line_width - line_width / 2.0);
            paint_arc(window, arc, state, line_width, fg);
        },
    )
    .absolute()
    .top(px(0.0))
    .left(px(0.0))
    .size_full()
    .into_any_element()
}

/// The row, its top on the bubble's unless a header is above it.
// shortcut: a still arc; tdesktop spins it while the transfer runs.
pub(super) fn row(file: FileRow, place: &Place, cx: &App) -> AnyElement {
    let st = &MSG_FILE_LAYOUT;
    let [bg, radial, name_fg, status_fg] = colors(place.is_out, place.is_selected);
    let line = |text_font: style::Font, fg: ColorId| {
        div()
            .min_w_0()
            .font(font(text_font))
            .text_size(px_of(text_font.size))
            .line_height(font_height(text_font))
            .text_color(color(cx, ColorRef::Palette(fg)))
            .whitespace_nowrap()
            .overflow_hidden()
            .text_ellipsis()
    };
    let on_click = file.on_click.clone();
    let thumb = circle(&file, place, bg, radial, cx);
    // Transparent figure spaces keep the status clear of the time.
    let room = place.time_room.clone().map(|room| {
        div()
            .flex_none()
            .text_color(gpui::transparent_black())
            .child(room)
    });
    let title = match file.title {
        Title::Name(name) => div()
            .pt(px_of(st.name_top) - px_of(st.padding.top))
            .child(line(SEMIBOLD_FONT, name_fg).child(name))
            .into_any_element(),
        Title::Waveform(waveform) => {
            super::waveform::element(waveform, place.is_out, place.is_selected, cx)
        }
    };
    let unread = file.is_unread.then(|| {
        div()
            .flex_none()
            .ml(px_of(MEDIA_UNREAD_SKIP))
            .mt(px_of(MEDIA_UNREAD_TOP))
            .size(px_of(MEDIA_UNREAD_SIZE))
            .rounded_full()
            .bg(color(cx, ColorRef::Palette(bg)))
    });
    let texts = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .h(px_of(st.status_top) - px_of(st.padding.top))
                .child(title),
        )
        .child(
            div()
                .flex()
                .child(line(NORMAL_FONT, status_fg).child(file.status))
                .children(unread)
                .children(room),
        );
    div()
        .id(("file-row", place.id))
        .flex()
        .gap(px_of(st.thumb_skip))
        .ml(-px_of(MSG_PADDING.left))
        .mr(-px_of(MSG_PADDING.right))
        .when(!place.has_header, |row| row.mt(-px_of(MSG_PADDING.top)))
        .when(place.time_room.is_some(), |row| {
            row.mb(-px_of(MSG_PADDING.bottom))
        })
        .min_w(px_of(MSG_FILE_MIN_WIDTH))
        .h(px_of(st.padding.top) + px_of(st.thumb_size) + px_of(st.padding.bottom))
        .pt(px_of(st.padding.top))
        .pl(px_of(st.padding.left))
        .pr(px_of(st.padding.right))
        .cursor_pointer()
        .child(thumb)
        .child(texts)
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_shows_a_sliver_at_the_start_and_stops_at_full() {
        assert_eq!(progress(0, 100), MIN_PROGRESS);
        assert_eq!(progress(50, 100), 0.5);
        assert_eq!(progress(150, 100), 1.0);
        assert_eq!(progress(10, 0), MIN_PROGRESS);
    }

    #[test]
    fn the_circle_shows_what_a_click_does() {
        assert_eq!(
            icon(Thumb::Download, false, false),
            HISTORY_FILE_IN_DOWNLOAD
        );
        assert_eq!(
            icon(Thumb::Cancel { progress: 0.5 }, true, true),
            HISTORY_FILE_OUT_CANCEL_SELECTED
        );
        assert_eq!(
            icon(Thumb::Document, true, false),
            HISTORY_FILE_OUT_DOCUMENT
        );
    }
}
