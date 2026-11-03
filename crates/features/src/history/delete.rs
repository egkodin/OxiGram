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

/// Where the messages are, as `HistoryItem::canDeleteForEveryone` and the
/// box's hints tell chats apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Place {
    /// A private chat with the other user's first name.
    Private(String),
    /// Bots receive every message, so there is nothing to revoke; deleting
    /// still revokes (`_revokeForBot`).
    Bot,
    SavedMessages,
    Group,
    /// Supergroups and channels delete for everyone by themselves.
    Supergroup,
    Channel,
}

impl Place {
    fn allows_revoke(&self) -> bool {
        matches!(self, Self::Private(_) | Self::Group)
    }
}

/// The "also delete for others" checkbox (`revokeText`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Revoke {
    /// "Also delete for {user}" in a private chat.
    User(String),
    Everyone,
    /// Only `count` of the user's own messages can be revoked.
    Own(usize),
}

/// The line under the question (`appendDetails`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Hint {
    /// "You can also delete the messages you sent from {user}'s inbox".
    UnsendUser {
        count: usize,
        user: String,
    },
    UnsendGroup {
        count: usize,
    },
    ForEveryone,
    ForMeInGroup,
    ForMe,
}

/// What the box offers for these messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Offer {
    pub revoke: Option<Revoke>,
    pub hint: Option<Hint>,
    /// Whether deleting revokes with no checkbox (bots).
    pub revokes_anyway: bool,
}

/// tdesktop's offer for messages with these abilities and outgoing flags in
/// `place`; nothing to offer when TDLib told nothing.
pub(super) fn offer(place: &Place, abilities: &[Abilities], outgoing: &[bool]) -> Offer {
    let none = Offer {
        revoke: None,
        hint: None,
        revokes_anyway: false,
    };
    if abilities.is_empty() {
        return none;
    }
    let can_revoke =
        |ability: &Abilities| place.allows_revoke() && ability.can_be_deleted_for_all_users;
    if abilities.iter().all(can_revoke) {
        let revoke = match place {
            Place::Private(user) => Revoke::User(user.clone()),
            _ => Revoke::Everyone,
        };
        return Offer {
            revoke: Some(revoke),
            ..none
        };
    }
    let own = abilities
        .iter()
        .zip(outgoing)
        .filter(|(ability, is_out)| **is_out && can_revoke(ability))
        .count();
    if own > 0 {
        let hint = match place {
            Place::Private(user) => Hint::UnsendUser {
                count: own,
                user: user.clone(),
            },
            _ => Hint::UnsendGroup { count: own },
        };
        return Offer {
            revoke: Some(Revoke::Own(own)),
            hint: Some(hint),
            revokes_anyway: false,
        };
    }
    let hint = match place {
        Place::Supergroup => Some(Hint::ForEveryone),
        Place::Group => Some(Hint::ForMeInGroup),
        Place::Private(_) | Place::Bot => Some(Hint::ForMe),
        Place::SavedMessages | Place::Channel => None,
    };
    Offer {
        revoke: None,
        hint,
        revokes_anyway: *place == Place::Bot,
    }
}

/// What to delete.
pub(super) struct Request {
    pub session: Session,
    pub chat: ChatId,
    pub ids: Vec<MessageId>,
    pub offer: Offer,
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
    // The owner's call: no "Remember this choice" (tdesktop's `_revokeRemember`); revoke starts checked.
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
        let offer = &self.request.offer;
        let revokes = if offer.revoke.is_some() {
            self.revoke == Checked::Yes
        } else {
            offer.revokes_anyway
        };
        let deletion = if revokes {
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

    /// The question, then the hint after a blank line (`appendDetails`).
    fn text(&self, cx: &App) -> Markup {
        let count = self.request.ids.len();
        let question = match count {
            1 => tr(cx, key::lng_selected_delete_sure_this).to_string(),
            count => tr_plural(cx, key::lng_selected_delete_sure, plural(count), &[]),
        };
        let Some(hint) = &self.request.offer.hint else {
            return Markup::plain(question);
        };
        let hint = match hint {
            Hint::UnsendUser { count: 1, user } => tr_with(
                cx,
                key::lng_selected_unsend_about_user_one,
                &[("user", &format!("**{user}**"))],
            ),
            Hint::UnsendUser { count, user } => tr_plural(
                cx,
                key::lng_selected_unsend_about_user,
                plural(*count),
                &[("user", &format!("**{user}**"))],
            ),
            Hint::UnsendGroup { count: 1 } => {
                tr(cx, key::lng_selected_unsend_about_group_one).to_string()
            }
            Hint::UnsendGroup { count } => tr_plural(
                cx,
                key::lng_selected_unsend_about_group,
                plural(*count),
                &[],
            ),
            Hint::ForEveryone => {
                tr_plural(cx, key::lng_delete_for_everyone_hint, plural(count), &[])
            }
            Hint::ForMeInGroup => {
                tr_plural(cx, key::lng_delete_for_me_chat_hint, plural(count), &[])
            }
            Hint::ForMe => tr_plural(cx, key::lng_delete_for_me_hint, plural(count), &[]),
        };
        // shortcut: a `**` typed in a user's name turns bold; escape names when Markup gains it.
        Markup::rich(&format!("{question}\n\n{hint}"))
    }

    fn checkboxes(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let text = match self.request.offer.revoke.as_ref()? {
            Revoke::User(name) => tr_with(cx, key::lng_delete_for_other_check, &[("user", name)]),
            Revoke::Everyone => tr(cx, key::lng_delete_for_everyone_check).to_string(),
            Revoke::Own(_) => tr(cx, key::lng_delete_for_other_my).to_string(),
        };
        let entity = cx.entity();
        let revoke = Checkbox::new(
            "delete-revoke",
            &DEFAULT_BOX_CHECKBOX,
            &DEFAULT_CHECK,
            text,
            self.revoke,
        )
        .on_toggle(move |_, cx| entity.update(cx, |this, cx| toggle(&mut this.revoke, cx)));
        Some(div().pt(px_of(BOX_MEDIUM_SKIP)).child(revoke))
    }
}

fn toggle(checked: &mut Checked, cx: &mut Context<DeleteBox>) {
    *checked = if *checked == Checked::Yes {
        Checked::No
    } else {
        Checked::Yes
    };
    cx.notify();
}

fn plural(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
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
            .child(FlatLabel::new("delete-text", &BOX_LABEL, self.text(cx)).width(width))
            .children(self.checkboxes(cx));
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
            can_be_replied: false,
            can_be_edited: false,
            can_be_forwarded: false,
            can_get_read_date: false,
            can_get_viewers: false,
        }
    }

    fn maria() -> Place {
        Place::Private("Maria".to_owned())
    }

    #[test]
    fn revoking_all_names_the_private_chat_user() {
        let all = [abilities(true), abilities(true)];
        let offer = offer(&maria(), &all, &[true, false]);
        assert_eq!(offer.revoke, Some(Revoke::User("Maria".to_owned())));
        assert_eq!(offer.hint, None);
        assert_eq!(
            super::offer(&Place::Group, &all, &[true, false]).revoke,
            Some(Revoke::Everyone)
        );
    }

    #[test]
    fn only_own_messages_revoke_when_others_cannot() {
        let mixed = [abilities(true), abilities(false)];
        let own = offer(&maria(), &mixed, &[true, false]);
        assert_eq!(own.revoke, Some(Revoke::Own(1)));
        assert_eq!(
            own.hint,
            Some(Hint::UnsendUser {
                count: 1,
                user: "Maria".to_owned()
            })
        );
        let theirs = offer(&maria(), &mixed, &[false, true]);
        assert_eq!((theirs.revoke, theirs.hint), (None, Some(Hint::ForMe)));
        assert_eq!(offer(&maria(), &[], &[]).revoke, None);
    }

    // The live bug's neighbours: TDLib allows revoking where tdesktop shows
    // no checkbox, because deleting there is always for everyone.
    #[test]
    fn supergroups_channels_saved_messages_and_bots_offer_no_checkbox() {
        let all = [abilities(true)];
        for (place, hint) in [
            (Place::Supergroup, Some(Hint::ForEveryone)),
            (Place::Channel, None),
            (Place::SavedMessages, None),
            (Place::Bot, Some(Hint::ForMe)),
        ] {
            let offer = offer(&place, &all, &[true]);
            assert_eq!((offer.revoke, offer.hint), (None, hint), "{place:?}");
        }
        assert!(offer(&Place::Bot, &all, &[true]).revokes_anyway);
        assert_eq!(
            offer(&Place::Group, &[abilities(false)], &[false]).hint,
            Some(Hint::ForMeInGroup)
        );
    }
}
