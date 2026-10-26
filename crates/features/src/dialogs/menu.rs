//! A chat row's context menu (`Window::FillDialogsEntryMenu`): pin, mute,
//! read state, archive, block, clear and delete.

use std::future::Future;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, DismissEvent, Focusable as _, IntoElement,
    ParentElement, Pixels, Point, SharedString, Window, anchored, deferred, div, px,
};
use i18n::key;
use model::chat::{Chat, ChatKind, ChatList};
use model::chat_actions::{Muting, Pinning, Removal, UnreadMark};
use model::user::UserKind;
use model::{ChatId, MessageId, RequestError, Session};
use style::menu_icons::{
    MENU_ICON_ARCHIVE, MENU_ICON_BLOCK, MENU_ICON_CLEAR, MENU_ICON_COLLAPSE,
    MENU_ICON_DELETE_ATTENTION, MENU_ICON_EXPAND, MENU_ICON_LEAVE_ATTENTION, MENU_ICON_MARK_READ,
    MENU_ICON_MARK_UNREAD, MENU_ICON_MUTE, MENU_ICON_PIN, MENU_ICON_TO_MAIN_MENU,
    MENU_ICON_UNARCHIVE, MENU_ICON_UNMUTE, MENU_ICON_UNPIN,
};
use style::widgets::POPUP_MENU_WITH_ICONS;
use ui::{Markup, MenuTone, PopupMenu};

use super::Dialogs;
use crate::boxes::{self, Callback, ConfirmArgs};
use crate::lang::{tr, tr_with};
use crate::settings;
use crate::store::spawn;

/// Marking more unread messages than this read asks first.
const MAX_UNREAD_WITHOUT_CONFIRMATION: i64 = 1000;
/// Keeps the menu off the window edges.
const EDGE_MARGIN: f32 = 8.0;

/// Sends a request and logs its failure; the result arrives as updates.
fn send(
    task: impl Future<Output = Result<(), RequestError>> + Send + 'static,
    what: &'static str,
    cx: &mut App,
) {
    let task = spawn(task, cx);
    cx.spawn(async move |_| {
        if let Err(error) = task.await {
            tracing::warn!(%error, what, "chat action failed");
        }
    })
    .detach();
}

/// A request to send once the user confirms.
fn on_confirm(
    session: &Session,
    request: impl Fn(
        &Session,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), RequestError>> + Send>>
    + 'static,
    what: &'static str,
) -> Callback {
    let session = session.clone();
    Rc::new(move |_, cx| send(request(&session), what, cx))
}

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
        // shortcut: no "moved to the main menu" toast until toasts exist (M3).
        menu.action(
            tr(cx, key::lng_context_archive_to_menu),
            MENU_ICON_TO_MAIN_MENU,
            MenuTone::Normal,
            |_, cx| settings::set_archive_in_main_menu(true, cx),
        );
        let state = self.store.read(cx).state();
        let unread: Vec<(ChatId, Option<MessageId>, i32)> = state
            .chats
            .list(ChatList::Archive)
            .filter_map(|id| state.chats.chat(id))
            .filter(|chat| chat.unread_count > 0 || chat.is_marked_as_unread)
            .map(|chat| {
                let last = chat.last_message.as_ref().map(|message| message.id);
                (chat.id, last, chat.unread_count)
            })
            .collect();
        if unread.is_empty() {
            return;
        }
        let messages: i64 = unread.iter().map(|(_, _, count)| i64::from(*count)).sum();
        let session = self.store.read(cx).session().clone();
        let layers = self.layers.clone();
        let read_all: Callback = Rc::new(move |_, cx| {
            for (chat, last, _) in &unread {
                send(session.read_chat(*chat, *last), "mark archive read", cx);
            }
        });
        menu.action(
            tr(cx, key::lng_context_mark_read),
            MENU_ICON_MARK_READ,
            MenuTone::Normal,
            move |window, cx| {
                if messages <= MAX_UNREAD_WITHOUT_CONFIRMATION {
                    read_all(window, cx);
                    return;
                }
                let mut args = ConfirmArgs::new(Markup::plain(
                    tr(cx, key::lng_context_mark_read_archive_sure).to_string(),
                ));
                args.confirm_text = Some(tr(cx, key::lng_box_ok));
                args.cancel_text = Some(tr(cx, key::lng_cancel));
                args.confirmed = Some(read_all.clone());
                boxes::confirm(&layers, args, window, cx);
            },
        );
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
            let (text, icon, muting) = if state.chats.is_muted(chat) {
                (key::lng_context_unmute, MENU_ICON_UNMUTE, Muting::Unmuted)
            } else {
                (key::lng_context_mute, MENU_ICON_MUTE, Muting::Muted)
            };
            let session = session.clone();
            // shortcut: mutes for good; the durations submenu comes with M7 notification settings.
            menu.action(tr(cx, text), icon, MenuTone::Normal, move |_, cx| {
                send(session.set_chat_muted(id, muting), "mute", cx);
            });
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

        let is_channel = matches!(chat.kind, ChatKind::Supergroup { is_channel: true });
        if !is_channel {
            let clear_text = if chat.user().is_some() {
                tr_with(cx, key::lng_sure_delete_history, &[("contact", &name)])
            } else {
                tr_with(cx, key::lng_sure_delete_group_history, &[("group", &name)])
            };
            let (layers, session) = (layers.clone(), session.clone());
            menu.action(
                tr(cx, key::lng_profile_clear_history),
                MENU_ICON_CLEAR,
                MenuTone::Normal,
                move |window, cx| {
                    let confirmed = on_confirm(
                        &session,
                        move |session| Box::pin(session.clear_chat_history(id)),
                        "clear history",
                    );
                    boxes::confirm_attention(
                        &layers,
                        clear_text.clone(),
                        tr(cx, key::lng_box_delete),
                        confirmed,
                        window,
                        cx,
                    );
                },
            );
        }

        let (text, icon, removal, question, button) = removal(chat, &name, cx);
        menu.action(text, icon, MenuTone::Attention, move |window, cx| {
            let confirmed = on_confirm(
                &session,
                move |session| Box::pin(session.remove_chat(id, removal)),
                "delete chat",
            );
            boxes::confirm_attention(
                &layers,
                question.clone(),
                button.clone(),
                confirmed,
                window,
                cx,
            );
        });
    }
}

/// The delete or leave item for `chat`: its text, icon, what it does, and
/// the question and button of its confirmation.
fn removal(
    chat: &Chat,
    name: &str,
    cx: &App,
) -> (SharedString, style::Icon, Removal, String, SharedString) {
    match chat.kind {
        ChatKind::Private { .. } | ChatKind::Secret { .. } => (
            tr(cx, key::lng_profile_delete_conversation),
            MENU_ICON_DELETE_ATTENTION,
            Removal::DeleteHistory,
            tr_with(cx, key::lng_sure_delete_history, &[("contact", name)]),
            tr(cx, key::lng_box_delete),
        ),
        ChatKind::Group => (
            tr(cx, key::lng_profile_clear_and_exit),
            MENU_ICON_DELETE_ATTENTION,
            Removal::LeaveAndDelete,
            tr_with(cx, key::lng_sure_delete_and_exit, &[("group", name)]),
            tr(cx, key::lng_box_leave),
        ),
        ChatKind::Supergroup { is_channel: false } => (
            tr(cx, key::lng_profile_leave_group),
            MENU_ICON_LEAVE_ATTENTION,
            Removal::Leave,
            tr(cx, key::lng_sure_leave_group).to_string(),
            tr(cx, key::lng_box_leave),
        ),
        ChatKind::Supergroup { is_channel: true } => (
            tr(cx, key::lng_profile_leave_channel),
            MENU_ICON_LEAVE_ATTENTION,
            Removal::Leave,
            tr(cx, key::lng_sure_leave_channel).to_string(),
            tr(cx, key::lng_box_leave),
        ),
    }
}
