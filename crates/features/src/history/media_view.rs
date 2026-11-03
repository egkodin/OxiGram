//! The media viewer (`Media::View::OverlayWidget`): a chat's photo or video
//! over the whole window, paging through the ones loaded around it, with
//! the sender, the date and the caption; photos zoom and drag at will,
//! videos play with the controller under them (`media_video`).

use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use chrono::{DateTime, Days, Local};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, ImageSource,
    InteractiveElement as _, IntoElement, KeyBinding, MouseButton, ParentElement as _, Pixels,
    Point, Render, ScrollEvent, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, actions, div, img, px,
};
use i18n::key;
use model::file::{Dimensions, File};
use model::message::{Content, Message};
use model::{FileId, MessageId};
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::media_view::{
    MEDIAVIEW_CAPTION_MARGIN, MEDIAVIEW_CAPTION_PADDING, MEDIAVIEW_CAPTION_RADIUS,
    MEDIAVIEW_CONTROL_SIZE, MEDIAVIEW_CONTROLLER_SIZE, MEDIAVIEW_HEADER_TOP, MEDIAVIEW_ICON_OVER,
    MEDIAVIEW_ICON_SIZE, MEDIAVIEW_LEFT, MEDIAVIEW_RIGHT, MEDIAVIEW_SAVE, MEDIAVIEW_TEXT_LEFT,
    MEDIAVIEW_TEXT_SKIP_HALF, MEDIAVIEW_TEXT_TOP,
};
use style::{ColorId, ColorRef, Font};
use ui::{color, font, font_height, icon};

use super::HistoryView;
use super::bubble::OnClick;
use super::layout;
use super::media_video::{Ending, VideoPlayback};
use super::menu::Item;
use crate::lang::{strings, tr_with};
use crate::peer_menu::send;
use crate::px_of;
use crate::store::Store;

actions!(
    media_view,
    [Previous, Next, ZoomIn, ZoomOut, ZoomReset, PlayPause]
);

const CONTEXT: &str = "MediaView";
/// `kMaxZoomLevel`: eight times either way.
const MAX_ZOOM: i32 = 7;
/// Download priority of the photo on screen, above the history's.
const PRIORITY: i32 = 32;

pub(crate) fn bind_keys(cx: &mut App) {
    let (zoom_in, zoom_out, reset) = if cfg!(target_os = "macos") {
        ("cmd-=", "cmd--", "cmd-0")
    } else {
        ("ctrl-=", "ctrl--", "ctrl-0")
    };
    cx.bind_keys([
        KeyBinding::new("left", Previous, Some(CONTEXT)),
        KeyBinding::new("right", Next, Some(CONTEXT)),
        KeyBinding::new(zoom_in, ZoomIn, Some(CONTEXT)),
        KeyBinding::new(zoom_out, ZoomOut, Some(CONTEXT)),
        KeyBinding::new(reset, ZoomReset, Some(CONTEXT)),
        KeyBinding::new("space", PlayPause, Some(CONTEXT)),
    ]);
}

/// What an entry shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Photo,
    /// Plays from `start` (a link's or forward's start time).
    Video {
        start: Duration,
        duration: Duration,
    },
}

/// One photo or video the viewer pages through.
pub(crate) struct Entry {
    pub message: MessageId,
    pub kind: Kind,
    /// The photo, or the video file itself.
    pub file: Option<File>,
    /// A video's still until it plays.
    pub still: Option<File>,
    pub dimensions: Option<Dimensions>,
    pub caption: String,
    /// "Maria Garcia", or nothing in a channel without signatures.
    pub sender: String,
    /// "today at 10:00".
    pub date: String,
    /// Whether the chat lets the photo be saved.
    pub is_saveable: bool,
}

impl Entry {
    /// The entry for a photo or video message.
    pub(crate) fn of(message: &Message, sender: String, date: String) -> Option<Self> {
        let (media, kind) = match &message.content {
            Content::Photo(media) => (media, Kind::Photo),
            Content::Video(media) => (
                media,
                Kind::Video {
                    start: Duration::from_secs(u64::try_from(media.start_timestamp).unwrap_or(0)),
                    duration: Duration::from_secs(u64::try_from(media.duration).unwrap_or(0)),
                },
            ),
            _ => return None,
        };
        Some(Self {
            message: message.id,
            kind,
            file: media.file.clone(),
            still: media.preview.clone(),
            dimensions: media.dimensions,
            caption: media.caption.text.clone(),
            sender,
            date,
            is_saveable: message.can_be_saved && message.self_destruct.is_none(),
        })
    }
}

/// What the viewer asks of the history.
pub(crate) enum MediaViewEvent {
    /// "Save As..." for the photo of this message.
    Save(MessageId),
}

/// How big the photo shows (`_zoom`): fitted to the window, or a level,
/// 0 being the photo's own size; at first, fitted only when it is bigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zoom {
    Default,
    Fit,
    Level(i32),
}

impl Zoom {
    /// The zoom `Default` stands for, given the fitted level `full`.
    fn resolve(self, full: f32) -> Self {
        match self {
            Self::Default if full < 0.0 => Self::Fit,
            Self::Default => Self::Level(0),
            zoom => zoom,
        }
    }
}

/// The scale of a zoom level (`z + 1`, or `1 / (1 - z)` below zero).
fn scale(level: f32) -> f32 {
    if level >= 0.0 {
        level + 1.0
    } else {
        1.0 / (1.0 - level)
    }
}

/// The level that fits `size` into `room` (`countZoomFor`).
fn fit_level(size: (f32, f32), room: (f32, f32)) -> f32 {
    let mut result = room.0 / size.0;
    if size.1 * result > room.1 {
        result = room.1 / size.1;
    }
    if result >= 1.0 {
        result - 1.0
    } else {
        1.0 - 1.0 / result
    }
}

/// The next level in (`zoomIn`), from the fitted level `full`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "zoom levels are small"
)]
fn zoom_in(zoom: Zoom, full: f32) -> Zoom {
    let past =
        |level: i32| level as f32 + 1.0 > full || (full > MAX_ZOOM as f32 && level == MAX_ZOOM);
    match zoom.resolve(full) {
        Zoom::Fit if full.ceil() as i32 <= MAX_ZOOM => Zoom::Level(full.ceil() as i32),
        Zoom::Level(level) if (level as f32) < full && past(level) => Zoom::Fit,
        Zoom::Level(level) => Zoom::Level((level + 1).min(MAX_ZOOM)),
        zoom => zoom,
    }
}

/// The next level out (`zoomOut`).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "zoom levels are small"
)]
fn zoom_out(zoom: Zoom, full: f32) -> Zoom {
    let past =
        |level: i32| level as f32 - 1.0 < full || (full < -MAX_ZOOM as f32 && level == -MAX_ZOOM);
    match zoom.resolve(full) {
        Zoom::Fit if full.floor() as i32 >= -MAX_ZOOM => Zoom::Level(full.floor() as i32),
        Zoom::Level(level) if (level as f32) > full && past(level) => Zoom::Fit,
        Zoom::Level(level) => Zoom::Level((level - 1).max(-MAX_ZOOM)),
        zoom => zoom,
    }
}

pub(crate) struct MediaView {
    store: Entity<Store>,
    entries: Vec<Entry>,
    index: usize,
    zoom: Zoom,
    /// How far the photo was dragged from the middle.
    offset: Point<Pixels>,
    /// Where the drag started, and the offset then.
    drag: Option<(Point<Pixels>, Point<Pixels>)>,
    requested: HashSet<FileId>,
    /// The video on screen once its file is on disk.
    playback: Option<Entity<VideoPlayback>>,
    focus: FocusHandle,
    _store: Subscription,
}

impl EventEmitter<MediaViewEvent> for MediaView {}

impl Focusable for MediaView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl MediaView {
    pub(super) fn new(
        store: Entity<Store>,
        entries: Vec<Entry>,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        let mut view = Self {
            store,
            entries,
            index,
            zoom: Zoom::Default,
            offset: Point::default(),
            drag: None,
            requested: HashSet::new(),
            playback: None,
            focus: cx.focus_handle(),
            _store: subscription,
        };
        view.show(index, cx);
        view
    }

    /// Pages to `index`, at the fitted size, downloading its photo.
    fn show(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        self.index = index;
        self.zoom = Zoom::Default;
        self.offset = Point::default();
        self.playback = None;
        if let Some(file) = entry
            .file
            .as_ref()
            .filter(|file| self.requested.insert(file.id))
        {
            let task = self
                .store
                .read(cx)
                .session()
                .download_file(file.id, PRIORITY);
            let task = crate::store::spawn(task, cx);
            cx.spawn(async move |_, _| {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "downloading a photo for the viewer failed");
                }
            })
            .detach();
        }
        cx.notify();
    }

    fn page(&mut self, forward: bool, cx: &mut Context<Self>) {
        let next = if forward {
            self.index.checked_add(1)
        } else {
            self.index.checked_sub(1)
        };
        if let Some(next) = next.filter(|next| *next < self.entries.len()) {
            self.show(next, cx);
        }
    }

    /// The photo's path once on disk, or a video's still.
    fn source(&self, cx: &App) -> Option<ImageSource> {
        let entry = self.entries.get(self.index)?;
        let file = match entry.kind {
            Kind::Photo => entry.file.as_ref()?,
            Kind::Video { .. } => entry.still.as_ref()?,
        };
        Some(ImageSource::from(self.path(file, cx)?))
    }

    fn path(&self, file: &File, cx: &App) -> Option<PathBuf> {
        let chats = &self.store.read(cx).state().chats;
        chats
            .file(file.id)
            .unwrap_or(file)
            .path
            .clone()
            .map(PathBuf::from)
    }

    /// The video's playback, started once its file is on disk, decoding
    /// frames no bigger than `size` in window pixels.
    fn playback(
        &mut self,
        size: (f32, f32),
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<VideoPlayback>> {
        if self.playback.is_none() {
            let entry = self.entries.get(self.index)?;
            let Kind::Video { start, duration } = entry.kind else {
                return None;
            };
            let path = self.path(entry.file.as_ref()?, cx)?;
            let scale = window.scale_factor();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a positive size in pixels"
            )]
            let pixels = |side: f32| (side * scale).ceil().max(1.0) as u32;
            let frames = (pixels(size.0), pixels(size.1));
            self.playback =
                Some(cx.new(|cx| {
                    VideoPlayback::new(&path, start, frames, duration, Ending::Pause, cx)
                }));
        }
        self.playback.clone()
    }

    fn play_pause(&mut self, cx: &mut Context<Self>) {
        if let Some(playback) = &self.playback {
            playback.update(cx, |playback, cx| playback.toggle(cx));
        }
    }

    /// The photo's size, a point per pixel as tdesktop shows it, and the
    /// level that fits the window.
    fn size(&self, window: &Window) -> Option<((f32, f32), f32)> {
        let dimensions = self.entries.get(self.index)?.dimensions?;
        #[expect(clippy::cast_precision_loss, reason = "photo sides")]
        let size = (dimensions.width as f32, dimensions.height as f32);
        let viewport = window.viewport_size();
        let room = (f32::from(viewport.width), f32::from(viewport.height));
        Some((size, fit_level(size, room)))
    }

    fn zoom(&mut self, change: fn(Zoom, f32) -> Zoom, window: &Window, cx: &mut Context<Self>) {
        if let Some((_, full)) = self.size(window) {
            self.zoom = change(self.zoom, full);
            cx.notify();
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !event.modifiers.secondary(cx) {
            return;
        }
        let delta = event.delta.pixel_delta(px(1.0)).y;
        if delta > px(0.0) {
            self.zoom(zoom_in, window, cx);
        } else if delta < px(0.0) {
            self.zoom(zoom_out, window, cx);
        }
    }
}

impl Render for MediaView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let video_box = self.size(window).map(|(size, full)| {
            let scale = scale(match self.zoom.resolve(full) {
                #[expect(clippy::cast_precision_loss, reason = "zoom levels are small")]
                Zoom::Level(level) => level as f32,
                Zoom::Default | Zoom::Fit => full,
            });
            (size.0 * scale, size.1 * scale)
        });
        let playback = video_box.and_then(|size| self.playback(size, window, cx));
        let photo = self.size(window).map(|(size, full)| {
            let level = match self.zoom.resolve(full) {
                #[expect(clippy::cast_precision_loss, reason = "zoom levels are small")]
                Zoom::Level(level) => level as f32,
                Zoom::Default | Zoom::Fit => full,
            };
            let scale = scale(level);
            let (width, height) = (px(size.0 * scale), px(size.1 * scale));
            let left = (viewport.width - width) / 2.0 + self.offset.x;
            let top = (viewport.height - height) / 2.0 + self.offset.y;
            div()
                .id("media-view-photo")
                .absolute()
                .left(left)
                .top(top)
                .w(width)
                .h(height)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &gpui::MouseDownEvent, _, _| {
                        this.drag = Some((event.position, this.offset));
                    }),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(match &playback {
                    Some(playback) => Some(playback.clone().into_any_element()),
                    None => self
                        .source(cx)
                        .map(|source| img(source).size_full().into_any_element()),
                })
        });
        let controller = playback.as_ref().map(|playback| {
            div()
                .absolute()
                .left(px_of(MEDIAVIEW_CAPTION_PADDING.bottom))
                .right(px_of(MEDIAVIEW_CAPTION_PADDING.bottom))
                .bottom(px_of(MEDIAVIEW_CAPTION_PADDING.bottom))
                .flex()
                .justify_center()
                .child(VideoPlayback::controller(playback, cx))
        });
        let caption_lift = if playback.is_some() {
            px_of(MEDIAVIEW_CONTROLLER_SIZE.height) + px_of(MEDIAVIEW_CAPTION_PADDING.bottom)
        } else {
            px(0.0)
        };
        let entry = self.entries.get(self.index);
        let fg = color(cx, ColorRef::Palette(ColorId::MediaviewControlFg));
        div()
            .id("media-view")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(color(cx, ColorRef::Palette(ColorId::MediaviewBg)))
            .on_action(cx.listener(|this, _: &Previous, _, cx| this.page(false, cx)))
            .on_action(cx.listener(|this, _: &Next, _, cx| this.page(true, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, window, cx| this.zoom(zoom_in, window, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, window, cx| this.zoom(zoom_out, window, cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, window, cx| {
                this.zoom(reset, window, cx);
                this.offset = Point::default();
            }))
            .on_action(cx.listener(|this, _: &PlayPause, _, cx| this.play_pause(cx)))
            .on_scroll(cx.listener(Self::on_scroll))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some((start, offset)) = this.drag {
                    this.offset = offset + (event.position - start);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = None),
            )
            .children(photo)
            .children((self.index > 0).then(|| nav(Side::Left, cx)))
            .children((self.index + 1 < self.entries.len()).then(|| nav(Side::Right, cx)))
            .children(entry.map(|entry| footer(entry, self.index, self.entries.len(), fg, cx)))
            .children(
                entry
                    .filter(|entry| entry.is_saveable)
                    .map(|entry| save_button(entry.message, cx)),
            )
            .children(
                entry
                    .filter(|entry| !entry.caption.is_empty())
                    .map(|entry| caption(entry.caption.clone(), caption_lift, cx)),
            )
            .children(controller)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// The arrow strip along a side (`_leftNav`, `_rightNav`), its circle
/// showing under the mouse.
fn nav(side: Side, cx: &mut Context<MediaView>) -> impl IntoElement {
    let over = px_of(MEDIAVIEW_ICON_OVER);
    let (id, arrow, forward) = match side {
        Side::Left => ("media-view-left", MEDIAVIEW_LEFT, false),
        Side::Right => ("media-view-right", MEDIAVIEW_RIGHT, true),
    };
    let control_bg = color(cx, ColorRef::Palette(ColorId::MediaviewControlBg));
    div()
        .id(id)
        .group(id)
        .absolute()
        .top_0()
        .bottom_0()
        .when(side == Side::Left, |this| this.left_0())
        .when(side == Side::Right, |this| this.right_0())
        .w(px_of(MEDIAVIEW_CONTROL_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, _, cx| this.page(forward, cx)))
        .child(
            div()
                .size(over)
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .group_hover(id, move |this| this.bg(control_bg))
                .child(icon(arrow)),
        )
}

fn line(text: impl Into<SharedString>, line_font: Font, fg: gpui::Hsla) -> gpui::Div {
    div()
        .font(font(line_font))
        .text_size(px_of(line_font.size))
        .line_height(font_height(line_font))
        .text_color(fg)
        .whitespace_nowrap()
        .child(text.into())
}

/// "Photo 2 of 5" over "Maria Garcia • today at 10:00", bottom left.
// shortcut: the name does not open the sender's profile; with profiles in M6.
fn footer(entry: &Entry, index: usize, count: usize, fg: gpui::Hsla, cx: &App) -> impl IntoElement {
    let header = (count > 1).then(|| {
        let n = (index + 1).to_string();
        let amount = count.to_string();
        let text = tr_with(
            cx,
            key::lng_mediaview_n_of_amount,
            &[("n", &n), ("amount", &amount)],
        );
        line(text, SEMIBOLD_FONT, fg)
            .absolute()
            .left(px_of(MEDIAVIEW_TEXT_LEFT))
            .bottom(px_of(MEDIAVIEW_HEADER_TOP) - font_height(SEMIBOLD_FONT))
    });
    let name = (!entry.sender.is_empty()).then(|| line(entry.sender.clone(), SEMIBOLD_FONT, fg));
    let separator = name.is_some().then(|| line("\u{2022}", NORMAL_FONT, fg));
    let texts = div()
        .absolute()
        .left(px_of(MEDIAVIEW_TEXT_LEFT))
        .bottom(px_of(MEDIAVIEW_TEXT_TOP) - font_height(NORMAL_FONT))
        .flex()
        .gap(px_of(MEDIAVIEW_TEXT_SKIP_HALF))
        .children(name)
        .children(separator)
        .child(line(entry.date.clone(), NORMAL_FONT, fg));
    div().absolute().inset_0().children(header).child(texts)
}

/// The save button, bottom right (`_saveNav`).
// shortcut: no rotate or more menu yet; with the viewer's actions (row 295).
fn save_button(message: MessageId, cx: &mut Context<MediaView>) -> impl IntoElement {
    let control_bg = color(cx, ColorRef::Palette(ColorId::MediaviewControlBg));
    div()
        .id("media-view-save")
        .group("media-view-save")
        .absolute()
        .right_0()
        .bottom_0()
        .w(px_of(MEDIAVIEW_ICON_SIZE.width))
        .h(px_of(MEDIAVIEW_ICON_SIZE.height))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |_, _, _, cx| cx.emit(MediaViewEvent::Save(message))))
        .child(
            div()
                .size(px_of(MEDIAVIEW_ICON_OVER))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .group_hover("media-view-save", move |this| this.bg(control_bg))
                .child(icon(MEDIAVIEW_SAVE)),
        )
}

/// The caption over the bottom, in its rounded box (`_captionRect`).
// shortcut: plain text without links or entities, never cut to a quarter of the height; with rich text in the viewer.
fn caption(text: String, lift: Pixels, cx: &App) -> impl IntoElement {
    let padding = MEDIAVIEW_CAPTION_PADDING;
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px_of(MEDIAVIEW_CAPTION_MARGIN.height) + px_of(MEDIAVIEW_TEXT_TOP) + lift)
        .flex()
        .justify_center()
        .child(
            div()
                .max_w(gpui::relative(0.6))
                .px(px_of(padding.left))
                .py(px_of(padding.top))
                .rounded(px_of(MEDIAVIEW_CAPTION_RADIUS))
                .bg(color(cx, ColorRef::Palette(ColorId::MediaviewCaptionBg)))
                .font(font(NORMAL_FONT))
                .text_size(px_of(NORMAL_FONT.size))
                .line_height(font_height(NORMAL_FONT))
                .text_color(color(cx, ColorRef::Palette(ColorId::MediaviewCaptionFg)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(text),
        )
}

/// Opens the viewer on `entries[index]`, closing with Escape or a click
/// beside the photo.
pub(crate) fn open(
    store: Entity<Store>,
    layers: &Entity<crate::layers::Layers>,
    entries: Vec<Entry>,
    index: usize,
    window: &mut Window,
    cx: &mut App,
) -> Entity<MediaView> {
    let view = cx.new(|cx| MediaView::new(store, entries, index, cx));
    layers.update(cx, |layers, cx| layers.show_full(view.clone(), window, cx));
    view
}

/// "today at 10:00", "yesterday at 10:00" or "October 26 at 10:00".
fn date(unix_time: i32, now: &DateTime<Local>, cx: &App) -> String {
    let time = layout::time(unix_time, &Local);
    let Some(day) = layout::day(unix_time, &Local) else {
        return time;
    };
    let today = now.date_naive();
    let (key, date) = if day == today {
        (key::lng_mediaview_today, String::new())
    } else if today.checked_sub_days(Days::new(1)) == Some(day) {
        (key::lng_mediaview_yesterday, String::new())
    } else {
        let date = layout::date_badge(strings(cx), day, now);
        (key::lng_mediaview_date_time, date)
    };
    tr_with(cx, key, &[("date", &date), ("time", &time)])
}

impl HistoryView {
    /// The click that opens a photo message in the viewer.
    pub(super) fn on_open_photo(&self, id: MessageId, cx: &Context<Self>) -> OnClick {
        let weak = cx.weak_entity();
        Rc::new(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.open_media(id, window, cx));
        })
    }

    /// Opens the viewer on a photo or video, paging through the ones shown.
    // shortcut: only the loaded messages' media, not the chat's shared media; with the shared media search (row 292 live).
    pub(super) fn open_media(
        &mut self,
        id: MessageId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // View-once photos open alone, never by paging.
        let entries = self.entries(|message| message.self_destruct.is_none(), cx);
        let Some(index) = entries.iter().position(|entry| entry.message == id) else {
            return;
        };
        let view = open(self.store.clone(), &self.layers, entries, index, window, cx);
        cx.subscribe(&view, |this, _, event, cx| match event {
            MediaViewEvent::Save(id) => this.save_photo(*id, cx),
        })
        .detach();
    }

    /// Opens a view-once or timed photo alone and tells TDLib it was
    /// opened, which starts its end.
    pub(super) fn open_once(&mut self, id: MessageId, window: &mut Window, cx: &mut Context<Self>) {
        let entries = self.entries(|message| message.id == id, cx);
        if entries.is_empty() {
            return;
        }
        open(self.store.clone(), &self.layers, entries, 0, window, cx);
        let session = self.store.read(cx).session().clone();
        send(
            session.open_message_content(self.chat, id),
            "opening view-once media",
            cx,
        );
    }

    /// The photos shown that `pick` keeps, oldest first.
    fn entries(&self, pick: impl Fn(&Message) -> bool, cx: &App) -> Vec<Entry> {
        let now = Local::now();
        let chats = &self.store.read(cx).state().chats;
        self.shown
            .iter()
            .filter(|message| pick(message))
            .filter_map(|message| {
                let sender = super::sender_face(chats, message.sender)
                    .map_or_else(String::new, |face| face.0);
                Entry::of(message, sender, date(message.date, &now, cx))
            })
            .collect()
    }

    fn save_photo(&self, id: MessageId, cx: &mut Context<Self>) {
        let Some(message) = self.shown.iter().find(|message| message.id == id).cloned() else {
            return;
        };
        let saving = self
            .file_items(&message, cx)
            .into_iter()
            .find_map(|item| match item {
                Item::SaveAs(saving) => Some(saving),
                _ => None,
            });
        if let Some(saving) = saving {
            self.save_as(&saving, cx);
        }
    }
}

/// The photo's own size, or the fit from it (`zoomReset`).
fn reset(zoom: Zoom, full: f32) -> Zoom {
    if zoom.resolve(full) == Zoom::Level(0) {
        Zoom::Fit
    } else {
        Zoom::Level(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_video_entry_plays_from_its_start_time() {
        let media = model::message::Media {
            duration: 75,
            start_timestamp: 12,
            ..Default::default()
        };
        let message = Message::new(
            MessageId(1),
            model::ChatId(1),
            model::message::Sender::User(model::UserId(1)),
            Content::Video(media),
        );
        let entry = Entry::of(&message, String::new(), String::new()).expect("a video entry");
        assert_eq!(
            entry.kind,
            Kind::Video {
                start: Duration::from_secs(12),
                duration: Duration::from_secs(75),
            }
        );
    }

    #[test]
    fn a_big_photo_fits_and_zooms_by_whole_levels() {
        // A 2000x1000 photo in a 1000x1000 window fits at half size.
        let half = fit_level((2000.0, 1000.0), (1000.0, 1000.0));
        assert!((scale(half) - 0.5).abs() < 1e-6);
        assert_eq!(Zoom::Default.resolve(half), Zoom::Fit);
        // In a 1500x1000 one it fits at 3/4, between two levels.
        let full = fit_level((2000.0, 1000.0), (1500.0, 1000.0));
        assert_eq!(zoom_in(Zoom::Default, full), Zoom::Level(0));
        assert_eq!(zoom_out(Zoom::Fit, full), Zoom::Level(-1));
        assert_eq!(zoom_in(Zoom::Level(-1), full), Zoom::Fit);
        assert_eq!(reset(Zoom::Fit, full), Zoom::Level(0));
        assert_eq!(reset(Zoom::Level(0), full), Zoom::Fit);
        assert_eq!(zoom_in(Zoom::Level(MAX_ZOOM), full), Zoom::Level(MAX_ZOOM));
        assert_eq!(
            zoom_out(Zoom::Level(-MAX_ZOOM), full),
            Zoom::Level(-MAX_ZOOM)
        );
    }

    #[test]
    fn zooming_past_the_fitted_level_stops_at_it() {
        // Fitted at 1.5x: it opens at 1x, and one step in lands on the fit.
        let full = fit_level((100.0, 100.0), (150.0, 150.0));
        assert_eq!(Zoom::Default.resolve(full), Zoom::Level(0));
        assert_eq!(zoom_in(Zoom::Level(0), full), Zoom::Fit);
        assert_eq!(zoom_out(Zoom::Level(1), full), Zoom::Fit);
    }
}
