use gpui::{Context, Pixels, Point, ScrollDelta, ScrollEvent, Task, TouchPhase, Window, px};
use model::MessageId;
use model::rights::Writing;

use super::{HistoryEvent, HistoryView, pinned_section::Section, selection::Selection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Scroll {
    Pass,
    Horizontal,
    Released(Direction),
}

#[derive(Default)]
pub(super) struct Swipe {
    tracking: Option<Tracking>,
}

struct Tracking {
    total: Point<Pixels>,
    horizontal: Option<bool>,
}

impl Swipe {
    pub(super) fn scroll(&mut self, event: &ScrollEvent) -> Scroll {
        let ScrollDelta::Pixels(delta) = event.delta else {
            self.tracking = None;
            return Scroll::Pass;
        };
        if event.modifiers.modified() || event.touch_phase == TouchPhase::Cancelled {
            self.tracking = None;
            return Scroll::Pass;
        }
        if event.touch_phase == TouchPhase::Started {
            self.tracking = Some(Tracking {
                total: Point::default(),
                horizontal: None,
            });
        }
        let Some(tracking) = self.tracking.as_mut() else {
            return Scroll::Pass;
        };
        tracking.total += delta;
        let x = tracking.total.x.abs();
        let y = tracking.total.y.abs();
        if tracking.horizontal.is_none() && x.max(y) >= px(8.0) {
            tracking.horizontal = Some(x > y);
        }
        let horizontal = tracking.horizontal == Some(true);
        if event.touch_phase != TouchPhase::Ended {
            return if horizontal {
                Scroll::Horizontal
            } else {
                Scroll::Pass
            };
        }
        let direction = if tracking.total.x < px(0.0) {
            Direction::Left
        } else {
            Direction::Right
        };
        self.tracking = None;
        if horizontal && x >= px(50.0) && x > y {
            Scroll::Released(direction)
        } else if horizontal {
            Scroll::Horizontal
        } else {
            Scroll::Pass
        }
    }
}

#[derive(Default)]
pub(super) struct ChatGestures {
    swipe: Swipe,
    row_under_start: Option<MessageId>,
    target: Option<MessageId>,
    reply: Option<Task<()>>,
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
        }
        if self.menu.is_some() || self.selection != Selection::None {
            self.gestures.swipe = Swipe::default();
            self.on_pull_scroll(event, cx);
            return;
        }
        match self.gestures.swipe.scroll(event) {
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

#[cfg(test)]
mod tests {
    use gpui::point;

    use super::*;

    fn at(phase: TouchPhase, x: f32, y: f32) -> ScrollEvent {
        ScrollEvent {
            position: point(px(x), px(y)),
            delta: ScrollDelta::Pixels(point(px(x), px(y))),
            touch_phase: phase,
            ..ScrollEvent::default()
        }
    }

    #[test]
    fn releases_once_in_the_swipe_direction() {
        for (delta, direction) in [(-55.0, Direction::Left), (55.0, Direction::Right)] {
            let mut swipe = Swipe::default();
            let start = at(TouchPhase::Started, 0.0, 0.0);
            assert_eq!(swipe.scroll(&start), Scroll::Pass);
            assert_eq!(
                swipe.scroll(&at(TouchPhase::Moved, delta, 3.0)),
                Scroll::Horizontal
            );
            assert_eq!(
                swipe.scroll(&at(TouchPhase::Ended, 0.0, 0.0)),
                Scroll::Released(direction)
            );
            assert_eq!(
                swipe.scroll(&at(TouchPhase::Moved, delta, 0.0)),
                Scroll::Pass
            );
            assert_eq!(swipe.scroll(&at(TouchPhase::Ended, 0.0, 0.0)), Scroll::Pass);
        }
    }

    #[test]
    fn vertical_scroll_wheel_reversal_and_cancellation_never_release() {
        let mut swipe = Swipe::default();
        assert_eq!(
            swipe.scroll(&at(TouchPhase::Started, 1.0, 10.0)),
            Scroll::Pass
        );
        assert_eq!(
            swipe.scroll(&at(TouchPhase::Moved, -70.0, 1.0)),
            Scroll::Pass
        );
        assert_eq!(swipe.scroll(&at(TouchPhase::Ended, 0.0, 0.0)), Scroll::Pass);

        let mut wheel = at(TouchPhase::Started, -60.0, 0.0);
        wheel.delta = ScrollDelta::Lines(point(-60.0, 0.0));
        assert_eq!(swipe.scroll(&wheel), Scroll::Pass);
        assert_eq!(swipe.scroll(&at(TouchPhase::Ended, 0.0, 0.0)), Scroll::Pass);

        swipe.scroll(&at(TouchPhase::Started, -55.0, 0.0));
        assert_eq!(
            swipe.scroll(&at(TouchPhase::Ended, 35.0, 0.0)),
            Scroll::Horizontal
        );
        swipe.scroll(&at(TouchPhase::Started, -55.0, 0.0));
        assert_eq!(
            swipe.scroll(&at(TouchPhase::Cancelled, 0.0, 0.0)),
            Scroll::Pass
        );
        assert_eq!(swipe.scroll(&at(TouchPhase::Ended, 0.0, 0.0)), Scroll::Pass);
    }
}
