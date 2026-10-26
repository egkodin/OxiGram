//! The chat list column (`Dialogs::InnerWidget`): the chats of one list in
//! TDLib's order, loaded page by page as the user scrolls.

mod archive;
mod date;
mod menu;
mod pinned;
mod preview;
mod row;
mod search;
mod tabs;

use std::collections::HashSet;
use std::ops::Range;

use chrono::{DateTime, Local};
use gpui::{
    AnyElement, App, Context, DragMoveEvent, ElementId, Entity, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Pixels, Point, Render, SharedString, Styled, Subscription,
    UniformListScrollHandle, Window, div, prelude::*, uniform_list,
};
use i18n::{Key, Strings, key};
use model::chat::{Chat, ChatKind, ChatList};
use model::chats::{ChatAction, Chats};
use model::message::{Message, Sender};
use model::user::{Badge, Photo, UserKind};
use model::{ChatId, FileId, ListLoad, State};
use style::dialogs::{
    DIALOGS_CHANNEL_ICON, DIALOGS_CHAT_ICON, DIALOGS_RECEIVED_ICON, DIALOGS_SENT_ICON,
};
use style::dialogs_layout::DIALOGS_BOT_ICON;
use style::{ColorId, ColorRef};
use ui::{PopupMenu, UserpicSource, color};

use self::date::DialogDate;
use self::pinned::{DragGhost, DraggedPinned};
use self::preview::Preview;
use self::row::{Badges, Lit, NameBadge, NarrowRow, Row, RowText};
use self::search::Search;
use crate::Store;
use crate::lang::strings;
use crate::layers::Layers;
use crate::{columns, settings};

/// Chats asked from TDLib at a time.
const PAGE: i32 = 50;
/// Load the next page when this few loaded rows are left below the view.
const PRELOAD_ROWS: usize = 10;
/// Userpic downloads go before media but after what the user opened.
const USERPIC_PRIORITY: i32 = 8;

/// How the archive shows at the top of the main list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchivePlace {
    /// A full row, like a chat.
    Row,
    /// Collapsed to a bar with its name and counter.
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Loading {
    Idle,
    Requested,
    Complete,
}

/// Which row the pointer is over; a chat can show in several search rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKey {
    Archive,
    Chat(ChatId),
    Global(ChatId),
    Found(usize),
}

impl RowKey {
    fn element_id(self) -> ElementId {
        let (name, id) = match self {
            Self::Archive => ("archive", 0),
            Self::Chat(chat) => ("dialog", u64::from_ne_bytes(chat.0.to_ne_bytes())),
            Self::Global(chat) => ("global", u64::from_ne_bytes(chat.0.to_ne_bytes())),
            Self::Found(index) => ("found", index as u64),
        };
        (name, id).into()
    }
}

pub(crate) struct Dialogs {
    store: Entity<Store>,
    layers: Entity<Layers>,
    list: ChatList,
    scroll: UniformListScrollHandle,
    loading: Loading,
    search: Option<Search>,
    hovered: Option<RowKey>,
    active: Option<ChatId>,
    requested_files: HashSet<FileId>,
    /// Userpics only, as the list is too narrow for names.
    narrow: bool,
    /// The open context menu, where it was asked for.
    menu: Option<(Point<Pixels>, Entity<PopupMenu>, Subscription)>,
    _subscription: Subscription,
}

impl std::fmt::Debug for Dialogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dialogs").finish_non_exhaustive()
    }
}

impl Dialogs {
    pub(crate) fn new(
        store: Entity<Store>,
        layers: Entity<Layers>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        let mut dialogs = Self {
            store,
            layers,
            list: ChatList::Main,
            scroll: UniformListScrollHandle::new(),
            loading: Loading::Idle,
            search: None,
            hovered: None,
            active: None,
            requested_files: HashSet::new(),
            narrow: false,
            menu: None,
            _subscription: subscription,
        };
        dialogs.load_more(cx);
        dialogs.load_archive(cx);
        dialogs
    }

    pub(crate) fn list(&self) -> ChatList {
        self.list
    }

    /// Shows the chats of `list`, as opening the archive does.
    pub(crate) fn open_list(&mut self, list: ChatList, cx: &mut Context<Self>) {
        if self.list == list {
            return;
        }
        self.list = list;
        self.loading = Loading::Idle;
        self.hovered = None;
        self.scroll = UniformListScrollHandle::new();
        self.load_more(cx);
        cx.notify();
    }

    /// The first archived chats, so the main list knows to show the
    /// archive row.
    fn load_archive(&self, cx: &mut Context<Self>) {
        let task = crate::store::spawn(
            self.store
                .read(cx)
                .session()
                .load_chats(ChatList::Archive, PAGE),
            cx,
        );
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "loading archived chats failed");
            }
        })
        .detach();
    }

    /// Whether the archive row leads the list.
    // shortcut: always expanded; collapsing it and moving it to the main menu come with the archive context menu later in M2.
    /// Where the archive shows in this list, if at all.
    fn archive_place(&self, state: &State, cx: &App) -> Option<ArchivePlace> {
        if self.list != ChatList::Main
            || state.chats.list_len(ChatList::Archive) == 0
            || settings::is_archive_in_main_menu(cx)
        {
            None
        } else if settings::is_archive_collapsed(cx) {
            Some(ArchivePlace::Bar)
        } else {
            Some(ArchivePlace::Row)
        }
    }

    /// Whether the archive is the list's first row.
    fn shows_archive(&self, state: &State, cx: &App) -> bool {
        self.archive_place(state, cx) == Some(ArchivePlace::Row)
    }

    fn row_count(&self, state: &State, cx: &App) -> usize {
        state.chats.list_len(self.list) + usize::from(self.shows_archive(state, cx))
    }

    /// Lights `chat` as the open one.
    // shortcut: selecting only lights the row; opening the chat comes with the history in M3.
    pub(crate) fn select(&mut self, chat: ChatId, cx: &mut Context<Self>) {
        self.active = Some(chat);
        cx.notify();
    }

    fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.loading != Loading::Idle {
            return;
        }
        self.loading = Loading::Requested;
        let task = crate::store::spawn(
            self.store.read(cx).session().load_chats(self.list, PAGE),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.loading = match result {
                    Ok(ListLoad::HasMore) => Loading::Idle,
                    Ok(ListLoad::Complete) => Loading::Complete,
                    Err(error) => {
                        tracing::warn!(%error, "loading chats failed");
                        Loading::Idle
                    }
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn download(&mut self, file: FileId, cx: &mut Context<Self>) {
        if !self.requested_files.insert(file) {
            return;
        }
        let task = crate::store::spawn(
            self.store
                .read(cx)
                .session()
                .download_file(file, USERPIC_PRIORITY),
            cx,
        );
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "downloading a userpic failed");
            }
        })
        .detach();
    }

    fn rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let state = self.store.read(cx).state();
        if range.end + PRELOAD_ROWS >= self.row_count(state, cx) {
            self.load_more(cx);
        }
        let state = self.store.read(cx).state();
        let archive = usize::from(self.shows_archive(state, cx));
        let leads = archive == 1 && range.start == 0;
        let ids: Vec<ChatId> = state
            .chats
            .list(self.list)
            .skip(range.start.saturating_sub(archive))
            .take(range.len() - usize::from(leads))
            .collect();
        let mut rows = Vec::with_capacity(range.len());
        if leads {
            rows.push(self.archive_element(cx));
        }
        rows.extend(
            ids.into_iter()
                .map(|id| self.row_element(RowKey::Chat(id), id, None, cx)),
        );
        rows
    }

    fn archive_element(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lit = if self.hovered == Some(RowKey::Archive) {
            Lit::Over
        } else {
            Lit::Normal
        };
        let row = archive_row(self.store.read(cx).state(), strings(cx), lit, &Local::now());
        let open = |this: &mut Self, cx: &mut Context<Self>| this.open_list(ChatList::Archive, cx);
        if self.narrow {
            return self.wrap_row(RowKey::Archive, NarrowRow(row), cx, open);
        }
        self.wrap_row(RowKey::Archive, row, cx, open)
    }

    /// The collapsed archive (`PaintCollapsedRow`): its name and a muted
    /// counter of unread chats.
    fn archive_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let is_hovered = self.hovered == Some(RowKey::Archive);
        let unread = self
            .store
            .read(cx)
            .state()
            .chats
            .unread(ChatList::Archive)
            .chats;
        let bg = if is_hovered {
            ColorId::DialogsBgOver
        } else {
            ColorId::DialogsBg
        };
        if self.narrow {
            // The archive's userpic, small and centered.
            let size = crate::px_of(style::dialogs::DIALOGS_UNREAD_HEIGHT);
            let bar = div()
                .h(crate::px_of(style::dialogs::DIALOGS_IMPORTANT_BAR_HEIGHT))
                .w_full()
                .bg(color(cx, ColorRef::Palette(bg)))
                .flex()
                .items_center()
                .justify_center()
                .child(ui::Userpic::new(size, UserpicSource::Archive));
            return self.wrap_row(RowKey::Archive, bar, cx, |this, cx| {
                this.open_list(ChatList::Archive, cx);
            });
        }
        let bar =
            div()
                .h(crate::px_of(style::dialogs::DIALOGS_IMPORTANT_BAR_HEIGHT))
                .w_full()
                .bg(color(cx, ColorRef::Palette(bg)))
                .flex()
                .items_center()
                .pl(crate::px_of(style::dialogs::DIALOGS_TOP_BAR_LEFT_PADDING))
                .pr(crate::px_of(
                    style::dialogs::DEFAULT_DIALOG_ROW.padding.right,
                ))
                .child(
                    div()
                        .flex_1()
                        .font(ui::font(style::basic::SEMIBOLD_FONT))
                        .text_size(crate::px_of(style::basic::SEMIBOLD_FONT.size))
                        .text_color(color(cx, ColorRef::Palette(ColorId::DialogsNameFg)))
                        .child(crate::lang::tr(cx, key::lng_archived_name)),
                )
                .children((unread > 0).then(|| {
                    row::unread_badge(unread.to_string().into(), true, row::Lit::Normal, cx)
                }));
        self.wrap_row(RowKey::Archive, bar, cx, |this, cx| {
            this.open_list(ChatList::Archive, cx);
        })
    }

    /// Row `key` for chat `id`, showing `found` instead of the last message
    /// when given, with its hover and click handlers.
    fn row_element(
        &mut self,
        key: RowKey,
        id: ChatId,
        found: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let lit = if self.active == Some(id) && found.is_none() {
            Lit::Active
        } else if self.hovered == Some(key) {
            Lit::Over
        } else {
            Lit::Normal
        };
        let found = found.and_then(|index| {
            self.search
                .as_ref()
                .and_then(|search| search.messages.get(index))
        });
        let state = self.store.read(cx).state();
        let (row, download) =
            build_row(state, strings(cx), id, self.list, found, lit, &Local::now());
        if let Some(file) = download {
            self.download(file, cx);
        }
        if self.narrow {
            return self.wrap_row(key, NarrowRow(row), cx, move |this, cx| this.select(id, cx));
        }
        self.wrap_row(key, row, cx, move |this, cx| this.select(id, cx))
    }

    /// `row` with the hover tracking for `key` and `on_click`.
    fn wrap_row(
        &mut self,
        key: RowKey,
        row: impl IntoElement,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let draggable =
            chat_of(key).filter(|chat| self.search.is_none() && self.pinned(cx).contains(chat));
        div()
            .id(key.element_id())
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                let next = if *hovered {
                    Some(key)
                } else if this.hovered == Some(key) {
                    None
                } else {
                    this.hovered
                };
                if this.hovered != next {
                    this.hovered = next;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
            .when_some(chat_of(key), |row, chat| {
                row.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.show_menu(chat, event.position, window, cx);
                    }),
                )
            })
            .when(key == RowKey::Archive, |row| {
                row.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        this.show_archive_menu(event.position, window, cx);
                    }),
                )
            })
            .when_some(draggable, |row, chat| {
                row.on_drag(DraggedPinned(chat), |_, _, _, cx| cx.new(|_| DragGhost))
                    // Hover does not track during a drag; light the row under the pointer.
                    .on_drag_move(cx.listener(
                        move |this, event: &DragMoveEvent<DraggedPinned>, _, cx| {
                            if event.bounds.contains(&event.event.position)
                                && this.hovered != Some(key)
                            {
                                this.hovered = Some(key);
                                cx.notify();
                            }
                        },
                    ))
                    .on_drop(cx.listener(move |this, dragged: &DraggedPinned, _, cx| {
                        this.drop_pinned(dragged.0, chat, cx);
                    }))
            })
            .child(row)
            .into_any_element()
    }
}

/// The chat a row's context menu acts on; search rows have none yet.
fn chat_of(key: RowKey) -> Option<ChatId> {
    match key {
        RowKey::Chat(chat) => Some(chat),
        RowKey::Archive | RowKey::Global(_) | RowKey::Found(_) => None,
    }
}

impl Render for Dialogs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = columns::layout(
            f32::from(window.viewport_size().width),
            settings::dialogs_width_ratio(cx),
        );
        self.narrow = layout == columns::Layout::Narrow;
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(self.tabs_strip(cx))
            .child(div().flex_1().min_h_0().child(self.body(cx)))
            .children(self.menu_overlay())
    }
}

impl Dialogs {
    fn body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.row_count(self.store.read(cx).state(), cx);
        let background = color(cx, ColorRef::Palette(ColorId::DialogsBg));
        if let Some(search) = &self.search {
            return div()
                .size_full()
                .bg(background)
                .child(self.search_list(search, cx))
                .into_any_element();
        }
        // `InnerWidget::refreshEmpty`.
        // shortcut: no "Add contact" / "Edit folder" links until those boxes exist (M6).
        let empty_text = match (self.loading, self.list) {
            (Loading::Complete, ChatList::Folder(_)) => key::lng_no_chats_filter,
            (Loading::Complete, ChatList::Main | ChatList::Archive) => key::lng_no_chats,
            (Loading::Idle | Loading::Requested, _) => key::lng_contacts_loading,
        };
        if count == 0 {
            return div()
                .size_full()
                .bg(background)
                .flex()
                .justify_center()
                .pt(crate::px_of(style::dialogs::DIALOGS_EMPTY_SKIP))
                .child(ui::FlatLabel::new(
                    "dialogs-empty",
                    &style::dialogs::DIALOGS_EMPTY_LABEL,
                    ui::Markup::plain(crate::lang::tr(cx, empty_text).to_string()),
                ))
                .into_any_element();
        }
        let list = uniform_list(
            "dialogs",
            count,
            cx.processor(|this, range: Range<usize>, _, cx| this.rows(range, cx)),
        )
        .track_scroll(&self.scroll)
        .size_full()
        .bg(background);
        let state = self.store.read(cx).state();
        if self.archive_place(state, cx) != Some(ArchivePlace::Bar) {
            return list.into_any_element();
        }
        // shortcut: the bar stays put instead of scrolling with the list; uniform_list needs equal rows.
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(background)
            .child(self.archive_bar(cx))
            .child(div().flex_1().min_h_0().child(list))
            .into_any_element()
    }
}

/// The row for chat `id`, and a userpic to download if it is missing.
fn build_row(
    state: &State,
    strings: &Strings,
    id: ChatId,
    list: ChatList,
    found: Option<&Message>,
    lit: Lit,
    now: &DateTime<Local>,
) -> (Row, Option<FileId>) {
    let chats = &state.chats;
    let Some(chat) = chats.chat(id) else {
        return (empty_row(lit), None);
    };
    let user = chat.user().and_then(|user| chats.user(user));
    let is_self = chat.user().is_some_and(|user| Some(user) == state.me)
        && matches!(chat.kind, ChatKind::Private { .. });
    let (userpic, download) = userpic(chats, chat, is_self);
    let name: SharedString = if is_self {
        strings.text(key::lng_saved_messages).into()
    } else if user.is_some_and(|user| user.kind == UserKind::Deleted) {
        strings.text(key::lng_deleted).into()
    } else {
        chat.title.clone().into()
    };
    let type_icon = match chat.kind {
        ChatKind::Supergroup { is_channel: true } => Some(&DIALOGS_CHANNEL_ICON),
        ChatKind::Group | ChatKind::Supergroup { is_channel: false } => Some(&DIALOGS_CHAT_ICON),
        ChatKind::Private { .. } | ChatKind::Secret { .. } => user
            .filter(|user| user.kind == UserKind::Bot)
            .map(|_| &DIALOGS_BOT_ICON),
    };
    // shortcut: group and channel badges need supergroup updates; they land with profiles in M6.
    let name_badge = user.filter(|_| !is_self).and_then(|user| match user.badge {
        Badge::Verified => Some(NameBadge::Verified),
        Badge::Scam => Some(NameBadge::Scam),
        Badge::Fake => Some(NameBadge::Fake),
        Badge::None => user.is_premium.then_some(NameBadge::Premium),
    });
    let scam_text: SharedString = match name_badge {
        Some(NameBadge::Fake) => strings.text(key::lng_fake_badge).into(),
        _ => strings.text(key::lng_scam_badge).into(),
    };
    let is_muted = chats.is_muted(chat);
    let badges = if found.is_some() {
        Badges::default()
    } else {
        Badges {
            unread: (chat.unread_count > 0)
                .then(|| SharedString::from(chat.unread_count.to_string()))
                .or_else(|| chat.is_marked_as_unread.then(SharedString::default)),
            is_muted,
            has_mention: chat.unread_mentions > 0,
            has_reaction: chat.unread_reactions > 0,
        }
    };
    let message = found.or(chat.last_message.as_ref());
    // A draft shows unless unread messages wait (`RowPainter::Paint`).
    let draft = chat
        .draft
        .as_ref()
        .filter(|_| found.is_none() && (message.is_none() || badges.unread.is_none()));
    let date = draft
        .map(|draft| draft.date)
        .into_iter()
        .chain(message.map(|message| message.date))
        .max()
        .filter(|date| *date > 0)
        .map(|date| date_text(strings, date, now));
    let send_state = message
        .filter(|message| message.is_outgoing && draft.is_none() && !is_self)
        .map(|message| {
            if message.id > chat.last_read_outbox {
                &DIALOGS_SENT_ICON
            } else {
                &DIALOGS_RECEIVED_ICON
            }
        });
    let action = found
        .is_none()
        .then(|| action_text(strings, chats, chat))
        .flatten();
    let text = if let Some(action) = action {
        RowText::Action(action.into())
    } else if let Some(draft) = draft {
        RowText::Draft(preview::draft(strings, draft))
    } else if let Some(message) = message {
        let sender = preview::sender(strings, chats, chat, message);
        let (preview, thumbnail) = preview::message(strings, sender.as_deref(), message);
        RowText::Message {
            preview,
            thumbnail: thumbnail.map(|thumbnail| ui::jpeg_image(thumbnail.jpeg.clone())),
        }
    } else {
        RowText::Empty
    };
    let is_pinned = found.is_none()
        && chat
            .position(list)
            .is_some_and(|position| position.is_pinned);
    let is_online = !is_self
        && user
            .is_some_and(|user| user.is_online(i32::try_from(now.timestamp()).unwrap_or(i32::MAX)));
    (
        Row {
            userpic,
            is_online,
            name,
            type_icon,
            name_badge,
            scam_text,
            date,
            send_state,
            text,
            badges,
            is_pinned,
            lit,
        },
        download,
    )
}

/// The archive folder's row: its latest chats' names, and how many of
/// them are unread in a muted badge.
fn archive_row(state: &State, strings: &Strings, lit: Lit, now: &DateTime<Local>) -> Row {
    let chats = &state.chats;
    let unread = chats.unread(ChatList::Archive).chats;
    Row {
        userpic: UserpicSource::Archive,
        is_online: false,
        name: strings.text(key::lng_archived_name).into(),
        type_icon: None,
        name_badge: None,
        scam_text: SharedString::default(),
        date: archive::date(chats).map(|date| date_text(strings, date, now)),
        send_state: None,
        text: RowText::Message {
            preview: archive::preview(strings, chats),
            thumbnail: None,
        },
        badges: Badges {
            unread: (unread > 0).then(|| unread.to_string().into()),
            is_muted: true,
            has_mention: false,
            has_reaction: false,
        },
        is_pinned: false,
        lit,
    }
}

fn empty_row(lit: Lit) -> Row {
    Row {
        userpic: UserpicSource::Empty {
            color_index: 0,
            name: SharedString::default(),
        },
        is_online: false,
        name: SharedString::default(),
        type_icon: None,
        name_badge: None,
        scam_text: SharedString::default(),
        date: None,
        send_state: None,
        text: RowText::Message {
            preview: Preview::default(),
            thumbnail: None,
        },
        badges: Badges::default(),
        is_pinned: false,
        lit,
    }
}

fn userpic(chats: &Chats, chat: &Chat, is_self: bool) -> (UserpicSource, Option<FileId>) {
    if is_self {
        return (UserpicSource::SavedMessages, None);
    }
    photo_userpic(chats, chat.photo.as_ref(), chat.accent_color, &chat.title)
}

/// The userpic for `photo`: the downloaded file, else its blurred preview
/// with the file to download, else initials on the accent color.
pub(crate) fn photo_userpic(
    chats: &Chats,
    photo: Option<&Photo>,
    accent_color: i32,
    name: &str,
) -> (UserpicSource, Option<FileId>) {
    let initials = || UserpicSource::Empty {
        color_index: accent_color,
        name: name.to_owned().into(),
    };
    let Some(photo) = photo else {
        return (initials(), None);
    };
    if let Some(path) = chats.file(photo.small).and_then(|file| file.path.clone()) {
        return (UserpicSource::File(path.into()), None);
    }
    let source = match &photo.minithumbnail {
        Some(thumbnail) => UserpicSource::Jpeg(ui::jpeg_image(thumbnail.jpeg.clone())),
        None => initials(),
    };
    (source, Some(photo.small))
}

fn date_text(strings: &Strings, date: i32, now: &DateTime<Local>) -> SharedString {
    match date::format(i64::from(date), now) {
        DialogDate::Time(text) | DialogDate::Date(text) => text.into(),
        DialogDate::Weekday(day) => strings.text(weekday_key(day)).into(),
    }
}

fn weekday_key(day: u32) -> Key {
    match day {
        1 => key::lng_weekday1,
        2 => key::lng_weekday2,
        3 => key::lng_weekday3,
        4 => key::lng_weekday4,
        5 => key::lng_weekday5,
        6 => key::lng_weekday6,
        _ => key::lng_weekday7,
    }
}

/// "typing" in private chats, "Ann is typing" in groups, for the first
/// one acting (`SendActionPainter`).
// shortcut: one person and no animation; "Ann and Bob are typing" and the animated dots come with the chat top bar in M3.
fn action_text(strings: &Strings, chats: &Chats, chat: &Chat) -> Option<String> {
    let (sender, action) = chats.actions(chat.id).first()?;
    let (alone, with_user) = action_keys(action);
    let emoji = match action {
        ChatAction::WatchingAnimations(emoji) => emoji.as_str(),
        _ => "",
    };
    if chat.user().is_some() {
        return Some(i18n::fill(strings.text(alone), &[("emoji", emoji)]));
    }
    let name = match sender {
        Sender::User(user) => chats
            .user(*user)
            .map(|user| preview::short_name(strings, user)),
        Sender::Chat(id) => chats.chat(*id).map(|chat| chat.title.clone()),
    }?;
    Some(i18n::fill(
        strings.text(with_user),
        &[("user", name.as_str()), ("emoji", emoji)],
    ))
}

fn action_keys(action: &ChatAction) -> (Key, Key) {
    match action {
        ChatAction::Typing | ChatAction::ChoosingLocation | ChatAction::ChoosingContact => {
            (key::lng_typing, key::lng_user_typing)
        }
        ChatAction::RecordingVideo => (
            key::lng_send_action_record_video,
            key::lng_user_action_record_video,
        ),
        ChatAction::UploadingVideo => (
            key::lng_send_action_upload_video,
            key::lng_user_action_upload_video,
        ),
        ChatAction::RecordingVoice => (
            key::lng_send_action_record_audio,
            key::lng_user_action_record_audio,
        ),
        ChatAction::UploadingVoice => (
            key::lng_send_action_upload_audio,
            key::lng_user_action_upload_audio,
        ),
        ChatAction::RecordingVideoNote => (
            key::lng_send_action_record_round,
            key::lng_user_action_record_round,
        ),
        ChatAction::UploadingVideoNote => (
            key::lng_send_action_upload_round,
            key::lng_user_action_upload_round,
        ),
        ChatAction::UploadingPhoto => (
            key::lng_send_action_upload_photo,
            key::lng_user_action_upload_photo,
        ),
        ChatAction::UploadingDocument => (
            key::lng_send_action_upload_file,
            key::lng_user_action_upload_file,
        ),
        ChatAction::ChoosingSticker => (
            key::lng_send_action_choose_sticker,
            key::lng_user_action_choose_sticker,
        ),
        ChatAction::PlayingGame => (key::lng_playing_game, key::lng_user_playing_game),
        ChatAction::WatchingAnimations(_) => (
            key::lng_user_action_watching_animations,
            key::lng_user_action_watching_animations,
        ),
    }
}
