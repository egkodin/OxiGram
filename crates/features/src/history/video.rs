//! A video or GIF in its bubble (`HistoryView::Gif` before it plays): the
//! still TDLib sends, a round button in the middle with what a click does,
//! and the length or the transfer on a pill in the top-left corner.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Corners, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _, div, px, relative,
};
use i18n::key;
use model::message::{Content, Media, Message};
use style::basic::NORMAL_FONT;
use style::chat::{
    HISTORY_VIDEO_TIMESTAMP_PROGRESS_LINE, MAX_VIDEO_MESSAGE_SIZE, MEDIA_UNREAD_SIZE,
    MEDIA_UNREAD_SKIP, MSG_DATE_IMG_DELTA, MSG_DATE_IMG_PADDING, MSG_FILE_LAYOUT,
};
use style::chat_style::{
    HISTORY_FILE_THUMB_CANCEL, HISTORY_FILE_THUMB_CANCEL_SELECTED, HISTORY_FILE_THUMB_DOWNLOAD,
    HISTORY_FILE_THUMB_DOWNLOAD_SELECTED, HISTORY_FILE_THUMB_PLAY,
    HISTORY_FILE_THUMB_PLAY_SELECTED,
};
use style::{ColorId, ColorRef, Icon};
use ui::{color, font, font_height};

use super::HistoryView;
use super::bubble::OnClick;
use super::documents::{CLICKED_PRIORITY, current};
use super::file_row::progress;
use super::photo::Photo;
use crate::file_sizes::{progress_text, size_text};
use crate::lang::{strings, tr};
use crate::px_of;

/// What the middle button shows, and so what a click on the video does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Center {
    /// Not on disk: a click downloads.
    Download,
    /// Moving, `progress` of it done: a click cancels.
    Cancel { progress: f32 },
    /// On disk: a click plays it.
    Play,
}

/// Where the status goes and how it looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    /// A video or GIF: the pill top left.
    Frame,
    /// A round video: the service pill bottom left, a dot until viewed.
    Round { is_unviewed: bool },
}

pub(super) struct Video {
    pub center: Center,
    pub status: String,
    pub shape: Shape,
    pub on_click: OnClick,
    /// How far into the video it starts, 0 to 1, when the sender set a
    /// timestamp.
    pub timestamp: Option<f32>,
}

/// Where the timestamp mark sits in the video, when it has one.
#[expect(clippy::cast_precision_loss, reason = "video lengths in seconds")]
fn timestamp(media: &Media) -> Option<f32> {
    (media.start_timestamp > 0 && media.start_timestamp <= media.duration)
        .then(|| media.start_timestamp as f32 / media.duration as f32)
}

/// "0:07", "12:30", "1:02:03" (`FormatDurationText`).
pub(super) fn duration_text(seconds: i32) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn icon(center: Center, is_selected: bool) -> Icon {
    match (center, is_selected) {
        (Center::Download, false) => HISTORY_FILE_THUMB_DOWNLOAD,
        (Center::Download, true) => HISTORY_FILE_THUMB_DOWNLOAD_SELECTED,
        (Center::Cancel { .. }, false) => HISTORY_FILE_THUMB_CANCEL,
        (Center::Cancel { .. }, true) => HISTORY_FILE_THUMB_CANCEL_SELECTED,
        (Center::Play, false) => HISTORY_FILE_THUMB_PLAY,
        (Center::Play, true) => HISTORY_FILE_THUMB_PLAY_SELECTED,
    }
}

impl HistoryView {
    /// A video or GIF message's still with its button and status.
    pub(super) fn video(&mut self, message: &Message, cx: &mut Context<Self>) -> Option<Photo> {
        let (media, is_gif, shape) = match &message.content {
            Content::Video(media) => (media, false, Shape::Frame),
            Content::Animation(media) => (media, true, Shape::Frame),
            Content::VideoNote { media, is_viewed } => {
                let is_unviewed = !is_viewed && !message.is_outgoing;
                (&**media, false, Shape::Round { is_unviewed })
            }
            _ => return None,
        };
        let id = message.id;
        let file = current(self, media.file.as_ref()?, cx).clone();
        // TDLib's GIFs are muted loops, "GIF" stands for their length.
        let length = if is_gif {
            "GIF".to_owned()
        } else {
            duration_text(media.duration)
        };
        let (center, status) = match (&file.path, file.is_downloading) {
            (Some(_), _) => (Center::Play, length),
            (None, true) => (
                Center::Cancel {
                    progress: progress(file.downloaded, file.size),
                },
                progress_text(strings(cx), file.downloaded, file.size),
            ),
            (None, false) => (
                Center::Download,
                i18n::fill(
                    &tr(cx, key::lng_duration_and_size),
                    &[("duration", &length), ("size", &size_text(file.size))],
                ),
            ),
        };
        let weak = cx.weak_entity();
        let path = file.path.clone().map(PathBuf::from);
        let on_click: OnClick = Rc::new(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| match center {
                Center::Download => this.start_download(file.id, CLICKED_PRIORITY, cx),
                Center::Cancel { .. } => this.cancel_download(file.id, cx),
                // shortcut: the system player plays it; inline playback comes with the media viewer (row 292).
                Center::Play => {
                    if let Some(path) = &path {
                        if matches!(shape, Shape::Round { is_unviewed: true }) {
                            this.mark_listened(id, cx);
                        }
                        this.open_file(path, window, cx);
                    }
                }
            });
        });
        let mut still = self.still(media.preview.as_ref(), media, cx);
        if let Shape::Round { .. } = shape {
            // Round videos take one size whatever their length.
            (still.width, still.height) = (MAX_VIDEO_MESSAGE_SIZE.0, MAX_VIDEO_MESSAGE_SIZE.0);
        }
        Some(Photo {
            video: Some(Video {
                center,
                status,
                shape,
                on_click,
                timestamp: matches!(message.content, Content::Video(_))
                    .then(|| timestamp(media))
                    .flatten(),
            }),
            ..still
        })
    }
}

/// The button over the whole picture, which takes its clicks, and the
/// status pill in its corner.
pub(super) fn overlays(
    video: Video,
    id: i64,
    corners: Corners<Pixels>,
    is_selected: bool,
    cx: &App,
) -> Vec<AnyElement> {
    let bg = color(
        cx,
        ColorRef::Palette(if is_selected {
            ColorId::MsgDateImgBgSelected
        } else {
            ColorId::MsgDateImgBg
        }),
    );
    let radial = color(
        cx,
        ColorRef::Palette(if is_selected {
            ColorId::HistoryFileThumbRadialFgSelected
        } else {
            ColorId::HistoryFileThumbRadialFg
        }),
    );
    let arc = match video.center {
        Center::Cancel { progress } => Some(super::file_row::arc(progress, radial)),
        Center::Download | Center::Play => None,
    };
    let circle = div()
        .relative()
        .size(px_of(MSG_FILE_LAYOUT.thumb_size))
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .child(ui::icon(icon(video.center, is_selected)))
        .children(arc);
    let on_click = video.on_click;
    let button = div()
        .id(("video", id.unsigned_abs()))
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(circle)
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .into_any_element();
    let (status_bg, status_fg) = match (video.shape, is_selected) {
        (Shape::Frame, _) => (bg, ColorId::MsgDateImgFg),
        (Shape::Round { .. }, false) => (palette(cx, ColorId::MsgServiceBg), ColorId::MsgServiceFg),
        (Shape::Round { .. }, true) => (
            palette(cx, ColorId::MsgServiceBgSelected),
            ColorId::MsgServiceFg,
        ),
    };
    let status_fg = palette(cx, status_fg);
    let dot = matches!(video.shape, Shape::Round { is_unviewed: true }).then(|| {
        div()
            .ml(px_of(MEDIA_UNREAD_SKIP))
            .size(px_of(MEDIA_UNREAD_SIZE))
            .rounded_full()
            .bg(status_fg)
    });
    let status = div()
        .absolute()
        .left(px_of(MSG_DATE_IMG_DELTA))
        .map(|this| match video.shape {
            Shape::Frame => this.top(px_of(MSG_DATE_IMG_DELTA)),
            Shape::Round { .. } => this.bottom(px_of(MSG_DATE_IMG_DELTA)),
        })
        .flex()
        .items_center()
        .px(px_of(MSG_DATE_IMG_PADDING.x))
        .py(px_of(MSG_DATE_IMG_PADDING.y))
        .rounded_full()
        .bg(status_bg)
        .font(font(NORMAL_FONT))
        .text_size(px_of(NORMAL_FONT.size))
        .line_height(font_height(NORMAL_FONT))
        .text_color(status_fg)
        .whitespace_nowrap()
        .child(video.status)
        .children(dot)
        .into_any_element();
    let mark = video
        .timestamp
        .map(|progress| timestamp_mark(progress, corners, cx));
    [button, status].into_iter().chain(mark).collect()
}

/// How far in a bottom corner of `radius` cuts into a line of `height`
/// along the bottom, at the line's middle.
fn corner_inset(radius: Pixels, height: Pixels) -> Pixels {
    let (radius, height) = (f32::from(radius), f32::from(height));
    let rise = (radius - height / 2.0).max(0.0);
    px(radius - (radius * radius - rise * rise).sqrt())
}

/// The started part along the bottom in the active color, the rest in the
/// player's (`PaintVideoTimestampMark`): the split lies `progress` of the
/// way between the two corner radii.
// shortcut: the line's ends are cut square where the corners cross its middle; tdesktop rounds them with the corners.
fn timestamp_mark(progress: f32, corners: Corners<Pixels>, cx: &App) -> AnyElement {
    let line = px_of(HISTORY_VIDEO_TIMESTAMP_PROGRESS_LINE);
    let (left, right) = (corners.bottom_left, corners.bottom_right);
    let used = div()
        .absolute()
        .bottom_0()
        .left(left)
        .right(right)
        .h(line)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(corner_inset(left, line) - left)
                .right(relative(1.0 - progress))
                .bg(palette(cx, ColorId::WindowBgActive)),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(progress))
                .right(corner_inset(right, line) - right)
                .bg(palette(cx, ColorId::MediaviewPlaybackProgressFg)),
        );
    used.into_any_element()
}

fn palette(cx: &App, id: ColorId) -> gpui::Hsla {
    color(cx, ColorRef::Palette(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timestamp_mark_follows_the_corners() {
        let media = |start, duration| Media {
            start_timestamp: start,
            duration,
            ..Media::default()
        };
        assert_eq!(timestamp(&media(30, 120)), Some(0.25));
        assert_eq!(timestamp(&media(0, 120)), None);
        assert_eq!(timestamp(&media(200, 120)), None);
        assert_eq!(corner_inset(px(0.0), px(4.0)), px(0.0));
        // A 16 px corner meets a 4 px line's middle 16 - sqrt(60) in.
        let inset = f32::from(corner_inset(px(16.0), px(4.0)));
        assert!((inset - 8.254).abs() < 0.01, "{inset}");
    }

    #[test]
    fn durations_drop_hours_under_an_hour() {
        assert_eq!(duration_text(7), "0:07");
        assert_eq!(duration_text(750), "12:30");
        assert_eq!(duration_text(3723), "1:02:03");
    }

    #[test]
    fn the_button_shows_what_a_click_does() {
        assert_eq!(icon(Center::Download, false), HISTORY_FILE_THUMB_DOWNLOAD);
        assert_eq!(
            icon(Center::Cancel { progress: 0.3 }, true),
            HISTORY_FILE_THUMB_CANCEL_SELECTED
        );
        assert_eq!(icon(Center::Play, false), HISTORY_FILE_THUMB_PLAY);
    }
}
