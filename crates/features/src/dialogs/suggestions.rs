//! What an empty, focused search shows (`Dialogs::Suggestions`, its chats
//! tab): the frequent contacts strip (`TopPeersStrip`) and the chats the
//! user picked from earlier searches (`RecentsController`).

use std::rc::Rc;

use chrono::Local;
use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::*, px,
};
use i18n::key;
use model::ChatId;
use model::chat::{Chat, ChatKind};
use model::user::UserKind;
use style::boxes::CONTACTS_STATUS_FONT;
use style::dialogs::{
    DIALOGS_EMPTY_LABEL, DIALOGS_ONLINE_BADGE_SIZE, DIALOGS_ONLINE_BADGE_SKIP,
    DIALOGS_ONLINE_BADGE_STROKE, DIALOGS_UNREAD_HEIGHT, RECENT_PEERS_EMPTY_SKIP, RECENT_PEERS_ITEM,
    RECENT_PEERS_SPECIAL_NAME, TOP_PEERS, TOP_PEERS_MARGIN, TOP_PEERS_RADIUS,
};
use style::menu_icons::{MENU_ICON_CANCEL_ATTENTION, MENU_ICON_DELETE_ATTENTION};
use style::{ColorId, ColorRef};
use ui::{Markup, MenuTone, Userpic, UserpicSource, color, font, font_height};

use super::menu::send;
use super::row::{Lit, unread_badge};
use super::search::{bar, bar_link};
use super::{Dialogs, RowKey, photo_userpic, preview};
use crate::boxes::{self, Callback, ConfirmArgs};
use crate::lang::{strings, tr};
use crate::px_of;
use crate::status::status;
use crate::store::spawn;

/// TDLib returns at most this many top chats.
const TOP_LIMIT: i32 = 30;
/// tdesktop keeps this many recent search picks (`RecentPeers` `kLimit`).
const RECENT_LIMIT: i32 = 48;

#[derive(Debug, Default)]
pub(super) struct Suggestions {
    top: Vec<ChatId>,
    recent: Vec<ChatId>,
}

/// What a suggestion row shows of its chat.
struct Peer {
    name: SharedString,
    userpic: UserpicSource,
    status: Option<(String, bool)>,
    unread: Option<(SharedString, bool)>,
    is_online: bool,
    is_self: bool,
}

impl Dialogs {
    /// Shows the suggestions while the search field is focused and empty.
    pub(crate) fn show_suggestions(&mut self, cx: &mut Context<Self>) {
        if self.suggestions.is_some() {
            return;
        }
        self.suggestions = Some(Suggestions::default());
        let session = self.store.read(cx).session().clone();
        self.when_suggested(
            spawn(session.top_chats(TOP_LIMIT), cx),
            |s, top| s.top = top,
            cx,
        );
        self.when_suggested(
            spawn(session.recently_found_chats(RECENT_LIMIT), cx),
            |s, recent| s.recent = recent,
            cx,
        );
        cx.notify();
    }

    pub(crate) fn hide_suggestions(&mut self, cx: &mut Context<Self>) {
        if self.suggestions.take().is_some() {
            cx.notify();
        }
    }

    fn when_suggested(
        &self,
        task: gpui::Task<Result<Vec<ChatId>, model::RequestError>>,
        apply: fn(&mut Suggestions, Vec<ChatId>),
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(chats) => {
                    if let Some(suggestions) = &mut this.suggestions {
                        apply(suggestions, chats);
                        cx.notify();
                    }
                }
                Err(error) => tracing::warn!(%error, "loading search suggestions failed"),
            })
            .ok();
        })
        .detach();
    }

    /// Puts `chat` first in the recent searches (`RecentPeers::bump`).
    pub(super) fn remember_found(&self, chat: ChatId, cx: &mut App) {
        let task = self.store.read(cx).session().add_recently_found_chat(chat);
        send(task, "adding a recent search", cx);
    }

    fn choose_suggested(&mut self, chat: ChatId, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_found(chat, cx);
        self.select(chat, cx);
        // Leaving the field hides the suggestions, as opening a chat does.
        self.main_focus.focus(window, cx);
    }

    pub(super) fn suggestions_view(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let suggestions = self.suggestions.as_ref()?;
        let (top, recent) = (suggestions.top.clone(), suggestions.recent.clone());
        let mut column = div()
            .id("suggestions")
            .size_full()
            .overflow_y_scroll()
            .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
            // Clicks here keep the search focused, so the suggestions stay.
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_mouse_down(MouseButton::Right, |_, window, _| window.prevent_default());
        if !top.is_empty() {
            // shortcut: the strip scrolls sideways; tdesktop's "Show more" grid expansion comes later in M2.
            column = column
                .child(bar(tr(cx, key::lng_recent_frequent).to_string(), None, cx))
                .child(
                    div()
                        .id("top-peers")
                        .flex()
                        .overflow_x_scroll()
                        .children(top.into_iter().map(|chat| self.top_peer(chat, cx))),
                );
        }
        if recent.is_empty() {
            column = column.child(
                div()
                    .flex()
                    .justify_center()
                    .pt(px_of(RECENT_PEERS_EMPTY_SKIP) * 4.0)
                    .child(ui::FlatLabel::new(
                        "recent-none",
                        &DIALOGS_EMPTY_LABEL,
                        Markup::plain(tr(cx, key::lng_recent_none).to_string()),
                    )),
            );
        } else {
            let clear = bar_link(
                "recent-clear",
                tr(cx, key::lng_recent_clear),
                cx,
                |this, _, window, cx| {
                    this.confirm_clear_recent(window, cx);
                },
            );
            column = column
                .child(bar(
                    tr(cx, key::lng_recent_title).to_string(),
                    Some(clear),
                    cx,
                ))
                .children(recent.into_iter().map(|chat| self.recent_row(chat, cx)));
        }
        Some(column.into_any_element())
    }

    fn peer(&self, chat: &Chat, cx: &App) -> (Peer, Option<model::FileId>) {
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
        // shortcut: groups and channels show no member count until supergroup info lands with profiles in M6.
        let status = user.filter(|_| !is_self).map(|user| {
            let line = status(strings, user, &now);
            (line.text, line.is_online)
        });
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
                    this.show_suggestion_menu(Suggested::Top(id), event.position, window, cx);
                }),
            )
            .into_any_element()
    }

    /// A recent search pick (`RecentRow`): userpic, name, status and its
    /// unread counter at the right.
    fn recent_row(&mut self, id: ChatId, cx: &mut Context<Self>) -> AnyElement {
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
        let key = RowKey::Recent(id);
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
                    .map(|(count, is_muted)| unread_badge(count, is_muted, Lit::Normal, cx)),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.track_hover(key, *hovered, cx);
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.choose_suggested(id, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.show_suggestion_menu(Suggested::Recent(id), event.position, window, cx);
                }),
            )
            .into_any_element()
    }

    /// Remove one (`FillEntryMenu`'s last items); recent picks can also be
    /// cleared all at once.
    // shortcut: no open, new window, add to folder or profile items until history (M3) and profiles (M6).
    fn show_suggestion_menu(
        &mut self,
        what: Suggested,
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
            if let Suggested::Recent(_) = what {
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

    fn remove_suggested(&mut self, what: Suggested, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        let Some(suggestions) = &mut self.suggestions else {
            return;
        };
        match what {
            Suggested::Top(chat) => {
                suggestions.top.retain(|known| *known != chat);
                send(session.remove_top_chat(chat), "removing a top chat", cx);
            }
            Suggested::Recent(chat) => {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suggested {
    Top(ChatId),
    Recent(ChatId),
}

/// The green dot at a userpic's bottom right, ringed in `ring`.
fn online_dot(photo: Pixels, ring: ColorId, cx: &App) -> impl IntoElement {
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
