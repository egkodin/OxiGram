//! The seek ring on a paused round video (`HistoryView::VideoMessageSeek`):
//! a wider track around the edge with the played part and a dot; a press
//! in the band along the edge grabs it, a drag moves around the circle,
//! and the release seeks there.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, Context, DispatchPhase, Hsla, IntoElement as _, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Styled as _, Window, canvas,
    point,
};
use model::MessageId;
use style::chat::{
    HISTORY_VIDEO_MESSAGE_SEEK_DOT_SIZE, HISTORY_VIDEO_MESSAGE_SEEK_DOT_SIZE_GRABBED,
    HISTORY_VIDEO_MESSAGE_SEEK_GRAB_BAND, HISTORY_VIDEO_MESSAGE_SEEK_INSET,
    HISTORY_VIDEO_MESSAGE_SEEK_LINE,
};
use ui::{RadialState, paint_arc};

use super::HistoryView;
use crate::px_of;

/// `kTrackOpacity`: the unplayed track under the played part.
const TRACK_OPACITY: f32 = 0.2;
/// `kSeekPreviewInterval`: how often a drag shows the frame it is over.
const PREVIEW_INTERVAL: Duration = Duration::from_millis(100);
/// A full turn and a quarter in Qt's sixteenths of a degree.
const FULL_ARC: i64 = 360 * 16;
const QUARTER_ARC: i64 = 90 * 16;

/// What the pointer does to the ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Seek {
    Grab(f32),
    Drag(f32),
    Release(f32),
}

pub(super) type OnSeek = Rc<dyn Fn(Seek, &mut Window, &mut App)>;

/// A round's ring while it plays in the bubble.
pub(super) struct Ring {
    /// The played part, or where the drag is, 0 to 1.
    pub played: f32,
    /// Paused or seeking: the wide track with its dot, which takes presses.
    pub is_shown: bool,
    pub is_grabbed: bool,
    pub on_seek: OnSeek,
}

/// A drag in progress on a round.
#[derive(Debug, Clone, Copy)]
pub(super) struct Seeking {
    pub message: MessageId,
    pub share: f32,
    previewed: Instant,
}

/// Whether `point` lies in the band along the circle's edge (`grabPoint`).
fn grabs(bounds: Bounds<Pixels>, point: Point<Pixels>) -> bool {
    let radius = f32::from(bounds.size.width) / 2.0;
    let band = f32::from(px_of(HISTORY_VIDEO_MESSAGE_SEEK_GRAB_BAND));
    if radius <= band {
        return false;
    }
    let distance = distance(bounds.center(), point);
    distance >= radius - band && distance <= radius
}

fn distance(center: Point<Pixels>, point: Point<Pixels>) -> f32 {
    let (dx, dy) = (f32::from(point.x - center.x), f32::from(point.y - center.y));
    dx.hypot(dy)
}

/// How far round the circle `point` is, clockwise from the top
/// (`updateRoundSeeking`).
fn share_at(bounds: Bounds<Pixels>, point: Point<Pixels>) -> f32 {
    let center = bounds.center();
    let (dx, dy) = (f32::from(point.x - center.x), f32::from(point.y - center.y));
    let angle = (-dy).atan2(dx);
    ((FRAC_PI_2 - angle) / TAU + 1.0)
        .rem_euclid(1.0)
        .clamp(0.0, 1.0)
}

/// The ring over the round: the thin played arc while it plays, the seek
/// track with its dot while paused or dragged.
pub(super) fn ring(ring: Ring, fg: Hsla, opacity: f32) -> AnyElement {
    if !ring.is_shown {
        return super::file_row::arc(ring.played, fg.opacity(opacity));
    }
    let Ring {
        played,
        is_grabbed,
        on_seek,
        ..
    } = ring;
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            paint(window, bounds, (played, is_grabbed), (fg, opacity));
            listen(window, bounds, is_grabbed, &on_seek);
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

fn paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    (played, is_grabbed): (f32, bool),
    (fg, opacity): (Hsla, f32),
) {
    let line = px_of(HISTORY_VIDEO_MESSAGE_SEEK_LINE);
    let arc = bounds.dilate(-px_of(HISTORY_VIDEO_MESSAGE_SEEK_INSET));
    let full = RadialState {
        shown: 1.0,
        arc_from: 0,
        arc_length: FULL_ARC,
    };
    paint_arc(window, arc, full, line, fg.opacity(TRACK_OPACITY));
    #[expect(clippy::cast_possible_truncation, reason = "within a full turn")]
    let length = -(FULL_ARC as f32 * played).round() as i64;
    let state = RadialState {
        shown: 1.0,
        arc_from: QUARTER_ARC,
        arc_length: length,
    };
    paint_arc(window, arc, state, line, fg.opacity(opacity));
    let dot = px_of(if is_grabbed {
        HISTORY_VIDEO_MESSAGE_SEEK_DOT_SIZE_GRABBED
    } else {
        HISTORY_VIDEO_MESSAGE_SEEK_DOT_SIZE
    });
    let angle = FRAC_PI_2 - played * 2.0 * PI;
    let radius = arc.size.width / 2.0;
    let center = arc.center();
    let at = point(
        center.x + radius * angle.cos(),
        center.y - radius * angle.sin(),
    );
    let square = Bounds::centered_at(at, gpui::size(dot, dot));
    window.paint_quad(gpui::fill(square, fg).corner_radii(dot / 2.0));
}

/// Presses in the band grab the ring before the bubble's click sees them.
fn listen(window: &mut Window, bounds: Bounds<Pixels>, is_grabbed: bool, on_seek: &OnSeek) {
    let on_down = on_seek.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase == DispatchPhase::Capture
            && event.button == MouseButton::Left
            && grabs(bounds, event.position)
        {
            cx.stop_propagation();
            on_down(Seek::Grab(share_at(bounds, event.position)), window, cx);
        }
    });
    if !is_grabbed {
        return;
    }
    let on_move = on_seek.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble {
            on_move(Seek::Drag(share_at(bounds, event.position)), window, cx);
        }
    });
    let on_up = on_seek.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
            cx.stop_propagation();
            on_up(Seek::Release(share_at(bounds, event.position)), window, cx);
        }
    });
}

impl HistoryView {
    /// Where a drag on `message`'s ring is, while one runs.
    pub(super) fn round_seeking(&self, message: MessageId) -> Option<f32> {
        self.round_seek
            .filter(|seeking| seeking.message == message)
            .map(|seeking| seeking.share)
    }

    /// What the pointer on `message`'s ring does: a grab and a drag show
    /// the frame there every 100 ms, the release seeks.
    pub(super) fn on_round_seek(&self, message: MessageId, cx: &Context<Self>) -> OnSeek {
        let weak = cx.weak_entity();
        Rc::new(move |seek, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                match seek {
                    Seek::Grab(share) => {
                        this.round_seek = Some(Seeking {
                            message,
                            share,
                            previewed: Instant::now(),
                        });
                        this.seek_inline(message, share, cx);
                    }
                    Seek::Drag(share) => {
                        let Some(seeking) = this.round_seek.as_mut() else {
                            return;
                        };
                        seeking.share = share;
                        if seeking.previewed.elapsed() >= PREVIEW_INTERVAL {
                            seeking.previewed = Instant::now();
                            this.seek_inline(message, share, cx);
                        }
                    }
                    Seek::Release(share) => {
                        this.round_seek = None;
                        this.seek_inline(message, share, cx);
                    }
                }
                cx.notify();
            });
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    fn circle() -> Bounds<Pixels> {
        Bounds::new(point(px(0.0), px(0.0)), gpui::size(px(240.0), px(240.0)))
    }

    #[test]
    fn the_band_along_the_edge_grabs() {
        assert!(grabs(circle(), point(px(120.0), px(10.0))));
        assert!(!grabs(circle(), point(px(120.0), px(120.0))));
        assert!(!grabs(circle(), point(px(0.0), px(0.0))));
    }

    #[test]
    fn the_share_runs_clockwise_from_the_top() {
        let at = |x: f32, y: f32| share_at(circle(), point(px(x), px(y)));
        assert!(at(120.0, 0.0).abs() < 0.01);
        assert!((at(240.0, 120.0) - 0.25).abs() < 0.01);
        assert!((at(120.0, 240.0) - 0.5).abs() < 0.01);
        assert!((at(0.0, 120.0) - 0.75).abs() < 0.01);
    }
}
