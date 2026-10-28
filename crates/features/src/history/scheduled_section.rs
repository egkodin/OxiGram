//! The scheduled messages section (`ScheduledWidget`): a chat's scheduled
//! messages, reminders in Saved Messages, each day under "Scheduled for",
//! opened from the button by the field.

use std::rc::Rc;
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, TimeZone};
use gpui::{AnyElement, Context, IntoElement as _, Window};
use i18n::{Strings, key};
use model::MessageId;
use model::message::{Message, Schedule};
use style::chat_helpers::HISTORY_SCHEDULED_TOGGLE;
use ui::IconButton;

use super::pinned_section::Section;
use super::send_menu::OnOptions;
use super::{HistoryView, bubble, layout, schedule_box, top_bar};
use crate::boxes::{self, ConfirmArgs};
use crate::lang::{strings, tr};
use crate::peer_menu::send;
use crate::store::spawn;

/// "Scheduled for October 28", or "Scheduled until online"
/// (`ScheduledWidget`'s date service messages).
pub(super) fn badge<Tz: TimeZone>(
    strings: &Strings,
    message: &Message,
    day: NaiveDate,
    now: &DateTime<Tz>,
) -> String {
    if message.schedule == Some(Schedule::WhenOnline) {
        return strings
            .text(key::lng_scheduled_date_until_online)
            .to_owned();
    }
    i18n::fill(
        strings.text(key::lng_scheduled_date),
        &[("date", &layout::date_badge(strings, day, now))],
    )
}

impl HistoryView {
    pub(super) fn date_badge(
        &self,
        message: &Message,
        day: NaiveDate,
        now: &DateTime<chrono::Local>,
        cx: &Context<Self>,
    ) -> bubble::DateBadge {
        let weak = cx.weak_entity();
        let is_scheduled = message.schedule.is_some();
        let text = if is_scheduled {
            badge(strings(cx), message, day, now)
        } else {
            layout::date_badge(strings(cx), day, now)
        };
        bubble::DateBadge {
            text: text.into(),
            id: ("date-badge", message.id.0.unsigned_abs()).into(),
            // Scheduled days have no history to jump to.
            on_click: Rc::new(move |_, window, cx| {
                if !is_scheduled {
                    let _ = weak.update(cx, |this, cx| this.show_calendar(day, window, cx));
                }
            }),
        }
    }

    /// The button by the field that opens the section, while the chat has
    /// scheduled messages (`HistoryWidget::_scheduled`).
    pub(super) fn scheduled_button(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.store.read(cx).state();
        let chat = state.chats.chat(self.chat)?;
        if !chat.has_scheduled_messages || self.topic.is_some() {
            return None;
        }
        let weak = cx.weak_entity();
        Some(
            IconButton::new("compose-scheduled", &HISTORY_SCHEDULED_TOGGLE)
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.open_scheduled_section(cx));
                })
                .into_any_element(),
        )
    }

    fn open_scheduled_section(&mut self, cx: &mut Context<Self>) {
        self.set_section(Section::Scheduled { is_loaded: false }, cx);
        self.load_scheduled(cx);
    }

    /// Fetches the list again, as tdesktop does when the section opens.
    fn load_scheduled(&mut self, cx: &mut Context<Self>) {
        let task = spawn(
            self.store.read(cx).session().scheduled_messages(self.chat),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let messages = match task.await {
                Ok(messages) => messages,
                Err(error) => {
                    tracing::warn!(%error, "loading scheduled messages failed");
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                let key = this.key();
                this.store
                    .update(cx, |store, cx| store.set_scheduled(key, messages, cx));
                if let Section::Scheduled { .. } = this.section {
                    this.section = Section::Scheduled { is_loaded: true };
                    this.sync(cx);
                }
            });
        })
        .detach();
    }

    /// The section's bar: "Scheduled Messages", or "Reminders" in Saved
    /// Messages, after the back arrow.
    pub(super) fn scheduled_bar(&self, cx: &Context<Self>) -> AnyElement {
        let state = self.store.read(cx).state();
        let is_self = state
            .chats
            .chat(self.chat)
            .is_some_and(|chat| chat.user().is_some() && chat.user() == state.me);
        let title = if is_self {
            key::lng_reminder_messages
        } else {
            key::lng_scheduled_messages
        };
        let weak = cx.weak_entity();
        top_bar::section_bar(
            tr(cx, title),
            Box::new(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.close_section(cx));
            }),
            cx,
        )
    }

    /// The scheduled messages to show, `None` until the list closes the
    /// section by running out.
    pub(super) fn scheduled_list(&self, cx: &Context<Self>) -> Option<Vec<Arc<Message>>> {
        let history = self.store.read(cx).state().history(self.key())?;
        let messages = history.scheduled().to_vec();
        let is_done =
            messages.is_empty() && self.section == (Section::Scheduled { is_loaded: true });
        (!is_done).then_some(messages)
    }

    /// Asks, then sends the message now (`SendScheduledNow`).
    pub(super) fn send_scheduled_now(
        &mut self,
        message: MessageId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut args = ConfirmArgs::new(ui::Markup::plain(
            tr(cx, key::lng_scheduled_send_now).to_string(),
        ));
        args.confirm_text = Some(tr(cx, key::lng_send_button));
        args.cancel_text = Some(tr(cx, key::lng_cancel));
        let weak = cx.weak_entity();
        args.confirmed = Some(Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| {
                let session = this.store.read(cx).session().clone();
                send(session.reschedule(this.chat, message, None), "send now", cx);
            });
        }));
        boxes::confirm(&self.layers, args, window, cx);
    }

    /// The schedule box for a new time (`EditScheduledMessage`); the list
    /// loads again, as TDLib moves the message.
    pub(super) fn reschedule(
        &mut self,
        message: MessageId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(kind) = self.send_menu(cx) else {
            return;
        };
        let weak = cx.weak_entity();
        let on_send: OnOptions = Rc::new(move |options, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                let session = this.store.read(cx).session().clone();
                let task = spawn(session.reschedule(this.chat, message, options.schedule), cx);
                cx.spawn(async move |this, cx| {
                    if let Err(error) = task.await {
                        tracing::warn!(%error, "rescheduling failed");
                    }
                    let _ = this.update(cx, |this, cx| this.load_scheduled(cx));
                })
                .detach();
            });
        });
        schedule_box::show(&self.layers, kind, on_send, window, cx);
    }
}
