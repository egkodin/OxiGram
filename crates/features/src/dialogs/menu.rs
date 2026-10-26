//! A chat row's context menu (`Window::FillDialogsEntryMenu`): pin, mute,
//! read state, archive, block, clear and delete.

use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext as _, Context, DismissEvent, Focusable as _, IntoElement,
    ParentElement, Pixels, Point, Window, anchored, deferred, div, px,
};
use i18n::key;
use model::ChatId;
use model::chat::{Chat, ChatList};
use model::chat_actions::{Pinning, UnreadMark};
use model::user::UserKind;
use style::menu_icons::{
    MENU_ICON_ARCHIVE, MENU_ICON_BLOCK, MENU_ICON_COLLAPSE, MENU_ICON_EXPAND, MENU_ICON_MARK_READ,
    MENU_ICON_MARK_UNREAD, MENU_ICON_PIN, MENU_ICON_TO_MAIN_MENU, MENU_ICON_UNARCHIVE,
    MENU_ICON_UNPIN,
};
use style::widgets::POPUP_MENU_WITH_ICONS;
use style::window::WINDOW_ARCHIVE_TOAST;
use ui::{MenuTone, PopupMenu};

use super::Dialogs;
use crate::boxes;
use crate::lang::{tr, tr_with};
use crate::peer_menu::{PeerMenu, on_confirm, send};
use crate::settings;

/// Keeps the menu off the window edges.
const EDGE_MARGIN: f32 = 8.0;
/// The "moved to the main menu" toast stays longer (`kArchivedToastDuration`).
const ARCHIVE_TOAST_DURATION: Duration = Duration::from_secs(5);

impl Dialogs {
    pub(super) fn show_menu(
        &mut self,
        chat: ChatId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(chat) = self.store.read(cx).state().chats.chat(chat).cloned() else {
            return;
        };
        self.open_menu(position, window, cx, |this, menu, cx| {
            this.fill_menu(menu, &chat, cx);
        });
    }

    /// The archive row's menu (`Filler::fillArchiveActions`).
    pub(super) fn show_archive_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu(position, window, cx, |this, menu, cx| {
            this.fill_archive_menu(menu, cx);
        });
    }

    pub(super) fn open_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
        fill: impl FnOnce(&Self, &mut PopupMenu, &App),
    ) {
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            fill(self, &mut menu, cx);
            menu
        });
        if menu.read(cx).is_empty() {
            return;
        }
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.menu = Some((position, menu, subscription));
        cx.notify();
    }

    fn fill_archive_menu(&self, menu: &mut PopupMenu, cx: &App) {
        let (text, icon) = if settings::is_archive_collapsed(cx) {
            (key::lng_context_archive_expand, MENU_ICON_EXPAND)
        } else {
            (key::lng_context_archive_collapse, MENU_ICON_COLLAPSE)
        };
        let collapsed = !settings::is_archive_collapsed(cx);
        menu.action(tr(cx, text), icon, MenuTone::Normal, move |_, cx| {
            settings::set_archive_collapsed(collapsed, cx);
        });
        let layers = self.layers.clone();
        menu.action(
            tr(cx, key::lng_context_archive_to_menu),
            MENU_ICON_TO_MAIN_MENU,
            MenuTone::Normal,
            move |_, cx| {
                let text = tr(cx, key::lng_context_archive_to_menu_info);
                layers.update(cx, |layers, cx| {
                    layers.show_toast(text, &WINDOW_ARCHIVE_TOAST, ARCHIVE_TOAST_DURATION, cx);
                });
                settings::set_archive_in_main_menu(true, cx);
            },
        );
        self.add_mark_read(menu, ChatList::Archive, cx);
    }

    pub(super) fn menu_overlay(&self) -> Option<AnyElement> {
        let (position, menu, _) = self.menu.as_ref()?;
        Some(
            deferred(
                anchored()
                    .position(*position)
                    .snap_to_window_with_margin(px(EDGE_MARGIN))
                    .child(div().child(menu.clone())),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn fill_menu(&self, menu: &mut PopupMenu, chat: &Chat, cx: &App) {
        let state = self.store.read(cx).state();
        let session = self.store.read(cx).session().clone();
        let id = chat.id;
        let user = chat.user().and_then(|user| state.chats.user(user));
        let is_self = chat.user().is_some() && chat.user() == state.me;
        let list = self.list;
        let peer = PeerMenu {
            chat,
            session: session.clone(),
            layers: self.layers.clone(),
        };

        if !is_self {
            let to = if chat.position(ChatList::Archive).is_some() {
                (
                    key::lng_archived_remove,
                    MENU_ICON_UNARCHIVE,
                    ChatList::Main,
                )
            } else {
                (key::lng_archived_add, MENU_ICON_ARCHIVE, ChatList::Archive)
            };
            let (text, icon, target) = to;
            let session = session.clone();
            menu.action(tr(cx, text), icon, MenuTone::Normal, move |_, cx| {
                send(session.move_chat_to(id, target), "archive", cx);
            });
        }

        let is_pinned = chat
            .position(list)
            .is_some_and(|position| position.is_pinned);
        let (text, icon, pinning) = if is_pinned {
            (
                key::lng_context_unpin_from_top,
                MENU_ICON_UNPIN,
                Pinning::Unpinned,
            )
        } else {
            (key::lng_context_pin_to_top, MENU_ICON_PIN, Pinning::Pinned)
        };
        {
            let session = session.clone();
            // shortcut: a failed pin over the limit is only logged; the Premium limit box comes with M13.
            menu.action(tr(cx, text), icon, MenuTone::Normal, move |_, cx| {
                send(session.set_chat_pinned(list, id, pinning), "pin", cx);
            });
        }

        if !is_self {
            peer.add_mute(menu, state.chats.is_muted(chat), cx);
        }

        let is_unread = chat.unread_count > 0 || chat.is_marked_as_unread;
        {
            let session = session.clone();
            let last = chat.last_message.as_ref().map(|message| message.id);
            if is_unread {
                menu.action(
                    tr(cx, key::lng_context_mark_read),
                    MENU_ICON_MARK_READ,
                    MenuTone::Normal,
                    move |_, cx| send(session.read_chat(id, last), "mark read", cx),
                );
            } else {
                menu.action(
                    tr(cx, key::lng_context_mark_unread),
                    MENU_ICON_MARK_UNREAD,
                    MenuTone::Normal,
                    move |_, cx| {
                        send(
                            session.set_chat_marked_unread(id, UnreadMark::Marked),
                            "mark unread",
                            cx,
                        )
                    },
                );
            }
        }

        let layers = self.layers.clone();
        let name = chat.title.clone();
        if let Some(user) = user.filter(|user| !user.is_contact && !is_self) {
            let text = if user.kind == UserKind::Bot {
                key::lng_profile_block_bot
            } else {
                key::lng_profile_block_user
            };
            let (layers, user_id, name) = (layers.clone(), user.id, name.clone());
            let session = session.clone();
            menu.action(
                tr(cx, text),
                MENU_ICON_BLOCK,
                MenuTone::Normal,
                move |window, cx| {
                    let confirmed = on_confirm(
                        &session,
                        move |session| Box::pin(session.block_user(user_id)),
                        "block",
                    );
                    boxes::confirm_attention(
                        &layers,
                        tr_with(cx, key::lng_blocked_list_confirm_text, &[("name", &name)]),
                        tr(cx, key::lng_blocked_list_confirm_ok),
                        confirmed,
                        window,
                        cx,
                    );
                },
            );
        }

        peer.add_clear_history(menu, cx);
        peer.add_removal(menu, cx);
    }
}
