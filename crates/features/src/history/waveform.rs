//! A voice message's waveform (`PaintWaveform`): its values resampled to the
//! bars that fit, the played part in the active color, and a click on it
//! seeks while it plays.

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Bounds, InteractiveElement as _, IntoElement as _, MouseButton,
    ParentElement as _, Pixels, Styled as _, Window, canvas, div, fill, point, px, size,
};
use style::chat::{MSG_WAVEFORM_BAR, MSG_WAVEFORM_MAX, MSG_WAVEFORM_MIN, MSG_WAVEFORM_SKIP};
use style::{ColorId, ColorRef};
use ui::color;

use crate::px_of;

/// Values a waveform has (`Player::kWaveformSamplesCount`).
const SAMPLES: usize = 100;

pub(super) type OnSeek = Rc<dyn Fn(f32, &mut Window, &mut App)>;

pub(super) struct Waveform {
    /// 0-31 each; empty draws flat bars.
    pub values: Vec<u8>,
    /// The share played, 0 to 1.
    pub progress: f32,
    /// Set while this voice plays.
    pub on_seek: Option<OnSeek>,
}

impl Waveform {
    pub(super) fn still(values: Vec<u8>) -> Self {
        Self {
            values,
            progress: 0.0,
            on_seek: None,
        }
    }
}

/// Each bar's value above the minimum height, at most `max_delta`: the
/// values spread over `count` bars, each the peak of its span.
fn bars(values: &[u8], count: usize, max_delta: i32) -> Vec<i32> {
    let size = if values.is_empty() {
        SAMPLES
    } else {
        values.len()
    };
    let count = count.min(size);
    let norm = i32::from(values.iter().copied().max().unwrap_or(0)) + 1;
    let half = count.div_ceil(2);
    let mut result = Vec::with_capacity(count);
    let (mut sum, mut peak) = (0, 0);
    for index in 0..size {
        let value = i32::from(values.get(index).copied().unwrap_or(0));
        if sum + count < size {
            peak = peak.max(value);
            sum += count;
            continue;
        }
        sum = sum + count - size;
        if sum < half {
            peak = peak.max(value);
        }
        result.push((peak * max_delta + norm / 2) / norm);
        peak = if sum < half { 0 } else { value };
    }
    result
}

fn palette(is_out: bool, is_selected: bool) -> (ColorId, ColorId) {
    match (is_out, is_selected) {
        (false, false) => (ColorId::MsgWaveformInActive, ColorId::MsgWaveformInInactive),
        (false, true) => (
            ColorId::MsgWaveformInActiveSelected,
            ColorId::MsgWaveformInInactiveSelected,
        ),
        (true, false) => (
            ColorId::MsgWaveformOutActive,
            ColorId::MsgWaveformOutInactive,
        ),
        (true, true) => (
            ColorId::MsgWaveformOutActiveSelected,
            ColorId::MsgWaveformOutInactiveSelected,
        ),
    }
}

/// The bars across the row's width.
// shortcut: no hover preview or drag to seek; a click seeks.
pub(super) fn element(waveform: Waveform, is_out: bool, is_selected: bool, cx: &App) -> AnyElement {
    let (active, inactive) = palette(is_out, is_selected);
    let (active, inactive) = (
        color(cx, ColorRef::Palette(active)),
        color(cx, ColorRef::Palette(inactive)),
    );
    let (bar, skip) = (px_of(MSG_WAVEFORM_BAR), px_of(MSG_WAVEFORM_SKIP));
    let (min, max) = (px_of(MSG_WAVEFORM_MIN), px_of(MSG_WAVEFORM_MAX));
    let max_delta = MSG_WAVEFORM_MAX.0 - MSG_WAVEFORM_MIN.0;
    let Waveform {
        values,
        progress,
        on_seek,
    } = waveform;
    let placed: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
    let paint = canvas(
        {
            let placed = Rc::clone(&placed);
            move |bounds, _, _| placed.set(Some(bounds))
        },
        move |bounds, (), window, _| {
            // tdesktop gives the bars the room plus one skip.
            let room = bounds.size.width + skip;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a bar count"
            )]
            let count = (f32::from(room) / f32::from(bar + skip)).floor().max(0.0) as usize;
            let active_width = (room * progress).round();
            let mut left = px(0.0);
            for value in bars(&values, count, max_delta) {
                #[expect(clippy::cast_precision_loss, reason = "a few pixels")]
                let value = px(value as f32);
                let height = min + value;
                // `lineWidth` above, then centered in the tallest bar.
                let top = px(1.0) + (max - value) / 2.0;
                let origin = bounds.origin + point(left, top);
                let played = (active_width - left).clamp(px(0.0), bar);
                if played > px(0.0) {
                    window.paint_quad(fill(Bounds::new(origin, size(played, height)), active));
                }
                if played < bar {
                    let rest =
                        Bounds::new(origin + point(played, px(0.0)), size(bar - played, height));
                    window.paint_quad(fill(rest, inactive));
                }
                left += bar + skip;
            }
        },
    )
    .size_full();
    // Room for every value as a bar, as tdesktop widens the bubble to it.
    #[expect(clippy::cast_precision_loss, reason = "a hundred bars")]
    let widest = (bar + skip) * SAMPLES as f32 - skip;
    div()
        .w(widest)
        .max_w_full()
        .h_full()
        .child(paint)
        .when_some(on_seek, |this, on_seek| {
            this.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let Some(bounds) = placed.get() else {
                    return;
                };
                cx.stop_propagation();
                let share = (event.position.x - bounds.origin.x) / bounds.size.width;
                on_seek(share.clamp(0.0, 1.0), window, cx);
            })
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hundred_values_shrink_to_the_bars_that_fit() {
        let values: Vec<u8> = (0..100)
            .map(|index| if index % 2 == 0 { 31 } else { 0 })
            .collect();
        let bars = bars(&values, 50, 14);
        assert_eq!(bars.len(), 50);
        // Each bar takes the peak of its two values.
        assert!(bars.iter().all(|&bar| bar == 14), "{bars:?}");
    }

    #[test]
    fn a_missing_waveform_draws_flat_bars() {
        assert_eq!(bars(&[], 10, 14), vec![0; 10]);
    }

    #[test]
    fn wide_rows_keep_one_bar_a_value() {
        let bars = bars(&[0, 31, 15], 80, 14);
        assert_eq!(bars, [0, 14, 7]);
    }
}
