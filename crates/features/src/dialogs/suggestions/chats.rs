//! The suggestions' chats tab: frequent contacts (`TopPeersStrip`) and
//! recent search picks (`RecentsController`).

use std::rc::Rc;

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::*, px,
};
use i18n::key;
use model::ChatId;
use model::search::TopChats;
use style::dialogs::{
    DIALOGS_EMPTY_LABEL, DIALOGS_UNREAD_HEIGHT, RECENT_PEERS_EMPTY_SKIP, TOP_PEERS,
    TOP_PEERS_MARGIN, TOP_PEERS_RADIUS,
};
use style::menu_icons::{MENU_ICON_CANCEL_ATTENTION, MENU_ICON_DELETE_ATTENTION};
use style::{ColorId, ColorRef};
use ui::{Markup, MenuTone, Userpic, color, font, font_height};

use super::super::menu::send;
use super::super::row::{Lit, unread_badge};
use super::super::search::{bar, bar_link};
use super::super::{Dialogs, RowKey, preview};
use super::row::{Counter, RowMenu, online_dot};
use crate::boxes::{self, Callback, ConfirmArgs};
use crate::lang::{strings, tr};
use crate::px_of;

/// What the remove item of a suggestion's menu removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Removable {
    Top(ChatId),
    Recent(ChatId),
}

impl Dialogs {
    pub(super) fn chats_tab(
        &mut self,
        top: Vec<ChatId>,
        recent: Vec<ChatId>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut children = Vec::new();
        if !top.is_empty() {
            // shortcut: the strip scrolls sideways; tdesktop's "Show more" grid expansion comes later in M2.
            children.push(bar(tr(cx, key::lng_recent_frequent).to_string(), None, cx));
            children.push(
                div()
                    .id("top-peers")
                    .flex()
                    .overflow_x_scroll()
                    .children(top.into_iter().map(|chat| self.top_peer(chat, cx)))
                    .into_any_element(),
            );
        }
        if recent.is_empty() {
            children.push(empty_text("recent-none", tr(cx, key::lng_recent_none)));
            return children;
        }
        let clear = bar_link(
            "recent-clear",
            tr(cx, key::lng_recent_clear),
            cx,
            |this, _, window, cx| this.confirm_clear_recent(window, cx),
        );
        children.push(bar(
            tr(cx, key::lng_recent_title).to_string(),
            Some(clear),
            cx,
        ));
        children.extend(
            recent
                .into_iter()
                .map(|chat| self.peer_row(chat, Counter::Shown, RowMenu::Recent, cx)),
        );
        children
    }

    /// One entry of the strip: userpic with its online dot or counter,
    /// and the short name under it.
    fn top_peer(&mut self, id: ChatId, cx: &mut Context<Self>) -> AnyElement {
        let Some(chat) = self.store.read(cx).state().chats.chat(id).cloned() else {
            return div().into_any_element();
        };
        let (peer, download) = self.peer(&chat, cx);
        if let Some(file) = download {
            self.download(file, cx);
        }
        let short_name: SharedString = match chat.user() {
            Some(user) if !peer.is_self => self
                .store
                .read(cx)
                .state()
                .chats
                .user(user)
                .map_or(peer.name.clone(), |user| {
                    preview::short_name(strings(cx), user).into()
                }),
            _ => peer.name.clone(),
        };
        let st = &TOP_PEERS;
        let key = RowKey::Top(id);
        let is_hovered = self.hovered == Some(key);
        let photo = px_of(st.photo);
        let single = px_of(st.photo_left) * 2.0 + photo;
        let name_font = st.name_style.font;
        let ring = if is_hovered {
            ColorId::WindowBgOver
        } else {
            ColorId::DialogsBg
        };
        div()
            .id(key.element_id())
            .relative()
            .flex_none()
            .w(single)
            .h(px_of(st.height))
            .when(is_hovered, |entry| {
                entry.child(
                    div()
                        .absolute()
                        .left(px_of(TOP_PEERS_MARGIN.left))
                        .top(px_of(TOP_PEERS_MARGIN.top))
                        .right(px_of(TOP_PEERS_MARGIN.right))
                        .bottom(px_of(TOP_PEERS_MARGIN.bottom))
                        .rounded(px_of(TOP_PEERS_RADIUS))
                        .bg(color(cx, ColorRef::Palette(ColorId::WindowBgOver))),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(px_of(st.photo_left))
                    .top(px_of(st.photo_top))
                    .size(photo)
                    .child(Userpic::new(photo, peer.userpic.clone()))
                    .when(peer.is_online, |userpic| {
                        userpic.child(online_dot(photo, ring, cx))
                    })
                    // The counter sits on the userpic's top right corner.
                    .children(peer.unread.clone().map(|(count, is_muted)| {
                        div()
                            .absolute()
                            .right(px(-1.0))
                            .top(px(-1.0))
                            .p(px(1.0))
                            .rounded(px_of(DIALOGS_UNREAD_HEIGHT))
                            .bg(color(cx, ColorRef::Palette(ring)))
                            .child(unread_badge(count, is_muted, Lit::Normal, cx))
                    })),
            )
            .child(
                div()
                    .absolute()
                    .left(px_of(st.name_left))
                    .right(px_of(st.name_left))
                    .top(px_of(st.name_top))
                    .text_center()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .font(font(name_font))
                    .text_size(px_of(name_font.size))
                    .line_height(font_height(name_font))
                    .text_color(color(cx, ColorRef::Palette(ColorId::DialogsNameFg)))
                    .child(short_name),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.track_hover(key, *hovered, cx);
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.choose_suggested(id, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.show_suggestion_menu(Removable::Top(id), event.position, window, cx);
                }),
            )
            .into_any_element()
    }

    /// Remove one (`FillEntryMenu`'s last items); recent picks can also be
    /// cleared all at once.
    // shortcut: no open, new window, add to folder or profile items until history (M3) and profiles (M6).
    pub(super) fn show_suggestion_menu(
        &mut self,
        what: Removable,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.entity().downgrade();
        self.open_menu(position, window, cx, move |_, menu, cx| {
            let remove = weak.clone();
            menu.action(
                tr(cx, key::lng_recent_remove),
                MENU_ICON_DELETE_ATTENTION,
                MenuTone::Attention,
                move |_, cx| {
                    remove
                        .update(cx, |this, cx| this.remove_suggested(what, cx))
                        .ok();
                },
            );
            if let Removable::Recent(_) = what {
                let clear = weak.clone();
                menu.action(
                    tr(cx, key::lng_recent_clear_all),
                    MENU_ICON_CANCEL_ATTENTION,
                    MenuTone::Attention,
                    move |window, cx| {
                        clear
                            .update(cx, |this, cx| this.confirm_clear_recent(window, cx))
                            .ok();
                    },
                );
            }
        });
    }

    fn remove_suggested(&mut self, what: Removable, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        let Some(suggestions) = &mut self.suggestions else {
            return;
        };
        match what {
            Removable::Top(chat) => {
                suggestions.top.retain(|known| *known != chat);
                send(
                    session.remove_top_chat(TopChats::Users, chat),
                    "removing a top chat",
                    cx,
                );
            }
            Removable::Recent(chat) => {
                suggestions.recent.retain(|known| *known != chat);
                send(
                    session.remove_recently_found_chat(chat),
                    "removing a recent search",
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn confirm_clear_recent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let confirmed: Callback = Rc::new(move |_, cx| {
            weak.update(cx, |this, cx| {
                if let Some(suggestions) = &mut this.suggestions {
                    suggestions.recent.clear();
                }
                let task = this.store.read(cx).session().clear_recently_found_chats();
                send(task, "clearing recent searches", cx);
                cx.notify();
            })
            .ok();
        });
        let mut args = ConfirmArgs::new(Markup::plain(
            tr(cx, key::lng_recent_clear_sure).to_string(),
        ));
        args.cancel_text = Some(tr(cx, key::lng_cancel));
        args.confirmed = Some(confirmed);
        boxes::confirm(&self.layers, args, window, cx);
    }
}

/// A centered grey text where a list is empty (`SearchEmpty`).
// shortcut: no animated sticker above the text until animated stickers land in M5.
pub(super) fn empty_text(id: &'static str, text: SharedString) -> AnyElement {
    div()
        .flex()
        .justify_center()
        .pt(px_of(RECENT_PEERS_EMPTY_SKIP) * 4.0)
        .child(ui::FlatLabel::new(
            id,
            &DIALOGS_EMPTY_LABEL,
            Markup::plain(text.to_string()),
        ))
        .into_any_element()
}
