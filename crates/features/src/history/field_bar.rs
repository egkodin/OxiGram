//! The bar over the field while replying or editing
//! (`HistoryWidget::drawField`): the icon, "Reply to Name" or "Edit
//! message", a line of the message, and a cancel button. Up in an empty
//! field edits the last outgoing message; Ctrl+Up and Ctrl+Down (Cmd on
//! macOS) move the reply through the history (`keyPressEvent`).

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, KeyBinding,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, actions, div,
};
use i18n::key;
use model::MessageId;
use model::message::{Content, Message};
use style::basic::NORMAL_FONT;
use style::chat::{MSG_REPLY_PADDING, MSG_SERVICE_NAME_FONT};
use style::chat_helpers::{
    HISTORY_EDIT_ICON, HISTORY_FORWARD_ICON, HISTORY_REPLY_CANCEL, HISTORY_REPLY_HEIGHT,
    HISTORY_REPLY_ICON, HISTORY_REPLY_ICON_POSITION, HISTORY_REPLY_NAME_FG, HISTORY_REPLY_SKIP,
};
use style::{ColorId, ColorRef, Icon};
use ui::{IconButton, color};

use super::HistoryView;
use super::headers::{first_line, line};
use super::layout::is_service;
use super::service::from;
use crate::dialogs::content_preview;
use crate::lang::strings;
use crate::px_of;
use crate::store::{Forwarding, spawn};

actions!(history_compose, [EditLast, ReplyPrevious, ReplyNext]);

/// The compose bar's key context, around the field.
pub(super) const CONTEXT: &str = "HistoryCompose";

/// How long after sending a message may be edited (`edit_time_limit`).
// shortcut: the default limit; read the server's option once options reach the model.
const EDIT_TIME_LIMIT: i32 = 48 * 60 * 60;

pub(super) fn bind_keys(cx: &mut App) {
    let (up, down) = if cfg!(target_os = "macos") {
        ("cmd-up", "cmd-down")
    } else {
        ("ctrl-up", "ctrl-down")
    };
    cx.bind_keys([
        KeyBinding::new("up", EditLast, Some(CONTEXT)),
        KeyBinding::new(up, ReplyPrevious, Some(CONTEXT)),
        KeyBinding::new(down, ReplyNext, Some(CONTEXT)),
    ]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FieldBar {
    Reply(MessageId),
    /// `stash` is what the field held before and gets back afterwards,
    /// with the reply that was open.
    Edit {
        message: MessageId,
        stash: String,
        reply: Option<MessageId>,
    },
    Forward(Forwarding),
}

/// The text the field edits, for a plain text message; formatted ones wait
/// for a field that keeps formatting.
// shortcut: no editing of formatted text or captions; with entities in the field (M5).
pub(super) fn editable_text(message: &Message) -> Option<&str> {
    match &message.content {
        Content::Text { text, .. } if text.entities.is_empty() => Some(&text.text),
        _ => None,
    }
}

/// A message's first line as previews show it, spoilers covered.
pub(super) fn one_line(strings: &i18n::Strings, message: &Message) -> String {
    let preview = content_preview(strings, &message.content)
        .0
        .hiding_spoilers();
    first_line(&preview).to_owned()
}

impl HistoryView {
    pub(super) fn bar_message<'a>(&'a self, id: MessageId, cx: &'a App) -> Option<&'a Message> {
        self.store.read(cx).state().history(self.key())?.message(id)
    }

    pub(super) fn focus_field(&self, window: &mut Window, cx: &mut Context<Self>) {
        let field = self.compose.field.clone();
        field.update(cx, |field, cx| field.focus(window, cx));
    }

    pub(super) fn reply_to(
        &mut self,
        message: MessageId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Replying drops a forward, as `HistoryWidget::replyToMessage` does.
        match &mut self.compose.bar {
            Some(FieldBar::Edit { reply, .. }) => *reply = Some(message),
            bar => *bar = Some(FieldBar::Reply(message)),
        }
        self.focus_field(window, cx);
        cx.notify();
    }

    pub(super) fn edit(&mut self, message: MessageId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self
            .bar_message(message, cx)
            .and_then(editable_text)
            .map(str::to_owned)
        else {
            return;
        };
        let field = self.compose.field.clone();
        let (stash, reply) = match self.compose.bar.take() {
            Some(FieldBar::Edit { stash, reply, .. }) => (stash, reply),
            Some(FieldBar::Reply(reply)) => (field.read(cx).text().to_owned(), Some(reply)),
            Some(FieldBar::Forward(_)) | None => (field.read(cx).text().to_owned(), None),
        };
        self.compose.bar = Some(FieldBar::Edit {
            message,
            stash,
            reply,
        });
        field.update(cx, |field, cx| field.set_text(&text, cx));
        self.focus_field(window, cx);
        cx.notify();
    }

    /// Escape or the cancel button; `false` when no bar was open.
    pub(super) fn cancel_field_bar(&mut self, cx: &mut Context<Self>) -> bool {
        match self.compose.bar.take() {
            None => return false,
            Some(FieldBar::Reply(_) | FieldBar::Forward(_)) => {}
            Some(FieldBar::Edit { stash, reply, .. }) => {
                self.compose.bar = reply.map(FieldBar::Reply);
                let field = self.compose.field.clone();
                field.update(cx, |field, cx| field.set_text(&stash, cx));
            }
        }
        cx.notify();
        true
    }

    /// Saves the edited text and gives the field back what it held; puts
    /// the edit back if TDLib refused it.
    pub(super) fn save_edit(&mut self, message: MessageId, text: String, cx: &mut Context<Self>) {
        let Some(FieldBar::Edit { stash, reply, .. }) = self.compose.bar.take() else {
            return;
        };
        self.compose.bar = reply.map(FieldBar::Reply);
        let field = self.compose.field.clone();
        field.update(cx, |field, cx| field.set_text(&stash, cx));
        let task = spawn(
            self.store
                .read(cx)
                .session()
                .edit_text(self.chat, message, &text),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let Err(error) = task.await else {
                return;
            };
            tracing::warn!(%error, "editing a message failed");
            // The view is gone once its chat closed; the edit goes with it.
            let _ = this.update(cx, |this, cx| {
                if matches!(this.compose.bar, Some(FieldBar::Edit { .. })) {
                    return;
                }
                let field = this.compose.field.clone();
                this.compose.bar = Some(FieldBar::Edit {
                    message,
                    stash: field.read(cx).text().to_owned(),
                    reply: match this.compose.bar {
                        Some(FieldBar::Reply(reply)) => Some(reply),
                        _ => None,
                    },
                });
                field.update(cx, |field, cx| field.set_text(&text, cx));
                cx.notify();
            });
        })
        .detach();
    }

    /// Up in an empty field: the newest own message still editable
    /// (`History::lastEditableMessage`).
    pub(super) fn edit_last(&mut self, _: &EditLast, window: &mut Window, cx: &mut Context<Self>) {
        if self.compose.bar.is_some() {
            return;
        }
        let state = self.store.read(cx).state();
        let is_self = state
            .chats
            .chat(self.chat)
            .is_some_and(|chat| chat.user().is_some() && chat.user() == state.me);
        let now = i32::try_from(chrono::Utc::now().timestamp()).unwrap_or(i32::MAX);
        let last = self.shown.iter().rev().find(|message| {
            message.is_outgoing
                && message.sending.is_none()
                && (is_self || now - message.date < EDIT_TIME_LIMIT)
                && editable_text(message).is_some()
        });
        if let Some(id) = last.map(|message| message.id) {
            self.edit(id, window, cx);
        }
    }

    /// The messages a reply may move through, oldest first.
    fn repliable(&self) -> impl DoubleEndedIterator<Item = MessageId> + '_ {
        self.shown
            .iter()
            .filter(|message| message.sending.is_none() && !is_service(message))
            .map(|message| message.id)
    }

    pub(super) fn reply_previous(
        &mut self,
        _: &ReplyPrevious,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = match self.compose.bar {
            Some(FieldBar::Reply(id)) => Some(id),
            // A key press shouldn't drop an edit or a forward.
            Some(FieldBar::Edit { .. } | FieldBar::Forward(_)) => return,
            None => None,
        };
        let target = self
            .repliable()
            .rev()
            .find(|id| current.is_none_or(|current| *id < current));
        if let Some(target) = target {
            self.jump_to(target, cx);
            self.reply_to(target, window, cx);
        }
    }

    pub(super) fn reply_next(
        &mut self,
        _: &ReplyNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(FieldBar::Reply(current)) = self.compose.bar else {
            return;
        };
        let next = self.repliable().find(|id| *id > current);
        match next {
            Some(target) => {
                self.jump_to(target, cx);
                self.reply_to(target, window, cx);
            }
            None => {
                self.cancel_field_bar(cx);
            }
        }
    }

    /// The bar's icon, its two lines, and the message a click shows.
    fn bar_lines(&self, cx: &App) -> Option<(Icon, String, String, Option<MessageId>)> {
        let strings = strings(cx);
        let (icon, id) = match self.compose.bar.as_ref()? {
            FieldBar::Reply(id) => (HISTORY_REPLY_ICON, *id),
            FieldBar::Edit { message, .. } => (HISTORY_EDIT_ICON, *message),
            // shortcut: no menu to hide sender names or captions; with forward options in M5.
            FieldBar::Forward(forwarding) => {
                let (title, text) = (forwarding.title.clone(), forwarding.text.clone());
                return Some((HISTORY_FORWARD_ICON, title, text, None));
            }
        };
        let found = self.bar_message(id, cx);
        let title = match self.compose.bar {
            Some(FieldBar::Edit { .. }) => strings.text(key::lng_edit_message).to_owned(),
            _ => found.map_or_else(String::new, |message| {
                let chats = &self.store.read(cx).state().chats;
                let name = from(strings, chats, message);
                i18n::fill(strings.text(key::lng_preview_reply_to), &[("name", &name)])
            }),
        };
        let text = found.map_or_else(
            || strings.text(key::lng_profile_loading).to_owned(),
            |message| one_line(strings, message),
        );
        Some((icon, title, text, Some(id)))
    }

    /// The bar over the field, if one is open.
    pub(super) fn field_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (icon, title, text, target) = self.bar_lines(cx)?;
        let weak = cx.weak_entity();
        let cancel =
            IconButton::new("field-bar-cancel", &HISTORY_REPLY_CANCEL).on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.cancel_field_bar(cx));
            });
        let weak = cx.weak_entity();
        let lines = div()
            .id("field-bar-message")
            .flex_1()
            .min_w_0()
            .h_full()
            .pl(px_of(HISTORY_REPLY_SKIP))
            .pt(px_of(MSG_REPLY_PADDING.top))
            .pr(px_of(MSG_REPLY_PADDING.right))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                if let Some(id) = target {
                    let _ = weak.update(cx, |this, cx| this.jump_to(id, cx));
                }
            })
            .child(line(
                title.into(),
                MSG_SERVICE_NAME_FONT,
                color(cx, HISTORY_REPLY_NAME_FG),
            ))
            .child(line(
                text.into(),
                NORMAL_FONT,
                color(cx, ColorRef::Palette(ColorId::HistoryComposeAreaFg)),
            ));
        Some(
            div()
                .relative()
                .flex()
                .h(px_of(HISTORY_REPLY_HEIGHT))
                .child(
                    div()
                        .absolute()
                        .left(px_of(HISTORY_REPLY_ICON_POSITION.x))
                        .top(px_of(HISTORY_REPLY_ICON_POSITION.y))
                        .child(ui::icon(icon)),
                )
                .child(lines)
                .child(cancel)
                .into_any_element(),
        )
    }
}
