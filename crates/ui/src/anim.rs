//! Time-based animations in the shape of lib_ui's `Animations::Simple`: a
//! value moves from one number to another over a duration, and reading it
//! after the end gives the final value. Elements that read a running
//! animation request the next frame.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::Window;
use style::Color;

/// lib_ui's easing curves (`ui/effects/animation_value.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseOutCirc,
    EaseInCirc,
    EaseOutQuint,
}

impl Easing {
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::EaseOutCirc => (1.0 - (t - 1.0) * (t - 1.0)).max(0.0).sqrt(),
            Self::EaseInCirc => 1.0 - (1.0 - t * t).max(0.0).sqrt(),
            Self::EaseOutQuint => 1.0 - (1.0 - t).powi(5),
        }
    }
}

/// "Interface animations" off in Power Usage (`anim::Disabled`).
static ARE_DISABLED: AtomicBool = AtomicBool::new(false);

/// Animations from now on jump to their end.
pub fn disable() {
    ARE_DISABLED.store(true, Ordering::Relaxed);
}

pub fn enable() {
    ARE_DISABLED.store(false, Ordering::Relaxed);
}

#[must_use]
pub fn are_disabled() -> bool {
    ARE_DISABLED.load(Ordering::Relaxed)
}

/// An animation that may be running.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Simple(Option<Running>);

#[derive(Debug, Clone, Copy, PartialEq)]
struct Running {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    easing: Easing,
}

impl Simple {
    /// Starts moving from `from` to `to` over `duration_ms` milliseconds, as
    /// the style durations are written.
    /// With animations off it is finished at once.
    pub fn start(&mut self, from: f32, to: f32, duration_ms: i32, easing: Easing) {
        if are_disabled() {
            self.0 = None;
            return;
        }
        let duration = Duration::from_millis(u64::try_from(duration_ms).unwrap_or(0));
        self.0 = Some(Running {
            from,
            to,
            started: Instant::now(),
            duration,
            easing,
        });
    }

    pub fn stop(&mut self) {
        self.0 = None;
    }

    #[must_use]
    pub fn animating(&self) -> bool {
        self.0
            .is_some_and(|running| running.started.elapsed() < running.duration)
    }

    /// The current value, or `finished` when the animation is not running.
    #[must_use]
    pub fn value(&self, finished: f32) -> f32 {
        let Some(running) = self.0 else {
            return finished;
        };
        let elapsed = running.started.elapsed();
        if elapsed >= running.duration {
            return finished;
        }
        let t = elapsed.as_secs_f32() / running.duration.as_secs_f32();
        running.from + (running.to - running.from) * running.easing.apply(t)
    }

    /// Like [`Simple::value`], and asks for the next frame while running.
    pub fn value_in(&self, window: &Window, finished: f32) -> f32 {
        if self.animating() {
            window.request_animation_frame();
        }
        self.value(finished)
    }
}

/// `anim::interpolate`: rounds like lib_ui.
#[must_use]
pub fn interpolate(from: f32, to: f32, t: f32) -> f32 {
    (from + (to - from) * t).round()
}

/// `anim::color`: mixes in RGB, as lib_ui does.
#[must_use]
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let channel = |a: u8, b: u8| {
        let value = f32::from(a) + (f32::from(b) - f32::from(a)) * t.clamp(0.0, 1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 0..=255"
        )]
        let value = value.round().clamp(0.0, 255.0) as u8;
        value
    };
    Color::rgba(
        channel(a.r, b.r),
        channel(a.g, b.g),
        channel(a.b, b.b),
        channel(a.a, b.a),
    )
}

/// `DefaultShakeCallback`: the sideways shift of a shake at `progress`
/// (0 to 1), as a multiple of `shakeShift`.
#[must_use]
pub fn shake(progress: f32) -> f32 {
    const SHIFT_PROGRESS: f32 = 6.0;
    const SEGMENTS: f32 = 5.0;
    let full = progress * SHIFT_PROGRESS;
    let segment = full.floor().clamp(0.0, SEGMENTS);
    let part = full - segment;
    #[expect(clippy::cast_possible_truncation, reason = "segment is 0 to 5")]
    let (from, to) = match segment as i32 {
        0 => (0.0, 1.0),
        1 | 3 => (1.0, -1.0),
        2 | 4 => (-1.0, 1.0),
        _ => (1.0, 0.0),
    };
    from * (1.0 - part) + to * part
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stopped_animation_reads_its_final_value() {
        let mut animation = Simple::default();
        assert!((animation.value(1.0) - 1.0).abs() < f32::EPSILON);
        animation.start(0.0, 1.0, 0, Easing::Linear);
        assert!(!animation.animating());
        assert!((animation.value(0.5) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn easings_run_from_zero_to_one() {
        for easing in [
            Easing::Linear,
            Easing::EaseOutCirc,
            Easing::EaseInCirc,
            Easing::EaseOutQuint,
        ] {
            assert!(easing.apply(0.0).abs() < 1e-6, "{easing:?}");
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-6, "{easing:?}");
        }
    }

    #[test]
    fn mixes_colors_in_rgb() {
        let black = Color::rgba(0, 0, 0, 255);
        let white = Color::rgba(255, 255, 255, 255);
        assert_eq!(mix(black, white, 0.5), Color::rgba(128, 128, 128, 255));
        assert_eq!(mix(black, white, 2.0), white);
    }

    #[test]
    fn shake_swings_both_ways_and_settles() {
        assert!(shake(0.0).abs() < f32::EPSILON);
        assert!((shake(1.0 / 6.0) - 1.0).abs() < 1e-5);
        assert!((shake(2.0 / 6.0) + 1.0).abs() < 1e-5);
        assert!(shake(1.0).abs() < 1e-5);
    }
}
