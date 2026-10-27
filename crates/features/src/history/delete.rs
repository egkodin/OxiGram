//! The box that asks before deleting messages (`DeleteMessagesBox`), with
//! "delete for everyone" when the user may revoke them.

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, Render, Styled, WeakEntity, Window, actions, div, prelude::*, px,
};
use i18n::key;
use model::message_actions::{Abilities, Deletion};
use model::{ChatId, MessageId, Session};
use style::layers::{
    BOX_LABEL, BOX_MEDIUM_SKIP, BOX_PADDING, BOX_WIDTH, DEFAULT_BOX, DEFAULT_BOX_CHECKBOX,
};
use style::widgets::DEFAULT_CHECK;
use ui::{BoxFrame, Checkbox, Checked, FlatLabel, Markup, RoundButton};

use crate::boxes::Callback;
use crate::lang::{tr, tr_plural, tr_with};
use crate::layers::{Dismiss, Layers};
use crate::peer_menu::send;
use crate::px_of;

actions!(delete_messages_box, [Confirm]);

const CONTEXT: &str = "DeleteMessagesBox";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("enter", Confirm, Some(CONTEXT))]);
}

/// The "also delete for others" checkbox (`revokeText`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Revoke {
    /// "Also delete for {user}" in a private chat.
    User(String),
    Everyone,
    /// Only the user's own messages can be revoked.
    Own,
}

/// The checkbox for messages with these abilities and outgoing flags, in a
/// private chat with `user` (their first name); none when nothing can be
/// revoked or TDLib did not tell.
// shortcut: no "unsend" description under the Own checkbox and no "remember" checkbox; both come with M8 polish.
pub(super) fn revoke(
    abilities: &[Abilities],
    outgoing: &[bool],
    user: Option<String>,
) -> Option<Revoke> {
    if abilities.is_empty() {
        return None;
    }
    if abilities.iter().all(|a| a.can_be_deleted_for_all_users) {
        return Some(user.map_or(Revoke::Everyone, Revoke::User));
    }
    let can_revoke_own = abilities
        .iter()
        .zip(outgoing)
        .any(|(a, is_out)| *is_out && a.can_be_deleted_for_all_users);
    can_revoke_own.then_some(Revoke::Own)
}

/// What to delete.
pub(super) struct Request {
    pub session: Session,
    pub chat: ChatId,
    pub ids: Vec<MessageId>,
    pub revoke: Option<Revoke>,
}

struct DeleteBox {
    request: Request,
    revoke: Checked,
    /// Runs after the request is sent.
    deleted: Callback,
    layers: WeakEntity<Layers>,
    focus: FocusHandle,
}

impl Focusable for DeleteBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Shows the box; Escape and a click outside only close it.
pub(super) fn show(
    layers: &Entity<Layers>,
    request: Request,
    deleted: Callback,
    window: &mut Window,
    cx: &mut App,
) {
    let weak = layers.downgrade();
    let view = cx.new(|cx| DeleteBox {
        request,
        revoke: Checked::Yes,
        deleted,
        layers: weak,
        focus: cx.focus_handle(),
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx);
    });
}

impl DeleteBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let deletion = if self.request.revoke.is_some() && self.revoke == Checked::Yes {
            Deletion::ForEveryone
        } else {
            Deletion::ForMe
        };
        let request = &self.request;
        send(
            request
                .session
                .delete_messages(request.chat, &request.ids, deletion),
            "delete messages",
            cx,
        );
        self.close(window, cx);
        (self.deleted)(window, cx);
    }

    fn question(&self, cx: &App) -> String {
        match self.request.ids.len() {
            1 => tr(cx, key::lng_selected_delete_sure_this).to_string(),
            count => tr_plural(
                cx,
                key::lng_selected_delete_sure,
                i64::try_from(count).unwrap_or(i64::MAX),
                &[],
            ),
        }
    }

    fn checkbox(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let text = match self.request.revoke.as_ref()? {
            Revoke::User(name) => tr_with(cx, key::lng_delete_for_other_check, &[("user", name)]),
            Revoke::Everyone => tr(cx, key::lng_delete_for_everyone_check).to_string(),
            Revoke::Own => tr(cx, key::lng_delete_for_other_my).to_string(),
        };
        let entity = cx.entity();
        Some(
            div().pt(px_of(BOX_MEDIUM_SKIP)).child(
                Checkbox::new(
                    "delete-revoke",
                    &DEFAULT_BOX_CHECKBOX,
                    &DEFAULT_CHECK,
                    text,
                    self.revoke,
                )
                .on_toggle(move |_, cx| {
                    entity.update(cx, |this, cx| {
                        this.revoke = if this.revoke == Checked::Yes {
                            Checked::No
                        } else {
                            Checked::Yes
                        };
                        cx.notify();
                    });
                }),
            ),
        )
    }
}

impl Render for DeleteBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let padding = BOX_PADDING;
        let width = px_of(BOX_WIDTH) - px_of(padding.left) - px_of(padding.right);
        let content = div()
            .flex()
            .flex_col()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .child(
                FlatLabel::new("delete-text", &BOX_LABEL, Markup::plain(self.question(cx)))
                    .width(width),
            )
            .children(self.checkbox(cx));
        let entity = cx.entity();
        let delete = RoundButton::new(
            "delete",
            &style::layers::ATTENTION_BOX_BUTTON,
            tr(cx, key::lng_box_delete),
        )
        .on_click({
            let entity = entity.clone();
            move |_, window, cx| entity.update(cx, |this, cx| this.delete(window, cx))
        });
        let cancel = RoundButton::new("cancel", &DEFAULT_BOX.button, tr(cx, key::lng_cancel))
            .on_click(move |_, window, cx| entity.update(cx, |this, cx| this.close(window, cx)));
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Confirm, window, cx| this.delete(window, cx)))
            .child(
                BoxFrame::new("delete-box", &DEFAULT_BOX)
                    .row(content.min_w(px(0.0)))
                    .button(delete)
                    .button(cancel),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abilities(for_all: bool) -> Abilities {
        Abilities {
            can_be_copied: true,
            can_be_deleted_only_for_self: true,
            can_be_deleted_for_all_users: for_all,
            can_get_link: false,
            can_be_pinned: false,
            can_get_read_date: false,
        }
    }

    #[test]
    fn revoking_all_names_the_private_chat_user() {
        let all = [abilities(true), abilities(true)];
        assert_eq!(
            revoke(&all, &[true, false], Some("Maria".to_owned())),
            Some(Revoke::User("Maria".to_owned()))
        );
        assert_eq!(revoke(&all, &[true, false], None), Some(Revoke::Everyone));
    }

    #[test]
    fn only_own_messages_revoke_when_others_cannot() {
        let mixed = [abilities(true), abilities(false)];
        assert_eq!(revoke(&mixed, &[true, false], None), Some(Revoke::Own));
        assert_eq!(revoke(&mixed, &[false, true], None), None);
        assert_eq!(revoke(&[], &[], None), None);
    }
}
