//! A message's context menu (`HistoryInner::showContextMenu`): copy the
//! selection, the text, a link or the message link, delete, select.

use std::time::Duration;

use gpui::{
    Anchor, AnyElement, App, AppContext as _, ClipboardItem, Context, DismissEvent, Entity,
    Focusable as _, IntoElement, ParentElement, Pixels, Point, SharedString, Styled as _,
    Subscription, Window, anchored, deferred, div, point, px,
};
use i18n::key;
use model::MessageId;
use model::chat::ChatKind;
use model::message::{Content, Message};
use model::message_actions::Abilities;
use model::reaction::ReactionKind;
use style::info::TOP_BAR_MENU_POSITION;
use style::menu_icons::{
    MENU_ICON_COPY, MENU_ICON_DELETE_ATTENTION, MENU_ICON_DOWNLOAD, MENU_ICON_EDIT,
    MENU_ICON_FORWARD, MENU_ICON_LINK, MENU_ICON_REMOVE, MENU_ICON_REPLY, MENU_ICON_RESCHEDULE,
    MENU_ICON_RETRACT_VOTE, MENU_ICON_SELECT, MENU_ICON_SEND, MENU_ICON_SHOW_IN_FOLDER,
};
use style::widgets::{DEFAULT_TOAST, POPUP_MENU_WITH_ICONS};
use ui::anim::{Easing, Simple};
use ui::{MenuTone, PopupMenu};

use super::reacting::Strip;
use super::selection::Selection;
use super::who_reacted::WhoLine;
use super::{HistoryView, Link, delete, field_bar, text};
use crate::lang::tr;
use crate::peer_menu::PeerMenu;
use crate::px_of;
use crate::store::spawn;

/// Keeps the menu off the window edges.
const EDGE_MARGIN: f32 = 8.0;
pub(super) const TOAST_DURATION: Duration = Duration::from_millis(1500);

/// What TDLib told about the messages a menu is for.
pub(super) struct Facts<'a> {
    /// Empty when unknown.
    abilities: &'a [Abilities],
    /// Who saw or reacted to a lone message.
    who_read: Option<WhoLine>,
    /// What a lone message takes, for the strip over the menu.
    reactions: Vec<ReactionKind>,
}

/// A menu over the history, at the window point its `corner` sits on.
pub(super) struct OpenMenu {
    position: Point<Pixels>,
    corner: Anchor,
    menu: Entity<PopupMenu>,
    strip: Option<Strip>,
    /// The message a message menu is for.
    message: Option<MessageId>,
    /// The menu fading in and sliding down into place.
    appear: Simple,
    _subscription: Subscription,
}

/// How long a menu takes to fade in (lib_ui's panel animation).
const APPEAR_MS: i32 = 150;
/// How far above its place a menu starts.
const APPEAR_LIFT: f32 = 6.0;

/// Where the menu opened, against the selection (`isUponSelected`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Upon {
    Nothing,
    SelectedText,
    /// Text is selected in another message or elsewhere in this one.
    OtherText,
    SelectedMessage,
    /// Messages are selected, but not this one.
    OtherMessages,
}

/// What a link under the pointer copies (`copyToClipboardContextItemText`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LinkCopy {
    Link(String),
    Email(String),
    Mention(String),
    Hashtag(String),
}

impl LinkCopy {
    fn of(link: &Link) -> Option<Self> {
        Some(match link {
            Link::Url(url) | Link::HiddenUrl(url) => Self::Link(url.clone()),
            Link::Email(email) => Self::Email(email.clone()),
            Link::Username(name) => Self::Mention(format!("@{name}")),
            Link::Hashtag(tag) => Self::Hashtag(tag.clone()),
            Link::User(_)
            | Link::Profile(_)
            | Link::Phone(_)
            | Link::BankCard(_)
            | Link::Date(_)
            | Link::Code(_)
            | Link::Spoiler
            | Link::CustomEmoji(_) => {
                return None;
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Item {
    Reply,
    Edit,
    CopySelectedText,
    CopySelectedMessages,
    CopyText,
    CopyLink(LinkCopy),
    CopyMessageLink,
    Forward,
    ForwardSelected,
    Delete,
    DeleteSelected,
    Select,
    SelectUpTo,
    ClearSelection,
    SendNow,
    Reschedule,
    /// A downloaded file in Finder, or the file manager elsewhere.
    ShowInFolder(std::path::PathBuf),
    SaveAs(super::save_as::Saving),
    RetractVote,
    StopPoll,
}

/// A scheduled message's menu: send now and reschedule, then copying and
/// deleting (`ScheduledWidget`'s context menu).
// shortcut: no Edit, as the section has no field yet.
fn scheduled_items(items: Vec<Item>) -> Vec<Item> {
    let kept = items.into_iter().filter(|item| {
        matches!(
            item,
            Item::CopySelectedText | Item::CopyText | Item::CopyLink(_) | Item::Delete
        )
    });
    [Item::SendNow, Item::Reschedule]
        .into_iter()
        .chain(kept)
        .collect()
}

/// The menu's items in tdesktop's order. `abilities` are those of the
/// message, or of every selected one over a selected message; empty when
/// TDLib did not tell, which offers only what needs no permission.
// shortcut: no Pin or Report yet; they come with the pinned bar and reports (M6).
pub(super) fn items(
    upon: Upon,
    has_text: bool,
    link: Option<LinkCopy>,
    abilities: &[Abilities],
) -> Vec<Item> {
    let can_copy = abilities.iter().all(|a| a.can_be_copied);
    let can_delete = !abilities.is_empty() && abilities.iter().all(Abilities::can_be_deleted);
    let can_forward = !abilities.is_empty() && abilities.iter().all(|a| a.can_be_forwarded);
    let can_get_link = abilities.first().is_some_and(|a| a.can_get_link);
    let can_reply = abilities.first().is_some_and(|a| a.can_be_replied);
    let can_edit = abilities.first().is_some_and(|a| a.can_be_edited);
    let link_item = |is_message_link_allowed: bool| {
        link.clone()
            .map(Item::CopyLink)
            .or_else(|| (is_message_link_allowed && can_get_link).then_some(Item::CopyMessageLink))
    };
    let mut items = Vec::new();
    match upon {
        Upon::SelectedMessage => {
            items.extend(can_copy.then_some(Item::CopySelectedMessages));
            items.extend(link_item(false));
            items.extend(can_forward.then_some(Item::ForwardSelected));
            items.extend(can_delete.then_some(Item::DeleteSelected));
            items.push(Item::ClearSelection);
        }
        Upon::OtherMessages => {
            items.extend(link_item(false));
            items.extend([Item::Select, Item::SelectUpTo]);
        }
        Upon::SelectedText | Upon::OtherText | Upon::Nothing => {
            items.extend(can_reply.then_some(Item::Reply));
            items.extend(can_edit.then_some(Item::Edit));
            if upon == Upon::SelectedText && can_copy {
                items.push(Item::CopySelectedText);
            }
            if upon == Upon::Nothing && has_text && can_copy {
                items.push(Item::CopyText);
            }
            items.extend(link_item(true));
            items.extend(can_forward.then_some(Item::Forward));
            items.extend(can_delete.then_some(Item::Delete));
            items.push(Item::Select);
        }
    }
    items
}

/// What a right click hit: the message at `index`, at a text offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Hit {
    pub index: usize,
    pub offset: Option<usize>,
}

impl LinkCopy {
    fn text(&self) -> &str {
        match self {
            Self::Link(text) | Self::Email(text) | Self::Mention(text) | Self::Hashtag(text) => {
                text
            }
        }
    }

    fn label(&self) -> i18n::Key {
        match self {
            Self::Link(_) => key::lng_context_copy_link,
            Self::Email(_) => key::lng_context_copy_email,
            Self::Mention(_) => key::lng_context_copy_mention,
            Self::Hashtag(_) => key::lng_context_copy_hashtag,
        }
    }
}

/// What every message allows instead of asking TDLib, as the snapshots
/// need with no account behind them.
struct PresetAbilities(Option<Abilities>);

impl gpui::Global for PresetAbilities {}

/// Every message allows `abilities` until this is called with `None`.
pub fn preset_message_abilities(abilities: Option<Abilities>, cx: &mut App) {
    cx.set_global(PresetAbilities(abilities));
}

/// With preset abilities every message takes these reactions too.
pub(super) fn preset_reactions(cx: &App) -> Option<Vec<ReactionKind>> {
    cx.try_global::<PresetAbilities>()
        .and_then(|preset| preset.0)
        .map(|_| {
            ["👍", "❤", "🔥", "🥰", "👏", "😁", "🤔"]
                .map(|emoji| ReactionKind::Emoji(emoji.to_owned()))
                .to_vec()
        })
}

impl HistoryView {
    /// What the user may do with each of `ids`, from TDLib; empty when it
    /// did not tell for any of them.
    pub(super) fn abilities_of(
        &self,
        ids: &[MessageId],
        cx: &App,
    ) -> impl std::future::Future<Output = Vec<Abilities>> + use<> {
        let preset = cx
            .try_global::<PresetAbilities>()
            .and_then(|preset| preset.0)
            .map(|preset| vec![preset; ids.len()]);
        let session = self.store.read(cx).session().clone();
        let asked = if preset.is_some() { &[][..] } else { ids };
        let tasks: Vec<_> = asked
            .iter()
            .map(|id| spawn(session.message_abilities(self.chat, *id), cx))
            .collect();
        async move {
            if let Some(preset) = preset {
                return preset;
            }
            let mut abilities = Vec::new();
            for task in tasks {
                match task.await {
                    Ok(ability) => abilities.push(ability),
                    Err(error) => {
                        tracing::warn!(%error, "message abilities unknown");
                        return Vec::new();
                    }
                }
            }
            abilities
        }
    }

    /// Asks TDLib what the user may do, then opens the menu at the pointer.
    pub(super) fn open_menu(&mut self, hit: Hit, window: &mut Window, cx: &mut Context<Self>) {
        let Some(message) = self.shown.get(hit.index).cloned() else {
            return;
        };
        if matches!(message.content, Content::Service(_)) {
            return;
        }
        let upon = self.upon(message.id, hit.offset);
        let ids = match (&self.selection, upon) {
            (Selection::Messages(ids), Upon::SelectedMessage) => ids.iter().copied().collect(),
            _ => vec![message.id],
        };
        let abilities_task = self.abilities_of(&ids, cx);
        let is_single = ids.len() == 1;
        let who_read_task = (is_single && message.is_outgoing).then(|| self.who_read(&message, cx));
        let reactors_task = (is_single && !self.is_private(cx))
            .then(|| self.reactors(&message, cx))
            .flatten();
        let strip_task = (is_single && message.schedule.is_none())
            .then(|| self.available_reactions(message.id, cx));
        let position = window.mouse_position();
        let generation = self.generation;
        let id = message.id;
        cx.spawn_in(window, async move |this, cx| {
            // TDLib answers these from its database: the menu opens on them,
            // as tdesktop's does, and the network's "who read" line follows.
            let abilities = abilities_task.await;
            let reactions = match strip_task {
                Some(task) => task.await,
                None => Vec::new(),
            };
            let opened = this.update_in(cx, |this, window, cx| {
                if this.generation != generation {
                    return false;
                }
                let facts = Facts {
                    abilities: &abilities,
                    who_read: None,
                    reactions: reactions.clone(),
                };
                this.show_menu(hit, upon, facts, position, window, cx);
                true
            });
            if !opened.unwrap_or(false) || (who_read_task.is_none() && reactors_task.is_none()) {
                return;
            }
            let who_read = match who_read_task {
                Some(task) => task.await,
                None => None,
            };
            let reactors = match reactors_task {
                Some(task) => task.await,
                None => None,
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let is_open = this
                    .menu
                    .as_ref()
                    .is_some_and(|open| open.message == Some(id));
                if this.generation != generation || !is_open {
                    return;
                }
                let line = this.who_read_line(&message, who_read, reactors, abilities.first(), cx);
                if line.is_none() {
                    return;
                }
                let facts = Facts {
                    abilities: &abilities,
                    who_read: line,
                    reactions,
                };
                this.show_menu(hit, upon, facts, position, window, cx);
            });
        })
        .detach();
    }

    fn upon(&self, message: MessageId, offset: Option<usize>) -> Upon {
        match &self.selection {
            Selection::Messages(ids) if ids.contains(&message) => Upon::SelectedMessage,
            Selection::Messages(_) => Upon::OtherMessages,
            Selection::Text { .. } => match (self.selection.text_range(message), offset) {
                (Some(range), Some(offset)) if range.contains(&offset) => Upon::SelectedText,
                _ => Upon::OtherText,
            },
            Selection::None => Upon::Nothing,
        }
    }

    fn show_menu(
        &mut self,
        hit: Hit,
        upon: Upon,
        facts: Facts<'_>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(message) = self.shown.get(hit.index).cloned() else {
            return;
        };
        let text = match &message.content {
            Content::Text { text, .. } => Some(text),
            _ => None,
        };
        let is_revealed = self.revealed_spoilers.contains(&message.id);
        let link = text.zip(hit.offset).and_then(|(text, offset)| {
            text::links(text, 0..text.text.len(), is_revealed)
                .into_iter()
                .find(|(range, _)| range.contains(&offset))
                .and_then(|(_, link)| LinkCopy::of(&link))
        });
        let abilities = facts.abilities;
        let mut items = items(upon, text.is_some(), link, abilities);
        if field_bar::editable_text(&message).is_none() {
            items.retain(|item| *item != Item::Edit);
        }
        if upon == Upon::Nothing {
            // After Reply and Edit, as tdesktop adds the media's actions.
            let at = items
                .iter()
                .take_while(|i| matches!(i, Item::Reply | Item::Edit))
                .count();
            let media = self.file_items(&message, cx).into_iter();
            items.splice(at..at, media.chain(super::poll::items(&message, abilities)));
        }
        if message.schedule.is_some() {
            items = scheduled_items(items);
        }
        let weak = cx.weak_entity();
        let is_megagroup = self.is_megagroup(cx);
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            if let Some(line) = facts.who_read {
                // shortcut: read dates and viewers are inert; tdesktop's "show when" link comes with privacy settings (M7).
                let on_click = line.on_click;
                menu.action(line.text, line.icon, MenuTone::Normal, move |window, cx| {
                    if let Some(on_click) = &on_click {
                        on_click(window, cx);
                    }
                });
                menu.separator();
            }
            for item in items {
                let (label, icon, tone) = look(&item, is_megagroup, cx);
                let (weak, message, abilities) =
                    (weak.clone(), message.clone(), abilities.to_vec());
                menu.action(label, icon, tone, move |window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.run(&item, &message, hit.index, &abilities, window, cx);
                    });
                });
            }
            menu
        });
        // The "who read" line arriving rebuilds the menu: the strip and
        // the fade go on as they were.
        let kept = self
            .menu
            .take()
            .filter(|open| open.message == Some(message.id))
            .map(|open| (open.strip, open.appear));
        self.present_menu(menu, position, Anchor::TopLeft, window, cx);
        let (strip, appear) = match kept {
            Some((strip, appear)) => (strip, Some(appear)),
            None => (self.strip(message.id, facts.reactions), None),
        };
        if let Some(open) = &mut self.menu {
            open.message = Some(message.id);
            if let Some(appear) = appear {
                open.appear = appear;
            }
            open.strip = strip;
        }
    }

    /// The top bar's chat menu (`TopBarWidget::showPeerMenu`), under the
    /// toggle at the bar's right edge.
    // shortcut: mute, clear history and delete or leave only; info, export, report and the rest come with their features.
    pub(super) fn show_chat_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state();
        let Some(chat) = state.chats.chat(self.chat).cloned() else {
            return;
        };
        let is_self = chat.user().is_some() && chat.user() == state.me;
        let is_muted = state.chats.is_muted(&chat);
        let peer = PeerMenu {
            chat: &chat,
            session: self.store.read(cx).session().clone(),
            layers: self.layers.clone(),
        };
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            if !is_self {
                peer.add_mute(&mut menu, is_muted, cx);
            }
            peer.add_clear_history(&mut menu, cx);
            peer.add_removal(&mut menu, cx);
            menu
        });
        let position = point(
            window.viewport_size().width + px_of(TOP_BAR_MENU_POSITION.x),
            px_of(TOP_BAR_MENU_POSITION.y),
        );
        self.present_menu(menu, position, Anchor::TopRight, window, cx);
    }

    pub(super) fn present_menu(
        &mut self,
        menu: Entity<PopupMenu>,
        position: Point<Pixels>,
        corner: Anchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if menu.read(cx).is_empty() {
            return;
        }
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.menu = Some(OpenMenu {
            position,
            corner,
            menu,
            strip: None,
            message: None,
            appear: {
                let mut appear = Simple::default();
                appear.start(0.0, 1.0, APPEAR_MS, Easing::EaseOutCirc);
                appear
            },
            _subscription: subscription,
        });
        cx.notify();
    }

    fn is_megagroup(&self, cx: &App) -> bool {
        let state = self.store.read(cx).state();
        state.chats.chat(self.chat).is_some_and(|chat| {
            matches!(
                chat.kind,
                ChatKind::Supergroup {
                    is_channel: false,
                    ..
                }
            )
        })
    }

    /// A one-to-one chat, where read dates exist.
    pub(super) fn is_private(&self, cx: &App) -> bool {
        let state = self.store.read(cx).state();
        state
            .chats
            .chat(self.chat)
            .is_some_and(|chat| matches!(chat.kind, ChatKind::Private { .. }))
    }

    fn run(
        &mut self,
        item: &Item,
        message: &Message,
        index: usize,
        abilities: &[Abilities],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match item {
            Item::Reply => self.reply_to(message.id, window, cx),
            Item::Edit => self.edit(message.id, window, cx),
            Item::CopySelectedText | Item::CopySelectedMessages => {
                self.copy_selection(cx);
            }
            Item::CopyText => {
                if let Content::Text { text, .. } = &message.content {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.text.clone()));
                }
            }
            Item::CopyLink(copy) => {
                cx.write_to_clipboard(ClipboardItem::new_string(copy.text().to_owned()));
            }
            Item::CopyMessageLink => self.copy_message_link(message.id, cx),
            Item::Forward => self.forward(vec![message.id], window, cx),
            Item::ForwardSelected => {
                if let Selection::Messages(ids) = &self.selection {
                    let ids = ids.iter().copied().collect();
                    self.forward(ids, window, cx);
                }
            }
            Item::Delete => self.confirm_delete(&[message.id], abilities, window, cx),
            Item::DeleteSelected => {
                if let Selection::Messages(ids) = &self.selection {
                    let ids: Vec<MessageId> = ids.iter().copied().collect();
                    self.confirm_delete(&ids, abilities, window, cx);
                }
            }
            Item::Select => {
                self.selection = Selection::Messages([message.id].into());
                cx.notify();
            }
            Item::SelectUpTo => self.select_up_to(index, cx),
            Item::SendNow => self.send_scheduled_now(message.id, window, cx),
            Item::Reschedule => self.reschedule(message.id, window, cx),
            Item::ShowInFolder(path) => cx.reveal_path(path),
            Item::SaveAs(saving) => self.save_as(saving, cx),
            Item::RetractVote => self.send_votes(message.id, Vec::new(), cx),
            Item::StopPoll => self.confirm_stop_poll(message.id, window, cx),
            Item::ClearSelection => {
                self.clear_selection(cx);
            }
        }
    }

    /// "Copying is not allowed in this ..." for protected content.
    pub(super) fn show_copy_restriction(&self, cx: &mut App) {
        let state = self.store.read(cx).state();
        let key = match state.chats.chat(self.chat).map(|chat| &chat.kind) {
            Some(ChatKind::Private { .. } | ChatKind::Secret { .. }) => key::lng_error_nocopy_user,
            Some(ChatKind::Supergroup {
                is_channel: true, ..
            }) => key::lng_error_nocopy_channel,
            _ => key::lng_error_nocopy_group,
        };
        let text = tr(cx, key);
        self.layers.update(cx, |layers, cx| {
            layers.show_toast(text, &DEFAULT_TOAST, TOAST_DURATION, cx);
        });
    }

    /// Copies the message link and says whether anyone can open it
    /// (`CopyPostLink`).
    fn copy_message_link(&self, message: MessageId, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        let task = spawn(session.message_link(self.chat, message), cx);
        let layers = self.layers.clone();
        cx.spawn(async move |_, cx| {
            let link = match task.await {
                Ok(link) => link,
                Err(error) => {
                    tracing::warn!(%error, "getting a message link failed");
                    return;
                }
            };
            cx.update(|cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(link.link));
                let text = tr(
                    cx,
                    if link.is_public {
                        key::lng_channel_public_link_copied
                    } else {
                        key::lng_context_about_private_link
                    },
                );
                layers.update(cx, |layers, cx| {
                    layers.show_toast(text, &DEFAULT_TOAST, TOAST_DURATION, cx);
                });
            });
        })
        .detach();
    }

    fn delete_place(&self, cx: &App) -> delete::Place {
        let state = self.store.read(cx).state();
        let Some(chat) = state.chats.chat(self.chat) else {
            return delete::Place::Channel;
        };
        match chat.kind {
            ChatKind::Private { user } | ChatKind::Secret { user, .. } => {
                if Some(user) == state.me {
                    return delete::Place::SavedMessages;
                }
                match state.chats.user(user) {
                    Some(user) if user.kind == model::user::UserKind::Bot => delete::Place::Bot,
                    Some(user) => delete::Place::Private(user.first_name.clone()),
                    None => delete::Place::Private(String::new()),
                }
            }
            ChatKind::Group { .. } => delete::Place::Group,
            ChatKind::Supergroup {
                is_channel: true, ..
            } => delete::Place::Channel,
            ChatKind::Supergroup { .. } => delete::Place::Supergroup,
        }
    }

    pub(super) fn confirm_delete(
        &mut self,
        ids: &[MessageId],
        abilities: &[Abilities],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let outgoing: Vec<bool> = ids
            .iter()
            .map(|id| self.shown.iter().any(|m| m.id == *id && m.is_outgoing))
            .collect();
        let offer = delete::offer(&self.delete_place(cx), abilities, &outgoing);
        let weak = cx.weak_entity();
        let session = self.store.read(cx).session().clone();
        delete::show(
            &self.layers,
            delete::Request {
                session,
                chat: self.chat,
                ids: ids.to_vec(),
                offer,
            },
            std::rc::Rc::new(move |_, cx| {
                let _ = weak.update(cx, |this, cx| this.clear_selection(cx));
            }),
            window,
            cx,
        );
    }

    /// The strip over the menu shows every reaction.
    pub(super) fn expand_strip(&mut self) {
        if let Some(strip) = self.menu.as_mut().and_then(|open| open.strip.as_mut()) {
            strip.expand();
        }
    }

    pub(super) fn menu_overlay(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        // Out while it draws, as its frames need the view.
        let taken = self.menu.as_mut()?.strip.take();
        let strip = taken.as_ref().map(|strip| self.strip_element(strip, cx));
        let open = self.menu.as_mut()?;
        open.strip = taken;
        let open = &*open;
        let shown = open.appear.value_in(window, 1.0);
        Some(
            deferred(
                anchored()
                    .anchor(open.corner)
                    .position(open.position)
                    .snap_to_window_with_margin(px(EDGE_MARGIN))
                    // The strip floats over the menu, which keeps its place.
                    // shortcut: a menu at the window's top puts the strip off screen; flip it under the menu when that shows up.
                    .child(
                        div()
                            .relative()
                            .opacity(shown)
                            .top(px((shown - 1.0) * APPEAR_LIFT))
                            .child(open.menu.clone())
                            .children(strip.map(|strip| {
                                div()
                                    .absolute()
                                    // It grows leftwards: menus snap off the right edge.
                                    .right_0()
                                    .bottom(gpui::relative(1.0))
                                    .child(strip)
                            })),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

fn look(item: &Item, is_megagroup: bool, cx: &App) -> (SharedString, style::Icon, MenuTone) {
    let normal = |text, icon| (tr(cx, text), icon, MenuTone::Normal);
    match item {
        Item::Reply => normal(key::lng_context_reply_msg, MENU_ICON_REPLY),
        Item::Edit => normal(key::lng_context_edit_msg, MENU_ICON_EDIT),
        Item::CopySelectedText => normal(key::lng_context_copy_selected, MENU_ICON_COPY),
        Item::CopySelectedMessages => normal(key::lng_context_copy_selected_items, MENU_ICON_COPY),
        Item::CopyText => normal(key::lng_context_copy_text, MENU_ICON_COPY),
        Item::CopyLink(copy) => normal(copy.label(), MENU_ICON_COPY),
        Item::CopyMessageLink if is_megagroup => {
            normal(key::lng_context_copy_message_link, MENU_ICON_LINK)
        }
        Item::CopyMessageLink => normal(key::lng_context_copy_post_link, MENU_ICON_LINK),
        Item::Forward => normal(key::lng_context_forward_msg, MENU_ICON_FORWARD),
        Item::ForwardSelected => normal(key::lng_context_forward_selected, MENU_ICON_FORWARD),
        Item::Delete => (
            tr(cx, key::lng_context_delete_msg),
            MENU_ICON_DELETE_ATTENTION,
            MenuTone::Attention,
        ),
        Item::DeleteSelected => (
            tr(cx, key::lng_context_delete_selected),
            MENU_ICON_DELETE_ATTENTION,
            MenuTone::Attention,
        ),
        Item::Select => normal(key::lng_context_select_msg, MENU_ICON_SELECT),
        Item::SelectUpTo => normal(key::lng_context_select_msg_bulk, MENU_ICON_SELECT),
        Item::ClearSelection => normal(key::lng_context_clear_selection, MENU_ICON_SELECT),
        Item::SendNow => normal(key::lng_context_send_now_msg, MENU_ICON_SEND),
        Item::Reschedule => normal(key::lng_context_reschedule, MENU_ICON_RESCHEDULE),
        Item::ShowInFolder(_) if cfg!(target_os = "macos") => {
            normal(key::lng_context_show_in_finder, MENU_ICON_SHOW_IN_FOLDER)
        }
        Item::ShowInFolder(_) => normal(key::lng_context_show_in_folder, MENU_ICON_SHOW_IN_FOLDER),
        Item::SaveAs(saving) => normal(saving.label, MENU_ICON_DOWNLOAD),
        Item::RetractVote => normal(key::lng_polls_retract, MENU_ICON_RETRACT_VOTE),
        Item::StopPoll => normal(key::lng_polls_stop, MENU_ICON_REMOVE),
    }
}

#[cfg(test)]
#[path = "menu_tests.rs"]
mod tests;
