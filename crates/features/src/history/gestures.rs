use gpui::{Context, ScrollEvent, Task, TouchPhase, Window};
use model::MessageId;
use model::rights::Writing;

use super::{HistoryEvent, HistoryView, pinned_section::Section, selection::Selection};

use crate::swipe::{Direction, Feedback, Scroll, Swipe};

#[derive(Default)]
pub(super) struct ChatGestures {
    swipe: Swipe,
    pub(super) feedback: Feedback,
    row_under_start: Option<MessageId>,
    target: Option<MessageId>,
    reply: Option<Task<()>>,
}

impl ChatGestures {
    pub(super) fn reply_progress(&self, id: MessageId) -> f32 {
        if self.target == Some(id) {
            (-self.feedback.value()).max(0.0)
        } else {
            0.0
        }
    }
}

impl HistoryView {
    pub(super) fn begin_row_swipe(&mut self, id: MessageId) {
        self.gestures.row_under_start = Some(id);
    }

    pub(super) fn on_history_scroll(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.touch_phase == TouchPhase::Started {
            self.gestures.target = self.gestures.row_under_start.take();
            if self.gestures.feedback.set(0.0) {
                cx.notify();
            }
        }
        if self.menu.is_some() || self.selection != Selection::None {
            self.gestures.swipe = Swipe::default();
            if self.gestures.feedback.finish() {
                cx.notify();
            }
            self.on_pull_scroll(event, cx);
            return;
        }
        let action = self.gestures.swipe.scroll(event);
        let progress = self.gestures.swipe.progress();
        let changed = if progress > 0.0
            || (progress < 0.0 && self.gestures.target.is_some() && self.can_swipe_reply(cx))
        {
            self.gestures.feedback.set(progress)
        } else {
            self.gestures.feedback.finish()
        };
        if changed {
            cx.notify();
        }
        match action {
            Scroll::Pass => self.on_pull_scroll(event, cx),
            Scroll::Horizontal => self.drag = None,
            Scroll::Released(Direction::Right) => {
                self.gestures.reply = None;
                if !self.close_search(cx) && !self.close_section(cx) {
                    cx.emit(HistoryEvent::Close);
                }
            }
            Scroll::Released(Direction::Left) => {
                if let Some(id) = self.gestures.target.filter(|_| self.can_swipe_reply(cx)) {
                    self.swipe_reply(id, window, cx);
                }
            }
        }
    }

    fn can_swipe_reply(&self, cx: &Context<Self>) -> bool {
        self.section == Section::Chat
            && self.search.is_none()
            && self.menu.is_none()
            && self.selection == Selection::None
            && self.store.read(cx).state().chats.writing(self.chat) == Writing::Field
    }

    fn swipe_reply(&mut self, id: MessageId, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.generation;
        let abilities = self.abilities_of(&[id], cx);
        self.gestures.reply = Some(cx.spawn_in(window, async move |this, cx| {
            if abilities.await.first().is_some_and(|a| a.can_be_replied) {
                let _ = this.update_in(cx, |this, window, cx| {
                    if generation == this.generation
                        && this.can_swipe_reply(cx)
                        && this.shown.iter().any(|message| message.id == id)
                    {
                        this.reply_to(id, window, cx);
                    }
                });
            }
        }));
    }
}
