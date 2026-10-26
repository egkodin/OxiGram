//! An open chat's messages (`HistoryWidget` / `HistoryInner`): a list
//! pinned to the bottom that loads older and newer pages as the user
//! scrolls toward them.

mod body;
mod bubble;
mod headers;
mod layout;
mod service;
mod sync;
mod text;
mod top_bar;

pub(crate) use self::headers::Replied;
pub(crate) use self::service::text as service_text;
pub(crate) use self::text::Link;

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use chrono::Local;
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, IntoElement, ListAlignment, ListState,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div, list, px,
};
use model::chat::ChatKind;
use model::history::Page;
use model::message::{Content, Message, Sender, Service};
use model::{ChatId, FileId, MessageId, State};
use style::{ColorId, ColorRef};
use ui::color;

use self::bubble::{Author, Item, Run};
use crate::Store;
use crate::dialogs::photo_userpic;
use crate::lang::strings;

/// Pixels of content the list lays out past each edge.
const OVERDRAW: f32 = 1000.0;
/// Within this many messages of an end the next page loads.
const PRELOAD: usize = 20;
/// The same as the chat list's (`kUserpicPriority`).
const USERPIC_PRIORITY: i32 = 1;

/// What the chat asks of the window around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HistoryEvent {
    /// The top bar's back arrow.
    Close,
    /// A link in a message other than a spoiler, which reveals in place.
    Link(Link),
}

impl EventEmitter<HistoryEvent> for HistoryView {}

/// Whether the chat fills the window, so its top bar leads back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Placement {
    #[default]
    BesideList,
    Alone,
}

pub(crate) struct HistoryView {
    store: Entity<Store>,
    chat: ChatId,
    placement: Placement,
    list: ListState,
    /// What the list's items stand for, to diff against the store.
    shown: Vec<Arc<Message>>,
    loading: Option<Page>,
    /// Set when items changed, so the ends are checked once laid out.
    needs_preload_check: bool,
    requested_files: HashSet<FileId>,
    /// Messages whose spoilers the user clicked open.
    revealed_spoilers: HashSet<MessageId>,
    /// Replied and pinned messages asked for, and those that are gone.
    requested_references: HashSet<(ChatId, MessageId)>,
    missing_references: HashSet<(ChatId, MessageId)>,
    _subscription: Subscription,
}

impl std::fmt::Debug for HistoryView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryView")
            .field("chat", &self.chat)
            .finish_non_exhaustive()
    }
}

impl HistoryView {
    pub(crate) fn new(store: Entity<Store>, chat: ChatId, cx: &mut Context<Self>) -> Self {
        store.update(cx, |store, cx| store.open_chat(chat, cx));
        cx.on_release(move |this, cx| {
            this.store
                .update(cx, |store, cx| store.close_chat(chat, cx));
        })
        .detach();
        let subscription = cx.observe(&store, |this, _, cx| this.sync(cx));
        let list = ListState::new(0, ListAlignment::Bottom, px(OVERDRAW));
        let weak = cx.weak_entity();
        list.set_scroll_handler(move |_, _, cx| {
            // The view is gone once its chat closed; nothing to load then.
            let _ = weak.update(cx, |this, cx| this.preload(cx));
        });
        let mut view = Self {
            store,
            chat,
            placement: Placement::default(),
            list,
            shown: Vec::new(),
            loading: None,
            needs_preload_check: false,
            requested_files: HashSet::new(),
            revealed_spoilers: HashSet::new(),
            requested_references: HashSet::new(),
            missing_references: HashSet::new(),
            _subscription: subscription,
        };
        view.sync(cx);
        view.load(Page::Newest, cx);
        view
    }

    pub(crate) fn chat(&self) -> ChatId {
        self.chat
    }

    pub(crate) fn set_placement(&mut self, placement: Placement, cx: &mut Context<Self>) {
        if self.placement != placement {
            self.placement = placement;
            cx.notify();
        }
    }

    /// Brings the list in line with the store's history.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let Some(history) = self.store.read(cx).state().history(self.chat) else {
            return;
        };
        if sync::apply(&self.list, &self.shown, history.messages()) {
            self.shown = history.messages().to_vec();
            self.needs_preload_check = true;
            cx.notify();
        }
    }

    /// Loads the next page when the user nears an end that has more.
    fn preload(&mut self, cx: &mut Context<Self>) {
        let Some(history) = self.store.read(cx).state().history(self.chat) else {
            return;
        };
        let count = self.list.item_count();
        let near_top = self.list.item_is_above_viewport(PRELOAD.min(count)) == Some(false);
        let near_bottom = count
            .checked_sub(PRELOAD + 1)
            .is_none_or(|index| self.list.item_is_below_viewport(index) == Some(false));
        let page = match (near_top, near_bottom) {
            (true, _) => history.older_page(),
            (_, true) => history.newer_page(),
            _ => None,
        };
        if let Some(page) = page {
            self.load(page, cx);
        }
    }

    fn load(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.loading.is_some() {
            return;
        }
        self.loading = Some(page);
        let chat = self.chat;
        let task = crate::store::spawn(self.store.read(cx).session().chat_history(chat, page), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.loading = None;
                match result {
                    Ok(messages) => this.store.update(cx, |store, cx| {
                        store.add_history_page(chat, page, messages, cx);
                    }),
                    Err(error) => tracing::warn!(%error, "loading history failed"),
                }
            });
        })
        .detach();
    }

    fn item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(message) = self.shown.get(index).cloned() else {
            return div().into_any_element();
        };
        if let Some(reply_to) = &message.reply_to {
            self.ensure_reference(reply_to.chat, reply_to.message, cx);
        }
        let previous = index.checked_sub(1).and_then(|i| self.shown.get(i));
        let next = self.shown.get(index + 1);
        let now = Local::now();
        let tz = Local;
        let day = layout::day(message.date, &tz);
        let date_badge = (previous.and_then(|p| layout::day(p.date, &tz)) != day)
            .then_some(day)
            .flatten()
            .map(|day| SharedString::from(layout::date_badge(strings(cx), day, &now)));
        if let Content::Service(service) = &message.content {
            return self.service_item(&message, service, date_badge, cx);
        }
        let run = Run {
            is_attached_to_previous: previous
                .is_some_and(|p| layout::is_attached(p, &message, &tz)),
            is_attached_to_next: next.is_some_and(|n| layout::is_attached(&message, n, &tz)),
        };
        let state = self.store.read(cx).state();
        let author = self.author(state, &message);
        let is_read = state
            .chats
            .chat(self.chat)
            .is_some_and(|chat| chat.last_read_outbox >= message.id);
        let time = layout::time(message.date, &tz);
        let color_index = sender_color(state, &message);
        let reply = message.reply_to.as_ref().map(|reply_to| {
            let replied = self.replied(state, reply_to.chat, reply_to.message);
            headers::reply(strings(cx), &state.chats, reply_to, replied, |replied| {
                sender_color(state, replied)
            })
        });
        let forward = message
            .forward
            .as_ref()
            .map(|forward| headers::forwarded(strings(cx), &state.chats, forward));
        if let Some((_, Some(file))) = &author {
            let file = *file;
            self.request_file(file, cx);
        }
        bubble::element(
            Item {
                message: &message,
                date_badge,
                run,
                author: author.map(|(author, _)| author),
                is_read,
                time,
                color_index,
                is_spoiler_revealed: self.revealed_spoilers.contains(&message.id),
                on_link: self.on_link(message.id, cx),
                reply,
                forward,
            },
            cx,
        )
    }

    fn on_link(&self, message: MessageId, cx: &Context<Self>) -> body::OnLink {
        let weak = cx.weak_entity();
        Rc::new(move |link, _, cx| {
            let _ = weak.update(cx, |this, cx| match link {
                Link::Spoiler => {
                    this.revealed_spoilers.insert(message);
                    cx.notify();
                }
                link => cx.emit(HistoryEvent::Link(link)),
            });
        })
    }

    fn service_item(
        &mut self,
        message: &Message,
        service: &Service,
        date_badge: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Service::Pinned(id) = service {
            self.ensure_reference(self.chat, *id, cx);
        }
        let state = self.store.read(cx).state();
        let pinned = match service {
            Service::Pinned(id) => self.replied(state, self.chat, *id),
            _ => Replied::Loading,
        };
        let text = state.chats.chat(self.chat).and_then(|chat| {
            service::text(strings(cx), &state.chats, chat, message, service, pinned)
        });
        bubble::service_element(date_badge, text.map(SharedString::from), cx)
    }

    fn replied<'s>(&self, state: &'s State, chat: ChatId, id: MessageId) -> Replied<'s> {
        match state
            .history(self.chat)
            .and_then(|history| history.find(chat, id))
        {
            Some(message) => Replied::Loaded(message),
            None if self.missing_references.contains(&(chat, id)) => Replied::Deleted,
            None => Replied::Loading,
        }
    }

    /// Fetches a replied or pinned message that is not loaded, once.
    fn ensure_reference(&mut self, chat: ChatId, id: MessageId, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state();
        let is_known = state
            .history(self.chat)
            .is_some_and(|history| history.find(chat, id).is_some());
        if is_known || !self.requested_references.insert((chat, id)) {
            return;
        }
        let task = crate::store::spawn(self.store.read(cx).session().message(chat, id), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(message) => {
                    let history = this.chat;
                    this.store
                        .update(cx, |store, cx| store.add_reference(history, message, cx));
                }
                Err(error) => {
                    tracing::debug!(%error, "a referenced message is gone");
                    this.missing_references.insert((chat, id));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The author of an incoming message in a group, with the userpic file
    /// to download.
    fn author(&self, state: &State, message: &Message) -> Option<(Author, Option<FileId>)> {
        let chats = &state.chats;
        let is_group = chats.chat(self.chat).is_some_and(|chat| match chat.kind {
            ChatKind::Group { .. } => true,
            ChatKind::Supergroup { is_channel, .. } => !is_channel,
            ChatKind::Private { .. } | ChatKind::Secret { .. } => false,
        });
        if !is_group || message.is_outgoing {
            return None;
        }
        let (name, color_index, (userpic, file)) = match message.sender {
            Sender::User(id) => {
                let user = chats.user(id)?;
                let name = user.name();
                let userpic = photo_userpic(chats, user.photo.as_ref(), user.accent_color, &name);
                (name, user.accent_color, userpic)
            }
            Sender::Chat(id) => {
                let chat = chats.chat(id)?;
                let userpic =
                    photo_userpic(chats, chat.photo.as_ref(), chat.accent_color, &chat.title);
                (chat.title.clone(), chat.accent_color, userpic)
            }
        };
        Some((
            Author {
                name: name.into(),
                color_index,
                userpic,
            },
            file,
        ))
    }

    fn request_file(&mut self, file: FileId, cx: &mut Context<Self>) {
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
}

/// The color of whoever sent `message` (`colorIndex`).
fn sender_color(state: &State, message: &Message) -> i32 {
    match message.sender {
        Sender::User(id) => state.chats.user(id).map_or(0, |user| user.accent_color),
        Sender::Chat(id) => state.chats.chat(id).map_or(0, |chat| chat.accent_color),
    }
}

impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.needs_preload_check) {
            // New items are measured only by this frame's layout.
            let weak = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                let _ = weak.update(cx, |this, cx| this.preload(cx));
            });
        }
        let on_back = (self.placement == Placement::Alone).then(|| {
            let weak = cx.weak_entity();
            let on_back: top_bar::OnBack = Box::new(move |_, _, cx| {
                let _ = weak.update(cx, |_, cx| cx.emit(HistoryEvent::Close));
            });
            on_back
        });
        let bar = top_bar::top_bar(self.store.read(cx).state(), self.chat, on_back, cx);
        let weak = cx.weak_entity();
        // shortcut: a plain color instead of the chat wallpaper, which comes with backgrounds in M6.
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(color(cx, ColorRef::Palette(ColorId::WindowBgOver)))
            .child(bar)
            .child(
                div()
                    .h(px(1.0))
                    .flex_none()
                    .bg(color(cx, ColorRef::Palette(ColorId::ShadowFg))),
            )
            .child(
                list(self.list.clone(), move |index, _, cx: &mut App| {
                    weak.update(cx, |this, cx| this.item(index, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .flex_1()
                .min_h_0()
                .pb(crate::px_of(style::chat::HISTORY_PADDING_BOTTOM)),
            )
    }
}
