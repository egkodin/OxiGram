//! Forwarding (`HistoryWidget::forwardSelected`): the messages go to a
//! chat picked in "Choose recipient...", whose field shows them until the
//! user sends them, after any text typed as a comment.

use std::rc::Rc;

use gpui::{App, Context, Window};
use i18n::key;
use model::send::SendOptions;
use model::{MessageId, RequestError};

use super::field_bar::{FieldBar, one_line};
use super::headers::origin_name;
use super::service::from;
use super::{HistoryEvent, HistoryView};
use crate::lang::{strings, tr_plural};
use crate::recipients_box::{self, OnChosen};
use crate::store::{Forwarding, spawn};

/// What a send gave up on.
enum Failed {
    /// The text, so nothing was sent.
    Text,
    /// The forward, after the text went.
    Forward,
}

impl HistoryView {
    /// Opens "Choose recipient..." for `messages`, then shows the picked
    /// chat with them over its field.
    pub(super) fn forward(
        &mut self,
        messages: Vec<MessageId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let forwarding = self.forwarding_of(messages, cx);
        let store = self.store.clone();
        let weak = cx.weak_entity();
        let on_chosen: OnChosen = Rc::new(move |chat, _, cx| {
            store.update(cx, |store, cx| {
                store.set_forwarding(chat, forwarding.clone(), cx)
            });
            // The view is gone once its chat closed; the forward still waits.
            let _ = weak.update(cx, |this, cx| {
                this.clear_selection(cx);
                cx.emit(HistoryEvent::OpenChat(chat));
            });
        });
        recipients_box::show(&self.store, &self.layers, on_chosen, window, cx);
    }

    /// The forward with the bar's lines (`ForwardPanel::updateTexts`): who
    /// wrote the messages, and the one message or how many.
    // shortcut: full names where tdesktop shortens them for two or more senders; with name helpers in M6.
    fn forwarding_of(&self, messages: Vec<MessageId>, cx: &App) -> Forwarding {
        let strings = strings(cx);
        let chats = &self.store.read(cx).state().chats;
        let found: Vec<_> = messages
            .iter()
            .filter_map(|id| self.bar_message(*id, cx))
            .collect();
        let mut names: Vec<String> = Vec::new();
        for message in &found {
            let name = message.forward.as_ref().map_or_else(
                || from(strings, chats, message),
                |forward| origin_name(strings, chats, &forward.origin),
            );
            if !names.contains(&name) {
                names.push(name);
            }
        }
        let title = match names.as_slice() {
            [] => String::new(),
            [only] => only.clone(),
            [first, second] => i18n::fill(
                strings.text(key::lng_forwarding_from_two),
                &[("user", first), ("second_user", second)],
            ),
            [first, rest @ ..] => tr_plural(
                cx,
                key::lng_forwarding_from,
                i64::try_from(rest.len()).unwrap_or(i64::MAX),
                &[("user", first)],
            ),
        };
        let text = match found.as_slice() {
            [only] => one_line(strings, only),
            _ => tr_plural(
                cx,
                key::lng_forward_messages,
                i64::try_from(messages.len()).unwrap_or(i64::MAX),
                &[],
            ),
        };
        Forwarding {
            from: self.chat,
            messages,
            title,
            text,
        }
    }

    /// Takes the messages waiting to be forwarded here, if any.
    pub(super) fn take_forwarding(&mut self, cx: &mut Context<Self>) {
        let chat = self.chat;
        if self.topic.is_some() || !self.store.read(cx).has_forwarding(chat) {
            return;
        }
        let Some(forwarding) = self
            .store
            .update(cx, |store, _| store.take_forwarding(chat))
        else {
            return;
        };
        if matches!(self.compose.bar, Some(FieldBar::Edit { .. })) {
            self.cancel_field_bar(cx);
        }
        self.compose.bar = Some(FieldBar::Forward(forwarding));
        cx.notify();
    }

    /// Sends the typed text, as a reply if one is open, then the forward;
    /// gives back whatever TDLib refused.
    pub(super) fn send_with_bar(
        &mut self,
        text: String,
        options: SendOptions,
        cx: &mut Context<Self>,
    ) {
        let has_text = !text.trim().is_empty();
        let bar = self.compose.bar.take();
        let (reply, forwarding) = match &bar {
            Some(FieldBar::Reply(reply)) => (Some(*reply), None),
            Some(FieldBar::Forward(forwarding)) => (None, Some(forwarding.clone())),
            Some(FieldBar::Edit { .. }) | None => (None, None),
        };
        let field = self.compose.field.clone();
        let tags = field.read(cx).tags().to_vec();
        let emoji: Vec<_> = tags
            .iter()
            .map(|(range, id)| model::send::CustomEmojiSpan {
                range: range.clone(),
                id: model::CustomEmojiId(*id),
            })
            .collect();
        if has_text {
            self.compose.draft.sent();
            field.update(cx, |field, cx| field.set_text("", cx));
        }
        let session = self.store.read(cx).session();
        let text_part = has_text
            .then(|| session.send_text(self.chat, self.topic, reply, (&text, &emoji), options));
        let forward_part = forwarding.map(|forwarding| {
            let messages = &forwarding.messages;
            session.forward_messages(self.chat, self.topic, forwarding.from, messages, options)
        });
        let task = spawn(
            async move {
                if let Some(part) = text_part {
                    part.await.map_err(|error| (Failed::Text, error))?;
                }
                if let Some(part) = forward_part {
                    part.await.map_err(|error| (Failed::Forward, error))?;
                }
                Ok::<(), (Failed, RequestError)>(())
            },
            cx,
        );
        self.scroll_down(cx);
        cx.spawn(async move |this, cx| {
            let Err((failed, error)) = task.await else {
                return;
            };
            tracing::warn!(%error, "sending a message failed");
            // The view is gone once its chat closed; TDLib keeps nothing then.
            let _ = this.update(cx, |this, cx| {
                if this.compose.bar.is_some() {
                    return;
                }
                match failed {
                    Failed::Text if field.read(cx).text().is_empty() => {
                        this.compose.bar = bar;
                        field.update(cx, |field, cx| field.set_tagged_text(&text, tags, cx));
                    }
                    Failed::Text => {}
                    Failed::Forward => {
                        this.compose.bar = bar.filter(|bar| matches!(bar, FieldBar::Forward(_)));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
