//! lib_ui's `InfiniteRadialAnimation`: the spinning arc of a loading state,
//! whose length breathes between `arc_min` and `arc_max` of a circle.

use std::f32::consts::{PI, TAU};
use std::time::{Duration, Instant};

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, point};
use style::widgets::InfiniteRadialAnimation;

/// A full circle in Qt's arc units (1/16 of a degree).
const FULL: i64 = 360 * 16;
/// Vertices per full circle of a painted arc.
const ARC_SEGMENTS: f32 = 96.0;

/// Where to draw the arc: Qt's `drawArc` angles, counterclockwise from three
/// o'clock in 1/16 of a degree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialState {
    pub shown: f32,
    pub arc_from: i64,
    pub arc_length: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct InfiniteRadial {
    style: &'static InfiniteRadialAnimation,
    work_started: Option<Instant>,
}

impl InfiniteRadial {
    #[must_use]
    pub const fn new(style: &'static InfiniteRadialAnimation) -> Self {
        Self {
            style,
            work_started: None,
        }
    }

    /// Fades the arc in over `sine_duration`; does nothing while running.
    pub fn start(&mut self) {
        if self.work_started.is_none() {
            self.work_started = Some(Instant::now() + millis(self.style.sine_duration));
        }
    }

    /// Hides the arc at once (`stop(anim::type::instant)`).
    // shortcut: no animated stop; add the fade-out phase when a widget needs it.
    pub fn stop(&mut self) {
        self.work_started = None;
    }

    #[must_use]
    pub const fn animating(&self) -> bool {
        self.work_started.is_some()
    }

    /// The arc now, asking for the next frame; `None` when stopped.
    pub fn state_in(&self, window: &Window) -> Option<RadialState> {
        let state = self.state(Instant::now())?;
        window.request_animation_frame();
        Some(state)
    }

    fn state(&self, now: Instant) -> Option<RadialState> {
        let st = self.style;
        let work_started = self.work_started?;
        let zero = work_started.checked_sub(millis(st.sine_duration))?;
        let since_zero = ms(now.saturating_duration_since(zero));
        let linear = FULL - (since_zero * FULL / i64::from(st.linear_period.max(1))) % FULL;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "arc lengths are below FULL"
        )]
        let (min, max) = (
            (FULL as f64 * st.arc_min).round() as i64,
            (FULL as f64 * st.arc_max).round() as i64,
        );
        let sine_duration = i64::from(st.sine_duration.max(1));
        if now <= work_started {
            #[expect(clippy::cast_precision_loss, reason = "durations are small")]
            let shown = (since_zero as f32 / sine_duration as f32).clamp(0.0, 1.0);
            return Some(RadialState {
                shown,
                arc_from: linear,
                arc_length: interpolate(FULL, min, sine_in_out(shown)),
            });
        }
        let working = ms(now - work_started);
        let period = i64::from(st.sine_period.max(1));
        let (cycles, relative) = (working / period, working % period);
        let small = i64::from(st.sine_shift) - sine_duration;
        let basic = (linear + min + cycles * (FULL + min - max)) % FULL;
        #[expect(clippy::cast_precision_loss, reason = "durations are small")]
        let progress = |from: i64| sine_in_out((relative - from) as f32 / sine_duration as f32);
        let (arc_from, arc_length) = if relative <= small {
            (basic - min, min)
        } else if relative <= small + sine_duration {
            let length = interpolate(min, max, progress(small));
            (basic - length, length)
        } else if relative <= period - sine_duration {
            (basic - max, max)
        } else {
            let shrink = interpolate(0, max - min, progress(period - sine_duration));
            (basic - max, max - shrink)
        };
        Some(RadialState {
            shown: 1.0,
            arc_from,
            arc_length,
        })
    }
}

fn millis(value: i32) -> Duration {
    Duration::from_millis(u64::try_from(value).unwrap_or(0))
}

fn ms(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

/// `anim::sineInOut(1., t)`.
fn sine_in_out(t: f32) -> f32 {
    -0.5 * ((PI * t).cos() - 1.0)
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "arc lengths are small"
)]
fn interpolate(from: i64, to: i64, t: f32) -> i64 {
    (from as f32 + (to - from) as f32 * t).round() as i64
}

/// Strokes the arc of `state` on the ellipse inscribed in `bounds`, with
/// round caps, as `QPainter::drawArc` with a `RoundCap` pen does.
pub fn paint_arc(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    state: RadialState,
    thickness: Pixels,
    color: Hsla,
) {
    if state.arc_length == 0 {
        return;
    }
    let center = bounds.center();
    let radius = bounds.size.width.min(bounds.size.height) / 2.0;
    #[expect(clippy::cast_precision_loss, reason = "arc angles are small")]
    let (from, length) = (
        state.arc_from as f32 / FULL as f32 * TAU,
        state.arc_length as f32 / FULL as f32 * TAU,
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive count below ARC_SEGMENTS"
    )]
    let segments = (length.abs() / TAU * ARC_SEGMENTS).ceil().max(1.0) as usize;
    #[expect(clippy::cast_precision_loss, reason = "segment counts are small")]
    let points: Vec<Point<Pixels>> = (0..=segments)
        .map(|i| at_angle(center, radius, from + length * i as f32 / segments as f32))
        .collect();
    let mut builder = PathBuilder::stroke(thickness);
    builder.add_polygon(&points, false);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
    for cap in [points[0], points[points.len() - 1]] {
        let mut builder = PathBuilder::fill();
        let circle: Vec<Point<Pixels>> = (0..16_u8)
            .map(|i| at_angle(cap, thickness / 2.0, TAU * f32::from(i) / 16.0))
            .collect();
        builder.add_polygon(&circle, true);
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}

/// A point on a circle; angles grow counterclockwise on screen, as in Qt.
fn at_angle(center: Point<Pixels>, radius: Pixels, angle: f32) -> Point<Pixels> {
    point(
        center.x + radius * angle.cos(),
        center.y - radius * angle.sin(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_from_the_full_circle_to_the_minimum_then_breathes() {
        let mut radial = InfiniteRadial::new(&style::widgets::DEFAULT_INFINITE_RADIAL_ANIMATION);
        radial.start();
        let started = radial.work_started.expect("started");
        let zero = started - Duration::from_millis(1000);
        let first = radial.state(zero).expect("running");
        assert_eq!((first.shown, first.arc_length), (0.0, FULL));
        let at_work = radial.state(started).expect("running");
        assert_eq!(at_work.arc_length, 360);
        let grown = radial
            .state(started + Duration::from_millis(1500))
            .expect("running");
        assert_eq!(grown.arc_length, 4320, "arc_max of the circle");
        radial.stop();
        assert!(radial.state(started).is_none());
    }
}
