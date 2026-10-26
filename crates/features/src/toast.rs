//! lib_ui's `Toast`: a short note in the middle of the window that fades
//! in, stays a while and fades out.

use std::time::{Duration, Instant};

use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div};
use style::{ColorId, ColorRef};
use ui::{color, font, font_height};

use crate::px_of;

/// How long a toast stays before it fades out (`Toast::kDefaultDuration`).
pub const DEFAULT_DURATION: Duration = Duration::from_millis(1500);

/// A shown toast.
// shortcut: one toast at a time, a new one replaces it; tdesktop's manager stacks them.
#[derive(Debug)]
pub(crate) struct Toast {
    text: SharedString,
    style: &'static style::widgets::Toast,
    duration: Duration,
    shown_at: Instant,
}

impl Toast {
    pub(crate) fn new(
        text: SharedString,
        style: &'static style::widgets::Toast,
        duration: Duration,
    ) -> Self {
        Self {
            text,
            style,
            duration,
            shown_at: Instant::now(),
        }
    }

    /// When it starts to fade out.
    pub(crate) fn duration(&self) -> Duration {
        self.duration
    }

    /// The toast while it shows, `None` once it has faded out.
    pub(crate) fn element(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        let opacity = opacity(self.shown_at.elapsed(), self.style, self.duration)?;
        if opacity < 1.0 {
            window.request_animation_frame();
        }
        let st = self.style;
        let text_font = st.style.font;
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px_of(st.margin.left))
                .child(
                    div()
                        .opacity(opacity)
                        .min_w(px_of(st.min_width))
                        .max_w(px_of(st.max_width))
                        .pt(px_of(st.padding.top))
                        .pr(px_of(st.padding.right))
                        .pb(px_of(st.padding.bottom))
                        .pl(px_of(st.padding.left))
                        .rounded(px_of(st.radius))
                        .bg(color(cx, ColorRef::Palette(ColorId::ToastBg)))
                        .font(font(text_font))
                        .text_size(px_of(text_font.size))
                        .line_height(font_height(text_font))
                        .text_color(color(cx, ColorRef::Palette(ColorId::ToastFg)))
                        .child(self.text.clone()),
                )
                .into_any_element(),
        )
    }
}

/// The toast's opacity `elapsed` after it showed: fading in, shown, fading
/// out after `duration`, then `None`.
fn opacity(elapsed: Duration, st: &style::widgets::Toast, duration: Duration) -> Option<f32> {
    let millis = |ms: i32| Duration::from_millis(u64::try_from(ms).unwrap_or(0));
    let (fade_in, fade_out) = (millis(st.duration_fade_in), millis(st.duration_fade_out));
    if elapsed < fade_in {
        return Some(elapsed.as_secs_f32() / fade_in.as_secs_f32());
    }
    if elapsed < duration {
        return Some(1.0);
    }
    let fading = elapsed - duration;
    (fading < fade_out).then(|| 1.0 - fading.as_secs_f32() / fade_out.as_secs_f32())
}

#[cfg(test)]
mod tests {
    use super::*;
    use style::widgets::DEFAULT_TOAST;

    #[test]
    fn fades_in_stays_and_fades_out() {
        let at = |ms| opacity(Duration::from_millis(ms), &DEFAULT_TOAST, DEFAULT_DURATION);
        assert_eq!(at(0), Some(0.0));
        assert_eq!(at(100), Some(0.5));
        assert_eq!(at(1000), Some(1.0));
        assert_eq!(at(2000), Some(0.5));
        assert_eq!(at(2500), None);
    }
}
