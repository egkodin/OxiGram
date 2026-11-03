//! Chat menu items shared by the chat list and the chat's top bar
//! (`Window::Filler`): mute, clear history, delete or leave.

use std::future::Future;
use std::rc::Rc;

use gpui::{App, Entity, SharedString};
use i18n::key;
use model::chat::{Chat, ChatKind};
use model::chat_actions::{Muting, Removal};
use model::{RequestError, Session};
use style::menu_icons::{
    MENU_ICON_CLEAR, MENU_ICON_DELETE_ATTENTION, MENU_ICON_LEAVE_ATTENTION, MENU_ICON_MUTE,
    MENU_ICON_UNMUTE,
};
use ui::{MenuTone, PopupMenu};

use crate::boxes::{self, Callback};
use crate::lang::{tr, tr_with};
use crate::layers::Layers;
use crate::store::spawn;

/// Sends a request and logs its failure; the result arrives as updates.
pub(crate) fn send(
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
pub(crate) fn on_confirm(
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

/// The chat a menu is for, and what its items need.
pub(crate) struct PeerMenu<'a> {
    pub chat: &'a Chat,
    pub session: Session,
    pub layers: Entity<Layers>,
}

impl PeerMenu<'_> {
    pub(crate) fn add_mute(&self, menu: &mut PopupMenu, is_muted: bool, cx: &App) {
        let (text, icon, muting) = if is_muted {
            (key::lng_context_unmute, MENU_ICON_UNMUTE, Muting::Unmuted)
        } else {
            (key::lng_context_mute, MENU_ICON_MUTE, Muting::Muted)
        };
        let (session, id) = (self.session.clone(), self.chat.id);
        // shortcut: mutes for good; the durations submenu comes with M7 notification settings.
        menu.action(tr(cx, text), icon, MenuTone::Normal, move |_, cx| {
            send(session.set_chat_muted(id, muting), "mute", cx);
        });
    }

    /// Clear history, except in channels (`addClearHistory`).
    pub(crate) fn add_clear_history(&self, menu: &mut PopupMenu, cx: &App) {
        let chat = self.chat;
        if matches!(
            chat.kind,
            ChatKind::Supergroup {
                is_channel: true,
                ..
            }
        ) {
            return;
        }
        let clear_text = if chat.user().is_some() {
            tr_with(
                cx,
                key::lng_sure_delete_history,
                &[("contact", &chat.title)],
            )
        } else {
            tr_with(
                cx,
                key::lng_sure_delete_group_history,
                &[("group", &chat.title)],
            )
        };
        let (layers, session, id) = (self.layers.clone(), self.session.clone(), chat.id);
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

    /// Delete the conversation, or leave the group or channel.
    pub(crate) fn add_removal(&self, menu: &mut PopupMenu, cx: &App) {
        let (text, icon, removal, question, button) = removal(self.chat, cx);
        let (layers, session, id) = (self.layers.clone(), self.session.clone(), self.chat.id);
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
pub(crate) fn removal(
    chat: &Chat,
    cx: &App,
) -> (SharedString, style::Icon, Removal, String, SharedString) {
    let name = chat.title.as_str();
    match chat.kind {
        ChatKind::Private { .. } | ChatKind::Secret { .. } => (
            tr(cx, key::lng_profile_delete_conversation),
            MENU_ICON_DELETE_ATTENTION,
            Removal::DeleteHistory,
            tr_with(cx, key::lng_sure_delete_history, &[("contact", name)]),
            tr(cx, key::lng_box_delete),
        ),
        ChatKind::Group { .. } => (
            tr(cx, key::lng_profile_clear_and_exit),
            MENU_ICON_DELETE_ATTENTION,
            Removal::LeaveAndDelete,
            tr_with(cx, key::lng_sure_delete_and_exit, &[("group", name)]),
            tr(cx, key::lng_box_leave),
        ),
        ChatKind::Supergroup {
            is_channel: false, ..
        } => (
            tr(cx, key::lng_profile_leave_group),
            MENU_ICON_LEAVE_ATTENTION,
            Removal::Leave,
            tr(cx, key::lng_sure_leave_group).to_string(),
            tr(cx, key::lng_box_leave),
        ),
        ChatKind::Supergroup {
            is_channel: true, ..
        } => (
            tr(cx, key::lng_profile_leave_channel),
            MENU_ICON_LEAVE_ATTENTION,
            Removal::Leave,
            tr(cx, key::lng_sure_leave_channel).to_string(),
            tr(cx, key::lng_box_leave),
        ),
    }
}
