//! A message's context menu (`HistoryInner::showContextMenu`): copy the
//! selection, the text, a link or the message link, delete, select.

use std::time::Duration;

use chrono::Local;

use gpui::{
    Anchor, AnyElement, App, AppContext as _, ClipboardItem, Context, DismissEvent, Entity,
    Focusable as _, IntoElement, ParentElement, Pixels, Point, SharedString, Subscription, Task,
    Window, anchored, deferred, div, point, px,
};
use i18n::key;
use model::MessageId;
use model::RequestError;
use model::chat::ChatKind;
use model::message::{Content, Message};
use model::message_actions::Abilities;
use model::reaction::AddedReactions;
use style::chat::WHO_READ_CHECKS;
use style::info::TOP_BAR_MENU_POSITION;
use style::menu_icons::{
    MENU_ICON_COPY, MENU_ICON_DELETE_ATTENTION, MENU_ICON_LINK, MENU_ICON_SELECT,
};
use style::widgets::{DEFAULT_TOAST, POPUP_MENU_WITH_ICONS};
use ui::{MenuTone, PopupMenu};

use super::read_date::WhoRead;
use super::selection::Selection;
use super::who_reacted::WhoLine;
use super::{HistoryView, Link, delete, read_date, text};
use crate::lang::{strings, tr};
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
}

/// A who-read answer, or none when TDLib could not tell.
pub(super) fn known<T>(result: Result<T, RequestError>) -> Option<T> {
    result
        .inspect_err(|error| tracing::warn!(%error, "who read the message unknown"))
        .ok()
}

/// A menu over the history, at the window point its `corner` sits on.
pub(super) struct OpenMenu {
    position: Point<Pixels>,
    corner: Anchor,
    menu: Entity<PopupMenu>,
    _subscription: Subscription,
}

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
            | Link::Phone(_)
            | Link::BankCard(_)
            | Link::Date(_)
            | Link::Code(_)
            | Link::Spoiler => {
                return None;
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Item {
    CopySelectedText,
    CopySelectedMessages,
    CopyText,
    CopyLink(LinkCopy),
    CopyMessageLink,
    Delete,
    DeleteSelected,
    Select,
    SelectUpTo,
    ClearSelection,
}

/// The menu's items in tdesktop's order. `abilities` are those of the
/// message, or of every selected one over a selected message; empty when
/// TDLib did not tell, which offers only what needs no permission.
// shortcut: no Reply, Edit, Forward, Pin or Report yet; they come with sending (M4) and the pinned bar.
pub(super) fn items(
    upon: Upon,
    has_text: bool,
    link: Option<LinkCopy>,
    abilities: &[Abilities],
) -> Vec<Item> {
    let can_copy = abilities.iter().all(|a| a.can_be_copied);
    let can_delete = !abilities.is_empty() && abilities.iter().all(Abilities::can_be_deleted);
    let can_get_link = abilities.first().is_some_and(|a| a.can_get_link);
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
            items.extend(can_delete.then_some(Item::DeleteSelected));
            items.push(Item::ClearSelection);
        }
        Upon::OtherMessages => {
            items.extend(link_item(false));
            items.extend([Item::Select, Item::SelectUpTo]);
        }
        Upon::SelectedText | Upon::OtherText | Upon::Nothing => {
            if upon == Upon::SelectedText && can_copy {
                items.push(Item::CopySelectedText);
            }
            if upon == Upon::Nothing && has_text && can_copy {
                items.push(Item::CopyText);
            }
            items.extend(link_item(true));
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

impl HistoryView {
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
        let session = self.store.read(cx).session().clone();
        let tasks: Vec<_> = ids
            .iter()
            .map(|id| spawn(session.message_abilities(self.chat, *id), cx))
            .collect();
        let is_single = ids.len() == 1;
        let who_read_task = (is_single && message.is_outgoing).then(|| self.who_read(&message, cx));
        let reactors_task = (is_single && !self.is_private(cx))
            .then(|| self.reactors(&message, cx))
            .flatten();
        let position = window.mouse_position();
        let generation = self.generation;
        cx.spawn_in(window, async move |this, cx| {
            let mut abilities = Vec::new();
            for task in tasks {
                match task.await {
                    Ok(ability) => abilities.push(ability),
                    Err(error) => {
                        tracing::warn!(%error, "message abilities unknown");
                        abilities.clear();
                        break;
                    }
                }
            }
            let who_read = match who_read_task {
                Some(task) => task.await,
                None => None,
            };
            let reactors = match reactors_task {
                Some(task) => task.await,
                None => None,
            };
            this.update_in(cx, |this, window, cx| {
                if this.generation == generation {
                    let facts = Facts {
                        abilities: &abilities,
                        who_read: this.who_read_line(
                            &message,
                            who_read,
                            reactors,
                            abilities.first(),
                            cx,
                        ),
                    };
                    this.show_menu(hit, upon, facts, position, window, cx);
                }
            })
        })
        .detach();
    }

    /// Asks who saw an outgoing message alongside its abilities: the read
    /// date in private chats, the viewers in groups.
    fn who_read(&self, message: &Message, cx: &App) -> Task<Option<WhoRead>> {
        let session = self.store.read(cx).session();
        if self.is_private(cx) {
            let task = session.message_read_date(self.chat, message.id);
            spawn(async move { known(task.await).map(WhoRead::Date) }, cx)
        } else {
            let task = session.message_viewers(self.chat, message.id);
            spawn(async move { known(task.await).map(WhoRead::Viewers) }, cx)
        }
    }

    /// The menu's first line, when the abilities allow asking.
    fn who_read_line(
        &self,
        message: &Message,
        who_read: Option<WhoRead>,
        reactors: Option<AddedReactions>,
        abilities: Option<&Abilities>,
        cx: &App,
    ) -> Option<WhoLine> {
        let abilities = abilities?;
        let viewers = match who_read {
            Some(WhoRead::Date(date)) if abilities.can_get_read_date => {
                return read_date::text(strings(cx), date, &Local::now()).map(|text| WhoLine {
                    text,
                    icon: WHO_READ_CHECKS,
                    on_click: None,
                });
            }
            Some(WhoRead::Viewers(viewers)) if abilities.can_get_viewers => Some(viewers),
            _ => None,
        };
        self.who_reacted_line(message, viewers, reactors, cx)
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
        let items = items(upon, text.is_some(), link, abilities);
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
        self.present_menu(menu, position, Anchor::TopLeft, window, cx);
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
    fn is_private(&self, cx: &App) -> bool {
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

    fn confirm_delete(
        &mut self,
        ids: &[MessageId],
        abilities: &[Abilities],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.store.read(cx).state();
        let outgoing: Vec<bool> = ids
            .iter()
            .map(|id| self.shown.iter().any(|m| m.id == *id && m.is_outgoing))
            .collect();
        let peer = state
            .chats
            .chat(self.chat)
            .and_then(|chat| chat.user())
            .filter(|user| Some(*user) != state.me)
            .and_then(|user| state.chats.user(user))
            .map(|user| user.first_name.clone());
        let revoke = delete::revoke(abilities, &outgoing, peer);
        let weak = cx.weak_entity();
        let session = self.store.read(cx).session().clone();
        delete::show(
            &self.layers,
            delete::Request {
                session,
                chat: self.chat,
                ids: ids.to_vec(),
                revoke,
            },
            std::rc::Rc::new(move |_, cx| {
                let _ = weak.update(cx, |this, cx| this.clear_selection(cx));
            }),
            window,
            cx,
        );
    }

    pub(super) fn menu_overlay(&self) -> Option<AnyElement> {
        let open = self.menu.as_ref()?;
        Some(
            deferred(
                anchored()
                    .anchor(open.corner)
                    .position(open.position)
                    .snap_to_window_with_margin(px(EDGE_MARGIN))
                    .child(div().child(open.menu.clone())),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

fn look(item: &Item, is_megagroup: bool, cx: &App) -> (SharedString, style::Icon, MenuTone) {
    let normal = |text, icon| (tr(cx, text), icon, MenuTone::Normal);
    match item {
        Item::CopySelectedText => normal(key::lng_context_copy_selected, MENU_ICON_COPY),
        Item::CopySelectedMessages => normal(key::lng_context_copy_selected_items, MENU_ICON_COPY),
        Item::CopyText => normal(key::lng_context_copy_text, MENU_ICON_COPY),
        Item::CopyLink(copy) => normal(copy.label(), MENU_ICON_COPY),
        Item::CopyMessageLink if is_megagroup => {
            normal(key::lng_context_copy_message_link, MENU_ICON_LINK)
        }
        Item::CopyMessageLink => normal(key::lng_context_copy_post_link, MENU_ICON_LINK),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Abilities = Abilities {
        can_be_copied: true,
        can_be_deleted_only_for_self: true,
        can_be_deleted_for_all_users: true,
        can_get_link: true,
        can_be_pinned: true,
        can_get_read_date: false,
        can_get_viewers: false,
    };

    #[test]
    fn a_plain_message_offers_copy_link_delete_and_select() {
        assert_eq!(
            items(Upon::Nothing, true, None, &[ALL]),
            [
                Item::CopyText,
                Item::CopyMessageLink,
                Item::Delete,
                Item::Select
            ]
        );
    }

    #[test]
    fn a_link_under_the_pointer_replaces_the_message_link() {
        let link = LinkCopy::Hashtag("#trail".to_owned());
        assert_eq!(
            items(Upon::Nothing, true, Some(link.clone()), &[ALL]),
            [
                Item::CopyText,
                Item::CopyLink(link),
                Item::Delete,
                Item::Select
            ]
        );
    }

    #[test]
    fn unknown_abilities_offer_only_what_needs_no_permission() {
        assert_eq!(
            items(Upon::Nothing, true, None, &[]),
            [Item::CopyText, Item::Select]
        );
    }

    #[test]
    fn protected_content_is_not_copied() {
        let protected = Abilities {
            can_be_copied: false,
            ..ALL
        };
        assert_eq!(
            items(Upon::SelectedText, true, None, &[protected]),
            [Item::CopyMessageLink, Item::Delete, Item::Select]
        );
    }

    #[test]
    fn selected_messages_delete_only_when_all_can() {
        let kept = Abilities {
            can_be_deleted_only_for_self: false,
            can_be_deleted_for_all_users: false,
            ..ALL
        };
        assert_eq!(
            items(Upon::SelectedMessage, true, None, &[ALL, kept]),
            [Item::CopySelectedMessages, Item::ClearSelection]
        );
        assert_eq!(
            items(Upon::OtherMessages, true, None, &[ALL]),
            [Item::Select, Item::SelectUpTo]
        );
    }
}
