//! The bars that take the field's place where the user cannot write
//! (`HistoryWidget::updateControlsVisibility`): Unblock, Restart bot, Join,
//! Mute or Unmute, Start, or why the admins closed the field.

use chrono::Local;
use gpui::{
    AnyElement, Context, IntoElement as _, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div,
};
use i18n::key;
use model::chat_actions::Muting;
use model::rights::{Restriction, Writing};
use style::chat::HISTORY_SEND_DISABLED;
use style::chat_helpers::{HISTORY_COMPOSE_BUTTON, HISTORY_SEND_SIZE, HISTORY_UNBLOCK};
use style::{ColorId, ColorRef};
use ui::{color, font, font_height};

use super::HistoryView;
use super::contact_status::flat_button;
use super::layout;
use crate::lang::{strings, tr, tr_with};
use crate::px_of;

/// What a bar's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Unblock,
    RestartBot,
    Join,
    Mute(Muting),
    StartBot,
}

impl HistoryView {
    /// The bar for `writing`, or `None` where the field shows.
    pub(super) fn write_bar(&self, writing: Writing, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (text, press, st) = match writing {
            Writing::Field => return None,
            Writing::Restricted(restriction) => return Some(self.restriction(restriction, cx)),
            Writing::Unblock => (key::lng_unblock_button, Press::Unblock, &HISTORY_UNBLOCK),
            Writing::RestartBot => (key::lng_restart_button, Press::RestartBot, &HISTORY_UNBLOCK),
            Writing::Join { is_channel } => (
                if is_channel {
                    key::lng_profile_join_channel
                } else {
                    key::lng_profile_join_group
                },
                Press::Join,
                &HISTORY_COMPOSE_BUTTON,
            ),
            Writing::Mute { is_muted: true } => (
                key::lng_channel_unmute,
                Press::Mute(Muting::Unmuted),
                &HISTORY_COMPOSE_BUTTON,
            ),
            Writing::Mute { is_muted: false } => (
                key::lng_channel_mute,
                Press::Mute(Muting::Muted),
                &HISTORY_COMPOSE_BUTTON,
            ),
            Writing::StartBot => (key::lng_bot_start, Press::StartBot, &HISTORY_COMPOSE_BUTTON),
        };
        let weak = cx.weak_entity();
        let button = flat_button(("write-bar", 0), tr(cx, text).to_uppercase(), st, cx).on_click(
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.press_write_bar(press, cx));
            },
        );
        Some(div().flex_none().flex().child(button).into_any_element())
    }

    fn press_write_bar(&self, press: Press, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        let user = self
            .store
            .read(cx)
            .state()
            .chats
            .chat(self.chat)
            .and_then(model::chat::Chat::user);
        let chat = self.chat;
        match (press, user) {
            (Press::Unblock, Some(user)) => {
                self.after(session.unblock_user(user), "unblock", None, None, cx);
            }
            (Press::RestartBot, Some(bot)) => {
                let start: super::contact_status::Step = Box::pin(session.start_bot(bot, chat));
                self.after(
                    session.unblock_user(bot),
                    "restart bot",
                    None,
                    Some(start),
                    cx,
                );
            }
            (Press::StartBot, Some(bot)) => {
                self.after(session.start_bot(bot, chat), "start bot", None, None, cx);
            }
            (Press::Join, _) => self.after(session.join_chat(chat), "join", None, None, cx),
            (Press::Mute(muting), _) => {
                self.after(session.set_chat_muted(chat, muting), "mute", None, None, cx);
            }
            (Press::Unblock | Press::RestartBot | Press::StartBot, None) => {}
        }
    }

    /// The admins' restriction where the field would be
    /// (`TextErrorSendRestriction`).
    fn restriction(&self, restriction: Restriction, cx: &mut Context<Self>) -> AnyElement {
        let text = match restriction {
            Restriction::Until(until) => {
                let now = Local::now();
                let date = layout::day(until, &Local)
                    .map(|date| layout::date_badge(strings(cx), date, &now))
                    .unwrap_or_default();
                let time = layout::time(until, &Local);
                tr_with(
                    cx,
                    key::lng_restricted_send_message_until,
                    &[("date", &date), ("time", &time)],
                )
            }
            Restriction::ByAdmins => tr(cx, key::lng_restricted_send_message).to_string(),
            Restriction::ForAll => tr(cx, key::lng_restricted_send_message_all).to_string(),
        };
        let st = &HISTORY_SEND_DISABLED;
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .px(px_of(HISTORY_COMPOSE_BUTTON.text_top))
            .h(px_of(HISTORY_SEND_SIZE.height))
            .bg(color(cx, ColorRef::Palette(ColorId::HistoryReplyBg)))
            .font(font(st.style.font))
            .text_size(px_of(st.style.font.size))
            .line_height(font_height(st.style.font))
            .text_color(color(cx, st.text_fg))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(text)
            .into_any_element()
    }
}
