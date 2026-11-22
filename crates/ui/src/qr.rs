//! The intro's QR code (`PrepareQrWidget` in tdesktop's `intro_qr.cpp`): black
//! modules on a white rounded square, a new code fading in over the old
//! one, a spinner while there is none, and a round mark in the center.

use std::sync::Arc;

use gpui::{
    App, Bounds, ElementId, IntoElement, Pixels, RenderOnce, Styled, Window, canvas, fill, point,
    px, size,
};
use style::Color;
use style::basic::{FADE_WRAP_DURATION, RADIAL_LINE};
use style::intro::{
    INTRO_QR_BACKGROUND_RADIUS, INTRO_QR_BACKGROUND_SKIP, INTRO_QR_CENTER_SIZE, INTRO_QR_MAX_SIZE,
    INTRO_QR_PIXEL,
};
use style::widgets::DEFAULT_INFINITE_RADIAL_ANIMATION;

use crate::anim::{Easing, Simple};
use crate::brand::paint_brand_logo;
use crate::radial::{InfiniteRadial, paint_arc};
use crate::theme::hsla;

const QUIET_ZONE_MODULES: usize = 4;
const FOOTER_HEIGHT: f32 = 30.0;

/// The code could not be encoded, for example because the text is too long.
#[derive(Debug, thiserror::Error)]
#[error("cannot encode a QR code")]
pub struct QrError(#[source] qrcode::types::QrError);

/// An encoded code: a square of dark and light modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrData {
    size: usize,
    dark: Vec<bool>,
    badge_modules: Option<usize>,
}

impl QrData {
    /// Encodes `text` with high error correction for the center badge.
    ///
    /// # Errors
    ///
    /// Fails when `text` does not fit in a QR code.
    pub fn encode(text: &str) -> Result<Self, QrError> {
        let code = qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::H)
            .or_else(|_| qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::Q))
            .map_err(QrError)?;
        let badge_modules = (code.error_correction_level() == qrcode::EcLevel::H)
            .then(|| badge_modules(&code))
            .flatten();
        let dark = code
            .to_colors()
            .into_iter()
            .map(|color| color == qrcode::Color::Dark)
            .collect();
        Ok(Self {
            size: code.width(),
            dark,
            badge_modules,
        })
    }

    fn is_dark(&self, x: usize, y: usize) -> bool {
        self.dark.get(y * self.size + x).copied().unwrap_or(false)
    }
}

/// Keep the white badge within a fifth of the symbol and off functional modules.
fn badge_modules(code: &qrcode::QrCode) -> Option<usize> {
    let width = code.width();
    let radius = (width / 5).min(9).saturating_sub(1) / 2;
    let center = width / 2;
    let span = center - radius..=center + radius;
    span.clone()
        .all(|y| span.clone().all(|x| !code.is_functional(x, y)))
        .then_some(2 * radius + 1)
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
                paint(state.read(cx), bounds, window);
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

fn paint(state: &QrState, bounds: Bounds<Pixels>, window: &mut Window) {
    window
        .paint_quad(fill(bounds, gpui::white()).corner_radii(px_of(INTRO_QR_BACKGROUND_RADIUS.0)));
    if let Some(current) = &state.current {
        let shown = state.shown.value_in(window, 1.0);
        let (qr, logo) = layout(bounds, current);
        if shown < 1.0
            && let Some(previous) = &state.previous
        {
            paint_modules(previous, qr, 1.0, window);
        }
        paint_modules(current, qr, shown, window);
        paint_logo(logo, window);
        return;
    }

    let center = centered_square(bounds, px_of(INTRO_QR_CENTER_SIZE.0));
    paint_logo(center, window);
    if let Some(radial) = state.waiting.state_in(window) {
        let line = px_of(RADIAL_LINE.0);
        let mut color = hsla(QR_ACTIVE);
        color.a *= radial.shown;
        paint_arc(window, center.dilate(line), radial, line, color);
    }
}

fn centered_square(bounds: Bounds<Pixels>, side: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        point(
            bounds.left() + (bounds.size.width - side) / 2.0,
            bounds.top() + (bounds.size.height - side) / 2.0,
        ),
        size(side, side),
    )
}

fn layout(bounds: Bounds<Pixels>, data: &QrData) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let has_footer = data.badge_modules.is_none();
    let footer_height = if has_footer {
        px(FOOTER_HEIGHT)
    } else {
        px(0.0)
    };
    let mut code_area = bounds;
    code_area.size.height -= footer_height;
    let module = module_pixel(data.size, code_area.size.width.min(code_area.size.height));
    #[expect(
        clippy::cast_precision_loss,
        reason = "QR codes have at most 177 modules"
    )]
    let qr = centered_square(code_area, module * data.size as f32);
    let logo = data.badge_modules.map_or_else(
        || {
            let footer = Bounds::new(
                point(bounds.left(), code_area.bottom()),
                size(bounds.size.width, footer_height),
            );
            centered_square(footer, footer_height)
        },
        |modules| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "the badge spans at most nine modules"
            )]
            let modules = modules as f32;
            centered_square(qr, module * modules)
        },
    );
    (qr, logo)
}

const QR_ACTIVE: Color = Color::rgba(0xfc, 0x65, 0x12, 0xff);

fn module_pixel(modules: usize, available: Pixels) -> Pixels {
    #[expect(
        clippy::cast_precision_loss,
        reason = "QR codes have at most 177 modules"
    )]
    let limit = f32::from(available) / (modules + 2 * QUIET_ZONE_MODULES) as f32;
    let pixel = if limit >= 1.0 { limit.floor() } else { limit };
    px(pixel).min(px_of(INTRO_QR_PIXEL.0))
}

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

fn paint_logo(badge: Bounds<Pixels>, window: &mut Window) {
    window.paint_quad(fill(badge, gpui::white()));
    paint_brand_logo(badge.dilate(px(-2.0)), window);
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
    fn reserves_four_white_modules_even_for_dense_codes() {
        for modules in [21, 41, 177] {
            let module = f32::from(module_pixel(modules, px(174.0)));
            #[expect(clippy::cast_precision_loss, reason = "the test uses small QR sizes")]
            let with_margin = (modules + 8) as f32 * module;
            assert!(with_margin <= 174.0 + f32::EPSILON);
        }
    }

    #[test]
    fn puts_the_badge_in_a_footer_when_an_alignment_pattern_is_in_the_center() {
        use qrcode::{EcLevel, Version};

        let short =
            qrcode::QrCode::with_version("https://t.me/oxigram", Version::Normal(3), EcLevel::H)
                .expect("the link fits");
        let dense =
            qrcode::QrCode::with_version("https://t.me/oxigram", Version::Normal(7), EcLevel::H)
                .expect("the link fits");
        assert_eq!(badge_modules(&short), Some(5));
        assert_eq!(badge_modules(&dense), None);
    }
}
