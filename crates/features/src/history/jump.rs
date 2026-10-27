//! Moving through a history: to the first unread message on open, to a
//! replied message and back, and down to the newest one
//! (`HistoryWidget::showAtPosition`, `_replyReturns`).

use std::time::Duration;

use gpui::{Context, ListOffset, px};
use model::MessageId;
use model::history::Page;
use style::basic::ACTIVE_FADE_OUT_DURATION;
use style::chat_helpers::HISTORY_TO_DOWN_SHOWN_AFTER;

use super::HistoryView;
use crate::px_of;

/// Where the list lands once the messages it waits for are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    /// The first incoming message after the last read one, under the
    /// unread bar.
    FirstUnread,
    Message(MessageId),
    /// The first message after this one, else the newest loaded.
    After(MessageId),
    Bottom,
}

impl HistoryView {
    /// The page a chat opens at: around the last read message when unread
    /// ones follow it, else the newest.
    pub(super) fn first_page(&mut self, cx: &Context<Self>) -> Page {
        let state = self.store.read(cx).state();
        match state.chats.chat(self.chat) {
            Some(chat) if chat.unread_count > 0 && chat.last_read_inbox.0 != 0 => {
                let last_read = chat.last_read_inbox;
                self.unread_bar = Some(last_read);
                self.target = Some(Target::FirstUnread);
                Page::Around(last_read)
            }
            _ => Page::Newest,
        }
    }

    /// Scrolls to the target once its message is shown.
    pub(super) fn land(&mut self) {
        let Some(target) = self.target else {
            return;
        };
        let index = match target {
            Target::Bottom => {
                self.list.scroll_to_end();
                self.target = None;
                return;
            }
            Target::FirstUnread => self.unread_bar_index,
            Target::Message(id) => self.shown.iter().position(|m| m.id >= id),
            Target::After(id) => self
                .shown
                .iter()
                .position(|m| m.id > id)
                .or_else(|| self.shown.len().checked_sub(1)),
        };
        if let Some(item_ix) = index {
            // shortcut: the target lands at the top; tdesktop centers a jumped-to message and flashes it.
            self.list.scroll_to(ListOffset {
                item_ix,
                offset_in_item: px(0.0),
            });
            self.target = None;
        }
    }

    pub(super) fn jump_to(&mut self, id: MessageId, cx: &mut Context<Self>) {
        self.highlight(id, cx);
        if let Some(item_ix) = self.shown.iter().position(|message| message.id == id) {
            self.list.scroll_to(ListOffset {
                item_ix,
                offset_in_item: px(0.0),
            });
            self.update_down_button(cx);
            cx.notify();
        } else {
            self.reload(Page::Around(id), Target::Message(id), cx);
        }
    }

    /// Shades the message jumped to for a moment.
    // shortcut: the shade goes at once instead of fading out (`ElementHighlighter`); the fade comes with M8 polish.
    fn highlight(&mut self, id: MessageId, cx: &mut Context<Self>) {
        self.highlight = Some(id);
        let duration = Duration::from_millis(ACTIVE_FADE_OUT_DURATION.unsigned_abs().into());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            let _ = this.update(cx, |this, cx| {
                if this.highlight == Some(id) {
                    this.highlight = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Scrolls to the message after `before`, or to the chat's first one
    /// when there is none before.
    pub(super) fn jump_after(&mut self, before: Option<MessageId>, cx: &mut Context<Self>) {
        let anchor = before.unwrap_or(MessageId(0));
        let has_oldest = self
            .store
            .read(cx)
            .state()
            .history(self.chat)
            .is_some_and(|history| history.older_page().is_none());
        let loaded = match before {
            Some(id) => self.shown.iter().position(|m| m.id == id),
            None => has_oldest.then_some(0),
        };
        match loaded {
            Some(index) => {
                let item_ix = if before.is_some() {
                    (index + 1).min(self.shown.len().saturating_sub(1))
                } else {
                    index
                };
                self.list.scroll_to(ListOffset {
                    item_ix,
                    offset_in_item: px(0.0),
                });
                self.update_down_button(cx);
                cx.notify();
            }
            None => self.reload(
                Page::Around(MessageId(anchor.0.max(1))),
                Target::After(anchor),
                cx,
            ),
        }
    }

    /// Opens a replied message; the down button comes back here first.
    pub(super) fn open_reply(&mut self, from: MessageId, to: MessageId, cx: &mut Context<Self>) {
        self.returns.push(from);
        self.jump_to(to, cx);
    }

    /// The down button: back from a reply, else to the newest message.
    pub(super) fn scroll_down(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.returns.pop() {
            self.jump_to(id, cx);
            return;
        }
        let has_newest = self
            .store
            .read(cx)
            .state()
            .history(self.chat)
            .is_some_and(model::history::History::has_newest);
        if has_newest {
            self.list.scroll_to_end();
            self.update_down_button(cx);
            cx.notify();
        } else {
            self.reload(Page::Newest, Target::Bottom, cx);
        }
    }

    /// Drops the loaded messages and loads `page` in their place.
    fn reload(&mut self, page: Page, target: Target, cx: &mut Context<Self>) {
        self.generation += 1;
        self.loading = None;
        self.target = Some(target);
        let chat = self.chat;
        self.store
            .update(cx, |store, cx| store.clear_history(chat, cx));
        self.load(page, cx);
    }

    /// Shows the down button far from the newest message.
    pub(super) fn update_down_button(&mut self, cx: &mut Context<Self>) {
        let has_newest = self
            .store
            .read(cx)
            .state()
            .history(self.chat)
            .is_some_and(model::history::History::has_newest);
        let below =
            self.list.max_offset_for_scrollbar().y + self.list.scroll_px_offset_for_scrollbar().y;
        let is_shown =
            !has_newest || !self.returns.is_empty() || below > px_of(HISTORY_TO_DOWN_SHOWN_AFTER);
        if is_shown != self.is_down_shown {
            self.is_down_shown = is_shown;
            cx.notify();
        }
    }
}
