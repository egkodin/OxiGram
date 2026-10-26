//! lib_ui's `RippleAnimation`: a circle grows from the press point over the
//! button's shape and fades out after release.

use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, Entity, IntoElement, PathBuilder, Pixels, Point, Styled, Window, canvas,
    point, px,
};
use style::Color;
use style::widgets::RippleAnimation;

use crate::anim::Easing;
use crate::theme::{Theme, hsla};

/// Vertices per circle and per rounded corner; enough to look round at 3x.
const CIRCLE_SEGMENTS: usize = 64;
const CORNER_SEGMENTS: usize = 8;

/// The shape ripples are clipped to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Shape {
    RoundRect { radius: Pixels },
    Circle,
}

/// Hover and ripples of one button.
#[derive(Debug, Default)]
pub(crate) struct ButtonState {
    pub(crate) hovered: bool,
    ripples: Vec<Ripple>,
    bounds: Option<Bounds<Pixels>>,
}

#[derive(Debug, Clone, Copy)]
struct Ripple {
    origin: Point<Pixels>,
    radius_to: Pixels,
    started: Instant,
    released: Option<Instant>,
}

impl ButtonState {
    /// Starts a ripple at `position` (window coordinates).
    pub(crate) fn press(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.bounds else {
            return;
        };
        let origin = position - bounds.origin;
        let corners = [
            point(px(0.0), px(0.0)),
            point(bounds.size.width, px(0.0)),
            point(bounds.size.width, bounds.size.height),
            point(px(0.0), bounds.size.height),
        ];
        let radius_to = corners
            .iter()
            .map(|corner| distance(origin, *corner))
            .fold(px(0.0), Pixels::max);
        self.ripples.push(Ripple {
            origin,
            radius_to,
            started: Instant::now(),
            released: None,
        });
        cx.notify();
    }

    pub(crate) fn release(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let mut changed = false;
        for ripple in self
            .ripples
            .iter_mut()
            .filter(|ripple| ripple.released.is_none())
        {
            ripple.released = Some(now);
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }
}

/// A layer that fills its parent and paints `state`'s ripples in `style`'s color.
pub(crate) fn layer(
    ripples: &Entity<ButtonState>,
    style: RippleAnimation,
    shape: Shape,
) -> impl IntoElement {
    let ripples = ripples.clone();
    canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            let color = style.color.resolve(&cx.global::<Theme>().palette);
            ripples.update(cx, |state, _| {
                state.bounds = Some(bounds);
                paint(state, bounds, style, shape, color, window);
            });
        },
    )
    .absolute()
    .size_full()
}

fn paint(
    state: &mut ButtonState,
    bounds: Bounds<Pixels>,
    style: RippleAnimation,
    shape: Shape,
    color: Color,
    window: &mut Window,
) {
    let show = duration(style.show_duration);
    let hide = duration(style.hide_duration);
    state.ripples.retain(|ripple| {
        ripple
            .released
            .is_none_or(|released| released.elapsed() < hide)
    });
    if state.ripples.is_empty() {
        return;
    }
    window.request_animation_frame();
    let clip = shape_polygon(bounds, shape);
    for ripple in &state.ripples {
        let shown = progress(ripple.started.elapsed(), show);
        let radius = ripple.radius_to * Easing::EaseOutQuint.apply(shown);
        let opacity = ripple
            .released
            .map_or(1.0, |released| 1.0 - progress(released.elapsed(), hide));
        let circle = circle_polygon(bounds.origin + ripple.origin, radius);
        let polygon = clip_convex(&circle, &clip);
        if polygon.len() < 3 {
            continue;
        }
        let mut builder = PathBuilder::fill();
        builder.add_polygon(&polygon, true);
        if let Ok(path) = builder.build() {
            let mut fill = hsla(color);
            fill.a *= opacity;
            window.paint_path(path, fill);
        }
    }
}

fn duration(ms: i32) -> Duration {
    Duration::from_millis(u64::try_from(ms).unwrap_or(0))
}

fn progress(elapsed: Duration, total: Duration) -> f32 {
    if total.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f32() / total.as_secs_f32()).min(1.0)
    }
}

fn distance(a: Point<Pixels>, b: Point<Pixels>) -> Pixels {
    let dx = f32::from(a.x - b.x);
    let dy = f32::from(a.y - b.y);
    px(dx.hypot(dy))
}

#[expect(clippy::cast_precision_loss, reason = "segment counts are small")]
fn circle_polygon(center: Point<Pixels>, radius: Pixels) -> Vec<Point<Pixels>> {
    (0..CIRCLE_SEGMENTS)
        .map(|i| {
            let angle = TAU * i as f32 / CIRCLE_SEGMENTS as f32;
            point(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

/// The clip shape as a convex polygon in clockwise order.
#[expect(clippy::cast_precision_loss, reason = "segment counts are small")]
fn shape_polygon(bounds: Bounds<Pixels>, shape: Shape) -> Vec<Point<Pixels>> {
    match shape {
        Shape::Circle => {
            let radius = bounds.size.width.min(bounds.size.height) / 2.0;
            circle_polygon(bounds.center(), radius)
        }
        Shape::RoundRect { radius } => {
            let radius = radius
                .min(bounds.size.width / 2.0)
                .min(bounds.size.height / 2.0);
            // Corner centers clockwise from the top left, each with its arc's start angle.
            let corners = [
                (point(bounds.left() + radius, bounds.top() + radius), 0.5),
                (point(bounds.right() - radius, bounds.top() + radius), 0.75),
                (
                    point(bounds.right() - radius, bounds.bottom() - radius),
                    0.0,
                ),
                (
                    point(bounds.left() + radius, bounds.bottom() - radius),
                    0.25,
                ),
            ];
            corners
                .iter()
                .flat_map(|(center, start)| {
                    (0..=CORNER_SEGMENTS).map(move |i| {
                        let turn = start + 0.25 * i as f32 / CORNER_SEGMENTS as f32;
                        let angle = TAU * turn;
                        point(
                            center.x + radius * angle.cos(),
                            center.y + radius * angle.sin(),
                        )
                    })
                })
                .collect()
        }
    }
}

/// Sutherland–Hodgman: `subject` clipped by the convex polygon `clip`. Both
/// are in the same winding order (y down, angles increasing clockwise).
fn clip_convex(subject: &[Point<Pixels>], clip: &[Point<Pixels>]) -> Vec<Point<Pixels>> {
    let mut output = subject.to_vec();
    for (index, &edge_start) in clip.iter().enumerate() {
        let edge_end = clip[(index + 1) % clip.len()];
        let input = std::mem::take(&mut output);
        let Some(&last) = input.last() else {
            break;
        };
        let inside = |p: Point<Pixels>| cross(edge_start, edge_end, p) >= 0.0;
        let mut previous = last;
        for &current in &input {
            if inside(current) {
                if !inside(previous) {
                    output.push(intersection(previous, current, edge_start, edge_end));
                }
                output.push(current);
            } else if inside(previous) {
                output.push(intersection(previous, current, edge_start, edge_end));
            }
            previous = current;
        }
    }
    output
}

fn cross(a: Point<Pixels>, b: Point<Pixels>, p: Point<Pixels>) -> f32 {
    let (ax, ay) = (f32::from(a.x), f32::from(a.y));
    let (bx, by) = (f32::from(b.x), f32::from(b.y));
    let (px_, py) = (f32::from(p.x), f32::from(p.y));
    (bx - ax) * (py - ay) - (by - ay) * (px_ - ax)
}

fn intersection(
    p1: Point<Pixels>,
    p2: Point<Pixels>,
    a: Point<Pixels>,
    b: Point<Pixels>,
) -> Point<Pixels> {
    let d1 = cross(a, b, p1);
    let d2 = cross(a, b, p2);
    let t = if (d1 - d2).abs() < f32::EPSILON {
        0.0
    } else {
        d1 / (d1 - d2)
    };
    point(p1.x + (p2.x - p1.x) * t, p1.y + (p2.y - p1.y) * t)
}

/// The state of the button keyed by `id`.
pub(crate) fn use_button_state(
    id: impl Into<gpui::ElementId>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ButtonState> {
    window.use_keyed_state(id, cx, |_, _| ButtonState::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn area(polygon: &[Point<Pixels>]) -> f32 {
        let mut sum = 0.0;
        for (i, a) in polygon.iter().enumerate() {
            let b = polygon[(i + 1) % polygon.len()];
            sum += f32::from(a.x) * f32::from(b.y) - f32::from(b.x) * f32::from(a.y);
        }
        sum.abs() / 2.0
    }

    #[test]
    fn a_large_circle_is_clipped_to_the_rounded_rect() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(40.0)));
        let clip = shape_polygon(bounds, Shape::RoundRect { radius: px(6.0) });
        let circle = circle_polygon(point(px(10.0), px(10.0)), px(500.0));
        let clipped = clip_convex(&circle, &clip);
        let expected = area(&clip);
        assert!(
            (area(&clipped) - expected).abs() < 1.0,
            "{} vs {expected}",
            area(&clipped)
        );
        assert!(expected < 4000.0, "the corners are cut: {expected}");
    }

    #[test]
    fn a_small_circle_inside_stays_whole() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(40.0)));
        let clip = shape_polygon(bounds, Shape::RoundRect { radius: px(6.0) });
        let circle = circle_polygon(point(px(50.0), px(20.0)), px(5.0));
        let clipped = clip_convex(&circle, &clip);
        assert!((area(&clipped) - area(&circle)).abs() < 0.01);
    }
}
