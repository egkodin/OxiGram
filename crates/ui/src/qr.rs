//! The intro's QR code (`PrepareQrWidget` in tdesktop's `intro_qr.cpp`): black
//! modules on a white rounded square, a new code fading in over the old
//! one, a spinner while there is none, and a round mark in the center.

use std::sync::Arc;

use gpui::{
    App, Bounds, ElementId, FontWeight, IntoElement, Pixels, RenderOnce, SharedString, Styled,
    TextAlign, TextRun, Window, canvas, fill, point, px, size,
};
use style::Color;
use style::basic::{FADE_WRAP_DURATION, RADIAL_LINE};
use style::intro::{
    INTRO_QR_BACKGROUND_RADIUS, INTRO_QR_BACKGROUND_SKIP, INTRO_QR_CENTER_SIZE, INTRO_QR_MAX_SIZE,
    INTRO_QR_PIXEL,
};
use style::widgets::DEFAULT_INFINITE_RADIAL_ANIMATION;

use crate::anim::{Easing, Simple};
use crate::font::FONT_FAMILY;
use crate::radial::{InfiniteRadial, paint_arc};
use crate::theme::{Theme, hsla};

/// Modules of a typical login link's code, used to size the empty square.
const USUAL_SIZE: i32 = 41;

/// The code could not be encoded, for example because the text is too long.
#[derive(Debug, thiserror::Error)]
#[error("cannot encode a QR code")]
pub struct QrError(#[source] qrcode::types::QrError);

/// An encoded code: a square of dark and light modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrData {
    size: usize,
    dark: Vec<bool>,
}

impl QrData {
    /// Encodes `text` with the quartile redundancy tdesktop uses.
    ///
    /// # Errors
    ///
    /// Fails when `text` does not fit in a QR code.
    pub fn encode(text: &str) -> Result<Self, QrError> {
        let code = qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::Q)
            .map_err(QrError)?;
        let dark = code
            .to_colors()
            .into_iter()
            .map(|color| color == qrcode::Color::Dark)
            .collect();
        Ok(Self {
            size: code.width(),
            dark,
        })
    }

    fn is_dark(&self, x: usize, y: usize) -> bool {
        self.dark.get(y * self.size + x).copied().unwrap_or(false)
    }
}

/// The code to show; `None` while waiting for the first one.
#[derive(IntoElement)]
pub struct QrCode {
    id: ElementId,
    data: Option<Arc<QrData>>,
}

impl QrCode {
    pub fn new(id: impl Into<ElementId>, data: Option<Arc<QrData>>) -> Self {
        Self {
            id: id.into(),
            data,
        }
    }

    /// The widget's side: the largest code plus the white margin.
    #[must_use]
    pub fn side() -> Pixels {
        px_of(INTRO_QR_MAX_SIZE.0 + 2 * INTRO_QR_BACKGROUND_SKIP.0)
    }
}

impl std::fmt::Debug for QrCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QrCode")
            .field("id", &self.id)
            .field("has_data", &self.data.is_some())
            .finish()
    }
}

#[derive(Clone)]
struct QrState {
    current: Option<Arc<QrData>>,
    previous: Option<Arc<QrData>>,
    shown: Simple,
    waiting: InfiniteRadial,
}

impl RenderOnce for QrCode {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, _| {
            let mut waiting = InfiniteRadial::new(&DEFAULT_INFINITE_RADIAL_ANIMATION);
            waiting.start();
            QrState {
                current: None,
                previous: None,
                shown: Simple::default(),
                waiting,
            }
        });
        let data = self.data;
        state.update(cx, |state, _| update(state, data));
        let side = Self::side();
        canvas(
            |_, _, _| {},
            move |bounds, (), window, cx| {
                let state = state.read(cx).clone();
                paint(&state, bounds, window, cx);
            },
        )
        .w(side)
        .h(side)
    }
}

fn update(state: &mut QrState, data: Option<Arc<QrData>>) {
    let same = match (&state.current, &data) {
        (Some(current), Some(data)) => Arc::ptr_eq(current, data),
        (None, None) => true,
        _ => false,
    };
    if same {
        return;
    }
    if data.is_some() {
        state.previous = state.current.take();
        state.current = data;
        state.waiting.stop();
        state
            .shown
            .start(0.0, 1.0, FADE_WRAP_DURATION, Easing::Linear);
    } else {
        state.previous = None;
        state.current = None;
        state.shown.stop();
        state.waiting.start();
    }
}

fn paint(state: &QrState, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let shown = if state.current.is_some() {
        state.shown.value_in(window, 1.0)
    } else {
        0.0
    };
    let modules = state.current.as_ref().map_or(USUAL_SIZE, |data| {
        i32::try_from(data.size).unwrap_or(USUAL_SIZE)
    });
    let pixel = module_pixel(modules, state.current.is_some());
    let qr_side = px_of(modules * pixel);
    let qr = Bounds::new(
        point(
            bounds.left() + (bounds.size.width - qr_side) / 2.0,
            bounds.top() + (bounds.size.height - qr_side) / 2.0,
        ),
        size(qr_side, qr_side),
    );
    let skip = px_of(INTRO_QR_BACKGROUND_SKIP.0);
    window.paint_quad(
        fill(qr.dilate(skip), gpui::white()).corner_radii(px_of(INTRO_QR_BACKGROUND_RADIUS.0)),
    );
    if let Some(current) = &state.current {
        if shown < 1.0
            && let Some(previous) = &state.previous
        {
            paint_modules(previous, qr, 1.0, window);
        }
        paint_modules(current, qr, shown, window);
    }

    let center_side = px_of(INTRO_QR_CENTER_SIZE.0);
    let center = Bounds::new(
        point(
            bounds.left() + (bounds.size.width - center_side) / 2.0,
            bounds.top() + (bounds.size.height - center_side) / 2.0,
        ),
        size(center_side, center_side),
    );
    let active = hsla(QR_ACTIVE);
    paint_mark(center, active, window, cx);
    if let Some(radial) = state.waiting.state_in(window) {
        let line = px_of(RADIAL_LINE.0);
        let mut color = active;
        color.a *= radial.shown * (1.0 - shown);
        paint_arc(window, center.dilate(line), radial, line, color);
    }
}

/// tdesktop's `QrActiveColor`: the default theme's `windowBgActive`, kept
/// in every theme so the code stays readable.
const QR_ACTIVE: Color = Color::rgba(0x40, 0xa7, 0xe3, 0xff);

/// The module size `TelegramQr` picks; the empty square uses the usual size.
fn module_pixel(modules: i32, has_code: bool) -> i32 {
    let max = INTRO_QR_MAX_SIZE.0;
    let pixel = INTRO_QR_PIXEL.0;
    if has_code {
        if modules * pixel > max {
            (max / modules.max(1)).max(1)
        } else {
            pixel
        }
    } else {
        (max / USUAL_SIZE).clamp(1, pixel)
    }
}

// shortcut: square modules; tdesktop's lib_qr rounds module corners, match it
// when its source is available for comparison.
fn paint_modules(data: &QrData, qr: Bounds<Pixels>, opacity: f32, window: &mut Window) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "QR codes have at most 177 modules"
    )]
    let module = qr.size.width / data.size as f32;
    let mut color = gpui::black();
    color.a *= opacity;
    for y in 0..data.size {
        // One quad per horizontal run of dark modules.
        let mut x = 0;
        while x < data.size {
            if !data.is_dark(x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < data.size && data.is_dark(x, y) {
                x += 1;
            }
            #[expect(
                clippy::cast_precision_loss,
                reason = "QR codes have at most 177 modules"
            )]
            let run = Bounds::new(
                point(
                    qr.left() + module * start as f32,
                    qr.top() + module * y as f32,
                ),
                size(module * (x - start) as f32, module),
            );
            window.paint_quad(fill(run, color));
        }
    }
}

/// rgram's mark: a lowercase "r" on the active circle, in place of
/// Telegram's plane.
fn paint_mark(center: Bounds<Pixels>, color: gpui::Hsla, window: &mut Window, cx: &mut App) {
    window.paint_quad(fill(center, color).corner_radii(center.size.width / 2.0));
    let text: SharedString = "r".into();
    let font_size = center.size.height * 0.62;
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight: FontWeight::BOLD,
            ..gpui::font(FONT_FAMILY)
        },
        color: hsla(cx.global::<Theme>().palette.active_button_fg),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text, font_size, &[run], None);
    let line_height = font_size * 1.2;
    let origin = point(
        center.left() + (center.size.width - line.width) / 2.0,
        center.top() + (center.size.height - line_height) / 2.0 - center.size.height * 0.03,
    );
    if let Err(error) = line.paint(origin, line_height, TextAlign::Left, None, window, cx) {
        tracing::warn!(%error, "cannot paint the QR mark");
    }
}

fn px_of(value: i32) -> Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    px(value as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_login_link() {
        let data = QrData::encode("tg://login?token=AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA")
            .expect("a login link fits");
        assert_eq!(data.dark.len(), data.size * data.size);
        // The top-left finder pattern starts with a dark row of seven.
        assert!((0..7).all(|x| data.is_dark(x, 0)));
        assert!(!data.is_dark(7, 0));
    }

    #[test]
    fn picks_the_module_size_like_tdesktop() {
        assert_eq!(module_pixel(41, true), 4);
        assert_eq!(module_pixel(3, true), 50);
        assert_eq!(module_pixel(0, false), 4);
    }
}
