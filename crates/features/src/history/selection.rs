//! Selecting in a history (`HistoryInner::mouseActionStart` and friends):
//! dragging inside one message selects its text, dragging across messages
//! selects whole messages, and Ctrl+click (Cmd+click on macOS) toggles one.

use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use chrono::{Local, TimeZone as _};
use gpui::{App, Context, Window};
use model::MessageId;
use model::message::{Content, Message};

use super::HistoryView;

/// What is selected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum Selection {
    #[default]
    None,
    /// Part of one message's text, from `anchor` to `head` in either order.
    Text {
        message: MessageId,
        anchor: usize,
        head: usize,
    },
    Messages(BTreeSet<MessageId>),
}

impl Selection {
    /// The selected byte range of `message`'s text, unless empty.
    pub(super) fn text_range(&self, message: MessageId) -> Option<Range<usize>> {
        match *self {
            Self::Text {
                message: selected,
                anchor,
                head,
            } if selected == message && anchor != head => Some(anchor.min(head)..anchor.max(head)),
            _ => None,
        }
    }

    pub(super) fn is_message_selected(&self, message: MessageId) -> bool {
        matches!(self, Self::Messages(messages) if messages.contains(&message))
    }

    pub(super) fn selected_count(&self) -> usize {
        match self {
            Self::Messages(messages) => messages.len(),
            Self::None | Self::Text { .. } => 0,
        }
    }
}

/// What the mouse did over a message, as its row and text report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pointer {
    /// Pressed over the text, at this byte offset.
    PressText(usize),
    /// Pressed elsewhere on the message's row.
    PressRow,
    /// Moved with the left button down, over the text at an offset or
    /// elsewhere on the row.
    Drag(Option<usize>),
    /// Moved with no button down.
    Hover,
    /// Released; `is_toggle` when Ctrl (Cmd on macOS) was held.
    Release { is_toggle: bool },
}

/// Mouse events of one message, bound to its index.
pub(super) type OnPointer = Rc<dyn Fn(Pointer, &mut Window, &mut App)>;

/// A drag in progress, from the message at `start`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Drag {
    start: usize,
    is_text: bool,
    has_moved: bool,
    /// The messages selected before the drag, which it adds to.
    before: BTreeSet<MessageId>,
}

/// The ids of the messages from index `a` to `b`, either order; service
/// messages are not selectable.
fn messages_between(shown: &[Arc<Message>], a: usize, b: usize) -> BTreeSet<MessageId> {
    let range = a.min(b)..=a.max(b).min(shown.len().saturating_sub(1));
    shown
        .get(range)
        .unwrap_or_default()
        .iter()
        .filter(|message| !matches!(message.content, Content::Service(_)))
        .map(|message| message.id)
        .collect()
}

fn message_text(message: &Message) -> Option<&str> {
    match &message.content {
        Content::Text(text) => Some(&text.text),
        _ => None,
    }
}

/// The text Ctrl+C copies: the selected part, one message's text, or
/// "[date] Name: text" lines for several (`HistorySelectedItemsTextParts`).
// shortcut: the date uses one fixed short format, not the system locale's, and copies leave out reply and forward context; both come with M8 polish.
pub(super) fn copied_text(
    selection: &Selection,
    shown: &[Arc<Message>],
    author: impl Fn(&Message) -> String,
) -> Option<String> {
    match selection {
        Selection::None => None,
        Selection::Text { message, .. } => {
            let range = selection.text_range(*message)?;
            let message = shown.iter().find(|shown| shown.id == *message)?;
            message_text(message)?.get(range).map(str::to_owned)
        }
        Selection::Messages(ids) => {
            let selected: Vec<&Message> = shown
                .iter()
                .filter(|message| ids.contains(&message.id))
                .map(|message| &**message)
                .collect();
            if let [only] = selected[..] {
                return message_text(only).map(str::to_owned);
            }
            let lines: Vec<String> = selected
                .iter()
                .map(|message| {
                    let date = Local
                        .timestamp_opt(i64::from(message.date), 0)
                        .single()
                        .map(|date| date.format("%d.%m.%y %H:%M").to_string())
                        .unwrap_or_default();
                    let text = message_text(message).unwrap_or_default();
                    format!("[{date}] {}: {text}", author(message))
                })
                .collect();
            (!lines.is_empty()).then(|| lines.join("\n"))
        }
    }
}

impl HistoryView {
    pub(super) fn on_pointer(&mut self, index: usize, pointer: Pointer, cx: &mut Context<Self>) {
        let before = self.selection.clone();
        match pointer {
            Pointer::PressText(offset) => self.press(index, Some(offset)),
            Pointer::PressRow => self.press(index, None),
            Pointer::Drag(offset) => self.drag_to(index, offset),
            Pointer::Hover => self.drag = None,
            Pointer::Release { is_toggle } => self.release(is_toggle),
        }
        if self.selection != before {
            cx.notify();
        }
    }

    fn press(&mut self, index: usize, offset: Option<usize>) {
        let Some(message) = self.shown.get(index) else {
            return;
        };
        let before = match &self.selection {
            Selection::Messages(messages) => messages.clone(),
            Selection::None | Selection::Text { .. } => BTreeSet::new(),
        };
        // In selection mode a press toggles messages, not text.
        let text_offset = offset.filter(|_| before.is_empty());
        if let Some(offset) = text_offset {
            self.selection = Selection::Text {
                message: message.id,
                anchor: offset,
                head: offset,
            };
        }
        self.drag = Some(Drag {
            start: index,
            is_text: text_offset.is_some(),
            has_moved: false,
            before,
        });
    }

    fn drag_to(&mut self, index: usize, offset: Option<usize>) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        if drag.is_text && index == drag.start {
            if let (Some(offset), Selection::Text { head, .. }) = (offset, &mut self.selection) {
                drag.has_moved |= *head != offset;
                *head = offset;
            }
            return;
        }
        if index == drag.start && !drag.has_moved {
            return;
        }
        drag.is_text = false;
        drag.has_moved = true;
        let mut messages = drag.before.clone();
        messages.extend(messages_between(&self.shown, drag.start, index));
        self.selection = Selection::Messages(messages);
    }

    fn release(&mut self, is_toggle: bool) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if drag.has_moved {
            return;
        }
        let in_selection_mode = !drag.before.is_empty();
        if !is_toggle && !in_selection_mode {
            // A click without a drag drops a text selection.
            self.selection = Selection::None;
            return;
        }
        let Some(message) = self.shown.get(drag.start) else {
            return;
        };
        let mut messages = drag.before;
        if !messages.remove(&message.id) {
            messages.insert(message.id);
        }
        self.selection = if messages.is_empty() {
            Selection::None
        } else {
            Selection::Messages(messages)
        };
    }

    /// Escape: drops the selection; `false` when there was none.
    pub(crate) fn clear_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had_selection = self.selection != Selection::None;
        self.selection = Selection::None;
        self.drag = None;
        if had_selection {
            cx.notify();
        }
        had_selection
    }

    /// Ctrl+C: copies the selection; `false` when nothing is selected.
    pub(crate) fn copy_selection(&self, cx: &mut App) -> bool {
        let state = self.store.read(cx).state();
        let strings = crate::lang::strings(cx);
        let text = copied_text(&self.selection, &self.shown, |message| {
            super::service::from(strings, &state.chats, message)
        });
        let Some(text) = text else {
            return false;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::message::Sender;
    use model::text::FormattedText;
    use model::{ChatId, UserId};

    fn shown(texts: &[&str]) -> Vec<Arc<Message>> {
        texts
            .iter()
            .zip(1..)
            .map(|(text, id)| {
                Arc::new(Message::new(
                    MessageId(id),
                    ChatId(1),
                    Sender::User(UserId(1)),
                    Content::Text(FormattedText::plain(*text)),
                ))
            })
            .collect()
    }

    #[test]
    fn a_text_selection_copies_its_part_either_way() {
        let shown = shown(&["hello world"]);
        let selection = Selection::Text {
            message: MessageId(1),
            anchor: 11,
            head: 6,
        };
        assert_eq!(
            copied_text(&selection, &shown, |_| String::new()).as_deref(),
            Some("world")
        );
    }

    #[test]
    fn one_message_copies_bare_and_several_get_names() {
        let shown = shown(&["first", "second"]);
        let one = Selection::Messages([MessageId(2)].into());
        assert_eq!(
            copied_text(&one, &shown, |_| "Ann".to_owned()).as_deref(),
            Some("second")
        );
        let both = Selection::Messages(messages_between(&shown, 1, 0));
        let copied = copied_text(&both, &shown, |_| "Ann".to_owned()).unwrap_or_default();
        let lines: Vec<&str> = copied.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("] Ann: first"));
        assert!(lines[1].ends_with("] Ann: second"));
    }
}
