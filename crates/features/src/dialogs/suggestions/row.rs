//! A suggestions row and what it shows of its chat.

use chrono::Local;
use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, div, prelude::*,
};
use i18n::key;
use model::ChatId;
use model::chat::{Chat, ChatKind};
use model::user::UserKind;
use style::boxes::CONTACTS_STATUS_FONT;
use style::dialogs::{
    DIALOGS_ONLINE_BADGE_SIZE, DIALOGS_ONLINE_BADGE_SKIP, DIALOGS_ONLINE_BADGE_STROKE,
    RECENT_PEERS_ITEM, RECENT_PEERS_SPECIAL_NAME,
};
use style::{ColorId, ColorRef};
use ui::{Userpic, UserpicSource, color, font, font_height};

use super::super::row::{Lit, unread_badge};
use super::super::{Dialogs, RowKey, photo_userpic};
use super::chats::Removable;
use crate::lang::strings;
use crate::px_of;
use crate::status::status;

/// Whether a row shows its chat's unread counter: recent picks do, joined
/// and recommended channels do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Counter {
    Shown,
    Hidden,
}

/// What a right click on a row opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowMenu {
    /// Remove from Recent and Clear all.
    Recent,
    /// The chat list's own menu for the chat.
    Chat,
    None,
}

/// What a suggestion row shows of its chat.
pub(super) struct Peer {
    pub name: SharedString,
    pub userpic: UserpicSource,
    pub status: Option<(String, bool)>,
    pub unread: Option<(SharedString, bool)>,
    pub is_online: bool,
    pub is_self: bool,
}

impl Dialogs {
    pub(super) fn peer(&self, chat: &Chat, cx: &App) -> (Peer, Option<model::FileId>) {
        let state = self.store.read(cx).state();
        let strings = strings(cx);
        let user = chat.user().and_then(|user| state.chats.user(user));
        let is_self = matches!(chat.kind, ChatKind::Private { .. }) && chat.user() == state.me;
        let (userpic, download) = if is_self {
            (UserpicSource::SavedMessages, None)
        } else {
            photo_userpic(
                &state.chats,
                chat.photo.as_ref(),
                chat.accent_color,
                &chat.title,
            )
        };
        let name: SharedString = if is_self {
            strings.text(key::lng_saved_messages).into()
        } else if user.is_some_and(|user| user.kind == UserKind::Deleted) {
            strings.text(key::lng_deleted).into()
        } else {
            chat.title.clone().into()
        };
        let now = Local::now();
        let status = if is_self {
            None
        } else if let Some(user) = user {
            let line = status(strings, user, &now);
            Some((line.text, line.is_online))
        } else {
            crate::status::members_status(chat, state.chats.member_count(chat), cx)
                .map(|text| (text, false))
        };
        let unread = (!is_self)
            .then(|| {
                let muted = state.chats.is_muted(chat);
                (chat.unread_count > 0)
                    .then(|| SharedString::from(chat.unread_count.to_string()))
                    .or_else(|| chat.is_marked_as_unread.then(SharedString::default))
                    .map(|count| (count, muted))
            })
            .flatten();
        let is_online = !is_self
            && user.is_some_and(|user| {
                user.is_online(i32::try_from(now.timestamp()).unwrap_or(i32::MAX))
            });
        (
            Peer {
                name,
                userpic,
                status,
                unread,
                is_online,
                is_self,
            },
            download,
        )
    }

    /// A chat in a suggestions list (`RecentRow`, `ChannelRow`): userpic,
    /// name, status and, with [`Counter::Shown`], its unread counter.
    pub(super) fn peer_row(
        &mut self,
        id: ChatId,
        counter: Counter,
        menu: RowMenu,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(chat) = self.store.read(cx).state().chats.chat(id).cloned() else {
            return div().into_any_element();
        };
        let (peer, download) = self.peer(&chat, cx);
        if let Some(file) = download {
            self.download(file, cx);
        }
        let st = if peer.is_self {
            &RECENT_PEERS_SPECIAL_NAME
        } else {
            &RECENT_PEERS_ITEM
        };
        let key = RowKey::Suggested(id);
        let is_hovered = self.hovered == Some(key);
        let name_font = st.name_style.font;
        let status_font = CONTACTS_STATUS_FONT;
        let photo = px_of(st.photo_size);
        let bg = if is_hovered {
            st.button.text_bg_over
        } else {
            st.button.text_bg
        };
        div()
            .id(key.element_id())
            .relative()
            .w_full()
            .h(px_of(st.height))
            .flex()
            .items_center()
            .justify_end()
            .pr(px_of(st.photo_position.x))
            .bg(color(cx, bg))
            .child(
                div()
                    .absolute()
                    .left(px_of(st.photo_position.x))
                    .top(px_of(st.photo_position.y))
                    .size(photo)
                    .child(Userpic::new(photo, peer.userpic.clone()))
                    .when(peer.is_online, |userpic| {
                        let ring = match bg {
                            ColorRef::Palette(id) => id,
                            _ => ColorId::DialogsBg,
                        };
                        userpic.child(online_dot(photo, ring, cx))
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left(px_of(st.name_position.x))
                    .top(px_of(st.name_position.y))
                    .right(px_of(st.photo_position.x))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .font(font(name_font))
                    .text_size(px_of(name_font.size))
                    .line_height(font_height(name_font))
                    .text_color(color(cx, st.name_fg))
                    .child(peer.name),
            )
            .children(peer.status.map(|(text, is_online)| {
                let fg = if is_online {
                    st.status_fg_active
                } else if is_hovered {
                    st.status_fg_over
                } else {
                    st.status_fg
                };
                div()
                    .absolute()
                    .left(px_of(st.status_position.x))
                    .top(px_of(st.status_position.y))
                    .whitespace_nowrap()
                    .font(font(status_font))
                    .text_size(px_of(status_font.size))
                    .line_height(font_height(status_font))
                    .text_color(color(cx, fg))
                    .child(text)
            }))
            .children(
                peer.unread
                    .filter(|_| counter == Counter::Shown)
                    .map(|(count, is_muted)| unread_badge(count, is_muted, Lit::Normal, cx)),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.track_hover(key, *hovered, cx);
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.choose_suggested(id, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| match menu {
                    RowMenu::Recent => {
                        this.show_suggestion_menu(
                            Removable::Recent(id),
                            event.position,
                            window,
                            cx,
                        );
                    }
                    RowMenu::Chat => this.show_menu(id, event.position, window, cx),
                    RowMenu::None => {}
                }),
            )
            .into_any_element()
    }
}

/// "12 members" or "48,210 subscribers", once the count is known.
/// The green dot at a userpic's bottom right, ringed in `ring`.
pub(super) fn online_dot(photo: Pixels, ring: ColorId, cx: &App) -> impl IntoElement {
    let size = px_of(DIALOGS_ONLINE_BADGE_SIZE);
    let stroke = px_of(DIALOGS_ONLINE_BADGE_STROKE);
    let half = stroke / 2.0;
    div()
        .absolute()
        .left(photo - px_of(DIALOGS_ONLINE_BADGE_SKIP.x) - size - half)
        .top(photo - px_of(DIALOGS_ONLINE_BADGE_SKIP.y) - size - half)
        .size(size + stroke)
        .rounded_full()
        .bg(color(cx, ColorRef::Palette(ring)))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(size - stroke)
                .rounded_full()
                .bg(color(cx, ColorRef::Palette(ColorId::DialogsOnlineBadgeFg))),
        )
}
