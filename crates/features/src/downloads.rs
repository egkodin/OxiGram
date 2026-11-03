//! The downloads section (`Info::Downloads`): TDLib's download list, newest
//! first, each file in the shared files layout (`Overview::Layout::Document`
//! with `forceFileLayout`): the colored square with the extension, the
//! name, the size or the bytes in, and the date that opens the message.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, canvas, div, relative,
};
use i18n::key;
use model::Event;
use model::downloads::{Change, Entry};
use model::file::File;
use model::message::Content;
use model::{ChatId, FileId, MessageId};
use style::basic::{NORMAL_FONT, ROUND_RADIUS_SMALL, SEMIBOLD_FONT};
use style::chat::MSG_FILE_RADIAL_LINE;
use style::chat_style::{HISTORY_FILE_THUMB_CANCEL, HISTORY_FILE_THUMB_DOWNLOAD};
use style::info::{
    INFO_EMPTY_FILE, INFO_EMPTY_ICON_TOP, INFO_EMPTY_LABEL, INFO_EMPTY_LABEL_TOP,
    INFO_MEDIA_HEADER_POSITION, INFO_MEDIA_MARGIN, INFO_TOP_BAR_BACK, INFO_TOP_BAR_HEIGHT,
    INFO_TOP_BAR_TITLE,
};
use style::overview::{
    LINKS_BORDER, LINKS_BORDER_FG, OVERVIEW_FILE_EXT_FG, OVERVIEW_FILE_EXT_FONT,
    OVERVIEW_FILE_EXT_PADDING, OVERVIEW_FILE_EXT_TOP, OVERVIEW_FILE_LAYOUT,
};
use style::{ColorId, ColorRef, Font};
use ui::{IconButton, RadialState, color, font, font_height, icon, icon_size, paint_arc};

use crate::dialogs::content_preview;
use crate::file_sizes::{progress_text, size_text};
use crate::history::layout;
use crate::lang::{strings, tr, tr_with};
use crate::store::spawn;
use crate::{Layers, Store, px_of};

/// Qt's arc units: a sixteenth of a degree.
const FULL_ARC: f32 = 360.0 * 16.0;
const QUARTER_ARC: i64 = 90 * 16;
/// Where tdesktop puts the title right of the back button.
const TITLE_TOP: f32 = 17.0;
/// A clicked file loads before the rest (`CLICKED_PRIORITY` in the history).
const CLICKED_PRIORITY: i32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DownloadsEvent {
    Close,
    OpenMessage(ChatId, MessageId),
}

pub(crate) struct DownloadsView {
    store: Entity<Store>,
    layers: Entity<Layers>,
    /// The last file the bar saw added; a new one reloads the list.
    latest: Option<FileId>,
    _subscription: Subscription,
}

impl EventEmitter<DownloadsEvent> for DownloadsView {}

impl DownloadsView {
    pub(crate) fn new(
        store: Entity<Store>,
        layers: Entity<Layers>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&store, |this: &mut Self, store, cx| {
            let latest = latest(&store, cx);
            if latest != this.latest {
                this.latest = latest;
                this.reload(cx);
            }
            cx.notify();
        });
        let mut view = Self {
            latest: latest(&store, cx),
            store,
            layers,
            _subscription: subscription,
        };
        view.reload(cx);
        view
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = spawn(self.store.read(cx).session().file_downloads(), cx);
        let store = self.store.downgrade();
        cx.spawn(async move |_, cx| match task.await {
            Ok(entries) => {
                let listed = Event::Downloads(Change::Listed(entries));
                let _ = store.update(cx, |store, cx| store.apply(listed, cx));
            }
            Err(error) => tracing::warn!(%error, "loading the downloads list failed"),
        })
        .detach();
    }

    /// What a click on a file's square or name does.
    fn press(&mut self, row: &Row, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        match (&row.path, row.progress) {
            (Some(path), _) => crate::launch::open(path, false, &self.layers, window, cx),
            // shortcut: cancels like the history, not `toggleDownloadIsPaused`; with the pause button.
            (None, Some(_)) => {
                let task = session.cancel_download_file(row.file);
                crate::peer_menu::send(task, "cancelling a download", cx);
            }
            (None, None) => {
                let task = session.add_file_to_downloads(
                    row.file,
                    row.chat,
                    row.message,
                    CLICKED_PRIORITY,
                );
                crate::peer_menu::send(task, "resuming a download", cx);
            }
        }
    }
}

fn latest(store: &Entity<Store>, cx: &App) -> Option<FileId> {
    store
        .read(cx)
        .state()
        .downloads
        .latest
        .as_ref()
        .map(|latest| latest.file)
}

/// One file's row, out of its message and the file's state now.
#[derive(Debug, Clone)]
struct Row {
    file: FileId,
    chat: ChatId,
    message: MessageId,
    name: SharedString,
    status: String,
    date: String,
    generic: Generic,
    /// Set while it loads.
    progress: Option<f32>,
    path: Option<PathBuf>,
}

/// The file's name and file, for the media the list keeps.
fn named_file(content: &Content) -> Option<(&str, &File)> {
    match content {
        Content::Document { name, media } => Some((name.as_str(), media.file.as_ref()?)),
        Content::Audio { name, music, .. } => Some((name.as_str(), &music.file)),
        Content::Photo(media) | Content::Video(media) | Content::Animation(media) => {
            Some(("", media.file.as_ref()?))
        }
        Content::VideoNote { media, .. } => Some(("", media.file.as_ref()?)),
        _ => None,
    }
}

fn row(entry: &Entry, state: &model::State, cx: &App) -> Option<Row> {
    let (name, sent) = named_file(&entry.message.content)?;
    let file = state.chats.file(sent.id).unwrap_or(sent);
    let strings = strings(cx);
    let name = if name.is_empty() {
        content_preview(strings, &entry.message.content).0.text
    } else {
        name.to_owned()
    };
    let is_loading = file.path.is_none() && file.is_downloading;
    #[expect(clippy::cast_precision_loss, reason = "a share of bytes")]
    let progress = is_loading.then(|| {
        if file.size > 0 {
            file.downloaded as f32 / file.size as f32
        } else {
            0.0
        }
    });
    let status = if is_loading {
        progress_text(strings, file.downloaded, file.size)
    } else {
        size_text(file.size)
    };
    Some(Row {
        file: file.id,
        chat: entry.message.chat,
        message: entry.message.id,
        generic: Generic::of(&name),
        name: name.into(),
        status,
        date: date(entry.added, cx),
        progress,
        path: file.path.clone().map(PathBuf::from),
    })
}

/// "Oct 28 at 10:00" (`langDateTime`).
fn date(unix_time: i32, cx: &App) -> String {
    let now = chrono::Local::now();
    let day = layout::day(unix_time, &chrono::Local).unwrap_or_else(|| now.date_naive());
    tr_with(
        cx,
        key::lng_mediaview_date_time,
        &[
            ("date", &layout::date_badge(strings(cx), day, &now)),
            ("time", &layout::time(unix_time, &chrono::Local)),
        ],
    )
}

/// A file's square without a thumbnail (`DocumentGenericPreview`): one of
/// four colors by its type, and the extension.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Generic {
    color: ColorId,
    dark: ColorId,
    ext: String,
}

impl Generic {
    fn of(name: &str) -> Self {
        let name = name.to_lowercase();
        let (color, dark) = match color_index(&name) {
            0 => (ColorId::MsgFile1Bg, ColorId::MsgFile1BgDark),
            1 => (ColorId::MsgFile2Bg, ColorId::MsgFile2BgDark),
            2 => (ColorId::MsgFile3Bg, ColorId::MsgFile3BgDark),
            _ => (ColorId::MsgFile4Bg, ColorId::MsgFile4BgDark),
        };
        let ext = match name.rfind('.') {
            Some(dot) if dot + 1 < name.len() => name[dot + 1..].to_owned(),
            _ => name.clone(),
        };
        Self { color, dark, ext }
    }
}

/// `ColorIndexForName`: documents blue, sheets green, slides red,
/// archives and media yellow, the rest by the extension's first letter.
// shortcut: ignores the MIME type (text/* is blue); with MIME types in the model.
fn color_index(name: &str) -> u32 {
    let ends = |exts: &[&str]| exts.iter().any(|ext| name.ends_with(ext));
    if ends(&[".doc", ".docx", ".txt", ".psd"]) {
        return 0;
    }
    if ends(&[".xls", ".xlsx", ".csv"]) {
        return 1;
    }
    if ends(&[".pdf", ".ppt", ".pptx", ".key"]) {
        return 2;
    }
    if ends(&[".zip", ".rar", ".ai", ".mp3", ".mov", ".avi"]) {
        return 3;
    }
    let letter = match name.rfind('.') {
        Some(dot) if dot + 1 < name.len() => name[dot + 1..].chars().next(),
        _ => name.chars().next(),
    };
    u32::from(letter.unwrap_or('0')) % 4
}

type Press = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

impl DownloadsView {
    fn top_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        let title = &INFO_TOP_BAR_TITLE;
        div()
            .relative()
            .flex_none()
            .h(px_of(INFO_TOP_BAR_HEIGHT))
            .border_b_1()
            .border_color(color(cx, ColorRef::Palette(ColorId::ShadowFg)))
            .child(
                IconButton::new("downloads-back", &INFO_TOP_BAR_BACK).on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |_, cx| cx.emit(DownloadsEvent::Close));
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(px_of(INFO_TOP_BAR_BACK.width))
                    .top(gpui::px(TITLE_TOP))
                    .font(font(title.style.font))
                    .text_size(px_of(title.style.font.size))
                    .line_height(font_height(title.style.font))
                    .text_color(color(cx, title.text_fg))
                    .child(tr(cx, key::lng_downloads_section)),
            )
            .into_any_element()
    }

    fn row_element(&self, index: usize, row: Row, cx: &mut Context<Self>) -> AnyElement {
        let layout = &OVERVIEW_FILE_LAYOUT;
        let border = px_of(LINKS_BORDER);
        let thumb = px_of(layout.file_thumb_size);
        let name_left = thumb + px_of(layout.file_padding.right);
        let height =
            border + px_of(layout.file_padding.top) + thumb + px_of(layout.file_padding.bottom);
        let weak = cx.weak_entity();
        let pressed = row.clone();
        let press: Press = Rc::new(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.press(&pressed, window, cx));
        });
        let fg = color(cx, ColorRef::Palette(ColorId::MediaInFg));
        let line = |text_font: Font, top| {
            div()
                .absolute()
                .left(name_left)
                .top(border + px_of(top))
                .font(font(text_font))
                .text_size(px_of(text_font.size))
                .line_height(font_height(text_font))
        };
        let open = {
            let weak = cx.weak_entity();
            let (chat, message) = (row.chat, row.message);
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |_, cx| {
                    cx.emit(DownloadsEvent::OpenMessage(chat, message));
                });
            }
        };
        let on_name = press.clone();
        div()
            .relative()
            .h(height)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(name_left)
                    .right_0()
                    .h(border)
                    .when(index > 0, |this| this.bg(color(cx, LINKS_BORDER_FG))),
            )
            .child(square(index, &row, press, cx))
            .child(
                line(SEMIBOLD_FONT, layout.file_name_top)
                    .id(("download-name", index))
                    .right_0()
                    .truncate()
                    .cursor_pointer()
                    .text_color(color(cx, ColorRef::Palette(ColorId::HistoryFileNameInFg)))
                    .on_click(move |event, window, cx| on_name(event, window, cx))
                    .child(row.name.clone()),
            )
            .child(
                line(NORMAL_FONT, layout.file_status_top)
                    .right_0()
                    .truncate()
                    .text_color(fg)
                    .child(row.status.clone()),
            )
            .child(
                line(NORMAL_FONT, layout.file_date_top)
                    .id(("download-date", index))
                    .cursor_pointer()
                    .text_color(fg)
                    .hover(|this| this.underline())
                    .on_click(open)
                    .child(row.date),
            )
            .into_any_element()
    }
}

/// The colored square: the extension once on disk, else the circle that
/// downloads or cancels with the arc of the progress.
fn square(index: usize, row: &Row, press: Press, cx: &App) -> AnyElement {
    let layout = &OVERVIEW_FILE_LAYOUT;
    let size = px_of(layout.file_thumb_size);
    let base = div()
        .id(("download-square", index))
        .absolute()
        .left_0()
        .top(px_of(LINKS_BORDER) + px_of(layout.file_padding.top))
        .size(size)
        .rounded(px_of(ROUND_RADIUS_SMALL))
        .bg(color(cx, ColorRef::Palette(row.generic.color)))
        .cursor_pointer()
        .on_click(move |event, window, cx| press(event, window, cx));
    if row.path.is_some() {
        let ext_font = OVERVIEW_FILE_EXT_FONT;
        let padding = px_of(OVERVIEW_FILE_EXT_PADDING);
        return base
            .child(
                div()
                    .absolute()
                    .top(px_of(OVERVIEW_FILE_EXT_TOP))
                    .left(padding)
                    .right(padding)
                    .flex()
                    .justify_center()
                    .font(font(ext_font))
                    .text_size(px_of(ext_font.size))
                    .line_height(font_height(ext_font))
                    .text_color(color(cx, OVERVIEW_FILE_EXT_FG))
                    .child(div().truncate().child(row.generic.ext.clone())),
            )
            .into_any_element();
    }
    let circle = px_of(layout.song_thumb_size);
    let line = px_of(MSG_FILE_RADIAL_LINE);
    let fg = color(cx, ColorRef::Palette(ColorId::HistoryFileThumbRadialFg));
    let glyph = if row.progress.is_some() {
        HISTORY_FILE_THUMB_CANCEL
    } else {
        HISTORY_FILE_THUMB_DOWNLOAD
    };
    let progress = row.progress;
    base.flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .relative()
                .size(circle)
                .rounded_full()
                .bg(color(cx, ColorRef::Palette(row.generic.dark)))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(glyph))
                .when_some(progress, |this, progress| {
                    #[expect(clippy::cast_possible_truncation, reason = "within a full turn")]
                    let state = RadialState {
                        shown: 1.0,
                        arc_from: QUARTER_ARC,
                        arc_length: -(FULL_ARC * progress.max(0.02)).round() as i64,
                    };
                    this.child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, (), window, _| {
                                let arc = bounds.dilate(-line - line / 2.0);
                                paint_arc(window, arc, state, line, fg);
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                }),
        )
        .into_any_element()
}

impl Render for DownloadsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.store.read(cx).state();
        let rows: Option<Vec<Row>> = state.downloads.list.as_ref().map(|entries| {
            entries
                .iter()
                .filter_map(|entry| row(entry, state, cx))
                .collect()
        });
        let body = match rows {
            None => div().flex_1().into_any_element(),
            Some(rows) if rows.is_empty() => empty(cx),
            Some(rows) => {
                let elements: Vec<AnyElement> = rows
                    .into_iter()
                    .enumerate()
                    .map(|(index, row)| self.row_element(index, row, cx))
                    .collect();
                div()
                    .id("downloads-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pt(px_of(INFO_MEDIA_MARGIN.top))
                    .pb(px_of(INFO_MEDIA_MARGIN.bottom))
                    .px(px_of(INFO_MEDIA_HEADER_POSITION.x))
                    .children(elements)
                    .into_any_element()
            }
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(color(cx, ColorRef::Palette(ColorId::WindowBg)))
            .child(self.top_bar(cx))
            .child(body)
    }
}

/// "No files here yet" under the file icon, its center a third down.
fn empty(cx: &App) -> AnyElement {
    let label = &INFO_EMPTY_LABEL;
    let icon_height = icon_size(INFO_EMPTY_FILE).height;
    div()
        .relative()
        .flex_1()
        .child(
            div()
                .absolute()
                .top(relative(1.0 / 3.0))
                .left_0()
                .right_0()
                .mt(-icon_height / 2.0)
                .flex()
                .flex_col()
                .items_center()
                .child(icon(INFO_EMPTY_FILE))
                .child(
                    div()
                        .mt(px_of(INFO_EMPTY_ICON_TOP) - px_of(INFO_EMPTY_LABEL_TOP) - icon_height)
                        .font(font(label.style.font))
                        .text_size(px_of(label.style.font.size))
                        .line_height(font_height(label.style.font))
                        .text_color(color(cx, label.text_fg))
                        .child(tr(cx, key::lng_media_file_empty)),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_takes_tdesktops_color_and_extension() {
        assert_eq!(Generic::of("Report.PDF").color, ColorId::MsgFile3Bg);
        assert_eq!(Generic::of("Report.PDF").ext, "pdf");
        assert_eq!(
            Generic::of("saturday-photos.zip").color,
            ColorId::MsgFile4Bg
        );
        assert_eq!(Generic::of("notes.txt").color, ColorId::MsgFile1Bg);
        // 'g' is 103, so the fourth color... 103 % 4 = 3.
        assert_eq!(color_index("archive.gz"), 3);
        assert_eq!(Generic::of("README").ext, "readme");
        assert_eq!(Generic::of("dot.").ext, "dot.");
    }
}
