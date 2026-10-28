//! The bar under the chat list while files download (`Ui::DownloadBar`):
//! the ring of their progress, the file's name or "3 files", the bytes in,
//! then "View in chat" once done.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, canvas, div,
};
use i18n::key;
use model::downloads::Downloads;
use model::{ChatId, MessageId};
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::dialogs::{
    DOWNLOAD_ARROW, DOWNLOAD_ARROW_OVER, DOWNLOAD_ARROW_RIGHT, DOWNLOAD_BAR_HEIGHT,
    DOWNLOAD_ICON_DOCUMENT, DOWNLOAD_ICON_SIZE, DOWNLOAD_ICON_SIZE_DONE, DOWNLOAD_INFO_LEFT,
    DOWNLOAD_INFO_TOP, DOWNLOAD_LOADING_LEFT, DOWNLOAD_LOADING_LINE, DOWNLOAD_LOADING_SIZE,
    DOWNLOAD_TITLE_LEFT, DOWNLOAD_TITLE_TOP,
};
use style::{ColorId, ColorRef, Font};
use ui::{RadialState, color, font, font_height, icon, icon_size, paint_arc};

use super::{Dialogs, OpenMessage};
use crate::file_sizes::progress_text;
use crate::lang::{strings, tr, tr_plural};
use crate::px_of;

/// Qt's arc units: a sixteenth of a degree.
const FULL_ARC: f32 = 360.0 * 16.0;
const QUARTER_ARC: i64 = 90 * 16;

/// What the bar shows, out of TDLib's totals.
#[derive(Debug, Clone, PartialEq)]
struct Content {
    title: SharedString,
    is_bold: bool,
    info: SharedString,
    is_done: bool,
    progress: f32,
    /// The one file's message, which a click opens.
    single: Option<(ChatId, MessageId)>,
}

fn content(downloads: &Downloads, cx: &App) -> Option<Content> {
    let totals = downloads.totals;
    if totals.total_count <= 0 {
        return None;
    }
    let single = downloads
        .latest
        .as_ref()
        .filter(|_| totals.total_count == 1);
    let (title, is_bold) = match single.map(|latest| latest.name.as_str()) {
        Some(name) if !name.is_empty() => (SharedString::from(name.to_owned()), false),
        _ => {
            let count = totals.total_count.to_string();
            let files = tr_plural(
                cx,
                key::lng_profile_files,
                i64::from(totals.total_count),
                &[("count", &count)],
            );
            (SharedString::from(files), true)
        }
    };
    let is_done = totals.is_done();
    let info = if is_done {
        tr(
            cx,
            if single.is_some() {
                key::lng_downloads_view_in_chat
            } else {
                key::lng_downloads_view_in_section
            },
        )
    } else {
        progress_text(strings(cx), totals.downloaded_size, totals.total_size).into()
    };
    #[expect(clippy::cast_precision_loss, reason = "a share of bytes")]
    let progress = if totals.total_size > 0 {
        totals.downloaded_size as f32 / totals.total_size as f32
    } else {
        0.0
    };
    Some(Content {
        title,
        is_bold,
        info,
        is_done,
        progress,
        single: single.map(|latest| (latest.chat, latest.message)),
    })
}

impl Dialogs {
    /// The bar, while TDLib counts downloads.
    // shortcut: a still ring and no fade or grow when done; with M8 polish. A file's thumbnail does not replace the icon.
    pub(super) fn download_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let downloads = &self.store.read(cx).state().downloads;
        let content = content(downloads, cx)?;
        let arrow_room = px_of(DOWNLOAD_ARROW_RIGHT) + icon_size(DOWNLOAD_ARROW).width;
        let single = content.single;
        let line = |text: SharedString, text_font: Font, fg: ColorId, top| {
            div()
                .absolute()
                .top(px_of(top))
                .left(px_of(DOWNLOAD_TITLE_LEFT))
                .right(arrow_room)
                .font(font(text_font))
                .text_size(px_of(text_font.size))
                .line_height(font_height(text_font))
                .text_color(color(cx, ColorRef::Palette(fg)))
                .truncate()
                .child(text)
        };
        let title_font = if content.is_bold {
            SEMIBOLD_FONT
        } else {
            NORMAL_FONT
        };
        let info_fg = if content.is_done {
            ColorId::WindowActiveTextFg
        } else {
            ColorId::WindowSubTextFg
        };
        let info = line(
            content.info.clone(),
            NORMAL_FONT,
            info_fg,
            DOWNLOAD_INFO_TOP,
        )
        .left(px_of(DOWNLOAD_INFO_LEFT));
        Some(
            div()
                .id("download-bar")
                .group("download-bar")
                .relative()
                .flex_none()
                .w_full()
                .h(px_of(DOWNLOAD_BAR_HEIGHT))
                .bg(color(cx, ColorRef::Palette(ColorId::WindowBg)))
                .border_t_1()
                .border_color(color(cx, ColorRef::Palette(ColorId::ShadowFg)))
                .cursor_pointer()
                .child(ring(&content, cx))
                .child(line(
                    content.title.clone(),
                    title_font,
                    ColorId::WindowBoldFg,
                    DOWNLOAD_TITLE_TOP,
                ))
                .child(info)
                .child(arrow())
                .on_click(cx.listener(move |_, _, _, cx| {
                    // shortcut: several files open nothing until the downloads section lands.
                    if let Some((chat, message)) = single {
                        cx.emit(OpenMessage(chat, message));
                    }
                }))
                .into_any_element(),
        )
    }
}

/// The circle with the file icon, and the ring of the progress around it;
/// done, the circle grows into the ring.
fn ring(content: &Content, cx: &App) -> AnyElement {
    let size = px_of(DOWNLOAD_LOADING_SIZE);
    let line = px_of(DOWNLOAD_LOADING_LINE);
    let added = line * 3.0;
    let full = size + added * 2.0;
    let fg = color(cx, ColorRef::Palette(ColorId::WindowBgActive));
    let shift = if content.is_done { line } else { added };
    #[expect(clippy::cast_possible_truncation, reason = "within a full turn")]
    let state = RadialState {
        shown: 1.0,
        arc_from: QUARTER_ARC,
        arc_length: -(FULL_ARC * content.progress).round() as i64,
    };
    let is_done = content.is_done;
    let icon_side = px_of(if is_done {
        DOWNLOAD_ICON_SIZE_DONE
    } else {
        DOWNLOAD_ICON_SIZE
    });
    div()
        .absolute()
        .left(px_of(DOWNLOAD_LOADING_LEFT) - added)
        .top((px_of(DOWNLOAD_BAR_HEIGHT) - full) / 2.0)
        .size(full)
        .flex()
        .items_center()
        .justify_center()
        .when(!is_done, |this| {
            this.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        paint_arc(window, bounds.dilate(-added / 2.0), state, line, fg);
                    },
                )
                .absolute()
                .inset_0(),
            )
        })
        .child(
            div()
                .size(full - shift * 2.0)
                .rounded_full()
                .bg(fg)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(DOWNLOAD_ICON_DOCUMENT).side(icon_side)),
        )
        .into_any_element()
}

/// The arrow at the right, lit under the mouse.
fn arrow() -> AnyElement {
    div()
        .absolute()
        .right(px_of(DOWNLOAD_ARROW_RIGHT))
        .top_0()
        .bottom_0()
        .flex()
        .items_center()
        .child(
            div()
                .group_hover("download-bar", |this| this.invisible())
                .child(icon(DOWNLOAD_ARROW)),
        )
        .child(
            div()
                .absolute()
                .invisible()
                .group_hover("download-bar", |this| this.visible())
                .child(icon(DOWNLOAD_ARROW_OVER)),
        )
        .into_any_element()
}
