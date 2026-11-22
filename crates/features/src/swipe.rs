use gpui::{
    App, Div, Pixels, Point, ScrollDelta, ScrollEvent, TouchPhase, Window, div, px, relative,
};
use gpui::{ParentElement as _, Styled as _};
use style::{ColorId, ColorRef, Icon};
use ui::anim::{Easing, Simple};
use ui::{color, icon};

const THRESHOLD: f32 = 50.0;
const BADGE_SIZE: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Scroll {
    Pass,
    Horizontal,
    Released(Direction),
}

#[derive(Default)]
pub(crate) struct Swipe {
    tracking: Option<Tracking>,
}

struct Tracking {
    total: Point<Pixels>,
    horizontal: Option<bool>,
}

impl Swipe {
    pub(crate) fn progress(&self) -> f32 {
        self.tracking
            .as_ref()
            .filter(|tracking| tracking.horizontal == Some(true))
            .map_or(0.0, |tracking| {
                (f32::from(tracking.total.x) / THRESHOLD).clamp(-1.0, 1.0)
            })
    }

    pub(crate) fn scroll(&mut self, event: &ScrollEvent) -> Scroll {
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
        if horizontal && x >= px(THRESHOLD) && x > y {
            Scroll::Released(direction)
        } else if horizontal {
            Scroll::Horizontal
        } else {
            Scroll::Pass
        }
    }
}

#[derive(Default)]
pub(crate) struct Feedback {
    progress: f32,
    retract: Simple,
}

impl Feedback {
    pub(crate) fn set(&mut self, progress: f32) -> bool {
        if self.progress == progress && !self.retract.animating() {
            return false;
        }
        self.retract.stop();
        self.progress = progress;
        true
    }

    pub(crate) fn finish(&mut self) -> bool {
        if self.progress == 0.0 {
            return false;
        }
        self.retract
            .start(self.progress, 0.0, 150, Easing::EaseOutCirc);
        self.progress = 0.0;
        true
    }

    pub(crate) fn value(&self) -> f32 {
        self.retract.value(self.progress)
    }

    pub(crate) fn animate(&self, window: &Window) {
        if self.retract.animating() {
            window.request_animation_frame();
        }
    }
}

fn badge(icon_style: Icon, progress: f32, cx: &App) -> Div {
    let foreground = ColorRef::Palette(ColorId::MsgServiceFg);
    let opacity = progress.sqrt();
    div()
        .absolute()
        .top(relative(0.5))
        .mt(px(-BADGE_SIZE / 2.0))
        .size(px(BADGE_SIZE))
        .rounded_full()
        .border_1()
        .border_color(color(cx, foreground).opacity(0.2 + progress * 0.5))
        .bg(color(cx, ColorRef::Palette(ColorId::MsgServiceBg)))
        .flex()
        .items_center()
        .justify_center()
        .opacity(opacity)
        .child(icon(icon_style).color(foreground))
}

pub(crate) fn back_badge(progress: f32, cx: &App) -> Option<Div> {
    (progress > 0.0).then(|| {
        badge(style::info::HISTORY_TOP_BAR_BACK.icon, progress, cx)
            .left(px(-BADGE_SIZE + (BADGE_SIZE + 8.0) * progress))
    })
}

pub(crate) fn reply_badge(outgoing: bool, progress: f32, cx: &App) -> Div {
    let gap = px(4.0 + 8.0 * (1.0 - progress));
    let badge = badge(style::menu_icons::MENU_ICON_REPLY, progress, cx);
    if outgoing {
        badge.left(px(-BADGE_SIZE) - gap)
    } else {
        badge.right(px(-BADGE_SIZE) - gap)
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
    fn progress_follows_distance_and_resets_on_cancel() {
        let mut swipe = Swipe::default();
        swipe.scroll(&at(TouchPhase::Started, 2.0, 0.0));
        assert_eq!(swipe.progress(), 0.0);
        swipe.scroll(&at(TouchPhase::Moved, -27.0, 0.0));
        assert_eq!(swipe.progress(), -0.5);
        swipe.scroll(&at(TouchPhase::Moved, -50.0, 0.0));
        assert_eq!(swipe.progress(), -1.0);
        swipe.scroll(&at(TouchPhase::Cancelled, 0.0, 0.0));
        assert_eq!(swipe.progress(), 0.0);
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
