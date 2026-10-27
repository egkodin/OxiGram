//! A file on its way up, at the top of its bubble (`HistoryDocument` while
//! uploading): a circle with the progress arc and a cross that cancels,
//! the name, and how much went up.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, canvas, div, px,
};
use i18n::key;
use model::message::{Content, Message, Sending};
use model::message_actions::Deletion;
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::chat::{MSG_FILE_LAYOUT, MSG_FILE_MIN_WIDTH, MSG_FILE_RADIAL_LINE, MSG_PADDING};
use style::chat_style::HISTORY_FILE_OUT_CANCEL;
use style::{ColorId, ColorRef};
use ui::{RadialState, color, font, font_height, paint_arc};

use super::HistoryView;
use super::bubble::OnClick;
use crate::file_sizes::progress_text;
use crate::lang::{strings, tr};
use crate::px_of;
use crate::store::spawn;

/// A full turn in Qt's arc units, and twelve o'clock where the arc starts.
const FULL_ARC: f32 = 360.0 * 16.0;
const QUARTER_ARC: i64 = 90 * 16;
/// The arc never shrinks below this share, so a starting upload shows.
const MIN_PROGRESS: f32 = 0.02;

pub(super) struct Upload {
    pub name: String,
    pub uploaded: i64,
    pub size: i64,
    pub on_cancel: OnClick,
}

/// The share of `size` that went up, kept between the arc's ends.
#[expect(clippy::cast_precision_loss, reason = "a share needs no exact bytes")]
fn progress(uploaded: i64, size: i64) -> f32 {
    if size <= 0 {
        return MIN_PROGRESS;
    }
    (uploaded as f32 / size as f32).clamp(MIN_PROGRESS, 1.0)
}

impl HistoryView {
    /// The upload of a message being sent with a file, if it has one.
    pub(super) fn upload(&self, message: &Message, cx: &Context<Self>) -> Option<Upload> {
        if message.sending != Some(Sending::Pending) {
            return None;
        }
        let (media, name) = match &message.content {
            Content::Photo(media) => (media, tr(cx, key::lng_in_dlg_photo).to_string()),
            Content::Video(media) | Content::Animation(media) => {
                (media, tr(cx, key::lng_in_dlg_video).to_string())
            }
            Content::Document { name, media } => (media, name.clone()),
            _ => return None,
        };
        let sent = media.file.as_ref()?;
        let file = self
            .store
            .read(cx)
            .state()
            .chats
            .file(sent.id)
            .unwrap_or(sent);
        let weak = cx.weak_entity();
        let (chat, id) = (self.chat, message.id);
        Some(Upload {
            name,
            uploaded: file.uploaded,
            size: file.size,
            on_cancel: std::rc::Rc::new(move |_, _, cx| {
                // Deleting a message being sent stops its upload (`cancelUpload`).
                let _ = weak.update(cx, |this, cx| {
                    let request =
                        this.store
                            .read(cx)
                            .session()
                            .delete_messages(chat, &[id], Deletion::ForMe);
                    let task = spawn(request, cx);
                    cx.spawn(async move |_, _| {
                        if let Err(error) = task.await {
                            tracing::warn!(%error, "cancelling an upload failed");
                        }
                    })
                    .detach();
                });
            }),
        })
    }
}

/// The block, its top on the bubble's unless `has_header` puts something
/// above it.
// shortcut: a still arc, and photos and videos going up show as files; tdesktop spins the arc and draws the media, both in M5.
pub(super) fn row(upload: Upload, id: u64, has_header: bool, cx: &App) -> AnyElement {
    let st = &MSG_FILE_LAYOUT;
    let thumb = px_of(st.thumb_size);
    let name_left = px_of(st.padding.left) + thumb + px_of(st.thumb_skip);
    let line = |text: String, top, text_font: style::Font, fg: ColorId| {
        div()
            .absolute()
            .top(px_of(top))
            .left(name_left)
            .right(px_of(st.padding.right))
            .font(font(text_font))
            .text_size(px_of(text_font.size))
            .line_height(font_height(text_font))
            .text_color(color(cx, ColorRef::Palette(fg)))
            .whitespace_nowrap()
            .overflow_hidden()
            .text_ellipsis()
            .child(text)
    };
    let progress = progress(upload.uploaded, upload.size);
    let radial_fg = color(cx, ColorRef::Palette(ColorId::HistoryFileOutRadialFg));
    #[expect(clippy::cast_possible_truncation, reason = "within a full turn")]
    let state = RadialState {
        shown: 1.0,
        arc_from: QUARTER_ARC,
        arc_length: -(FULL_ARC * progress).round() as i64,
    };
    let line_width = px_of(MSG_FILE_RADIAL_LINE);
    let on_cancel = upload.on_cancel;
    let status = progress_text(strings(cx), upload.uploaded, upload.size);
    div()
        .relative()
        .ml(-px_of(MSG_PADDING.left))
        .mr(-px_of(MSG_PADDING.right))
        .when(!has_header, |row| row.mt(-px_of(MSG_PADDING.top)))
        .min_w(px_of(MSG_FILE_MIN_WIDTH))
        .h(px_of(st.padding.top) + thumb + px_of(st.padding.bottom))
        .child(
            div()
                .id(("upload-cancel", id))
                .absolute()
                .left(px_of(st.padding.left))
                .top(px_of(st.padding.top))
                .size(thumb)
                .rounded_full()
                .bg(color(cx, ColorRef::Palette(ColorId::MsgFileOutBg)))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(ui::icon(HISTORY_FILE_OUT_CANCEL))
                .child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), window, _| {
                            // The arc runs inside the circle by its width (`msgFileRadialLine`).
                            let arc = bounds.dilate(-line_width - line_width / 2.0);
                            paint_arc(window, arc, state, line_width, radial_fg);
                        },
                    )
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .size_full(),
                )
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_cancel(event, window, cx);
                }),
        )
        .child(line(
            upload.name,
            st.name_top,
            SEMIBOLD_FONT,
            ColorId::HistoryFileNameOutFg,
        ))
        .child(line(
            status,
            st.status_top,
            NORMAL_FONT,
            ColorId::MediaOutFg,
        ))
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
}
