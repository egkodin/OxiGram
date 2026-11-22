use std::sync::{Arc, OnceLock};

use gpui::{Bounds, Corners, IntoElement, Pixels, RenderImage, Styled, Window, canvas};
use image::{Frame, ImageFormat};

// Exported from resources/OxiGram.icns for cross-platform rendering.
const LOGO_PNG: &[u8] = include_bytes!("../../../resources/oxigram-qr.png");

/// The client icon at the requested size.
pub fn brand_logo(side: Pixels) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |bounds, (), window, _| paint_brand_logo(bounds, window),
    )
    .w(side)
    .h(side)
}

pub(crate) fn paint_brand_logo(bounds: Bounds<Pixels>, window: &mut Window) {
    static LOGO: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();
    let image = LOGO.get_or_init(|| {
        let mut pixels = image::load_from_memory_with_format(LOGO_PNG, ImageFormat::Png)
            .inspect_err(|error| tracing::warn!(%error, "cannot decode the client logo"))
            .ok()?
            .into_rgba8();
        for pixel in pixels.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        Some(Arc::new(RenderImage::new([Frame::new(pixels)])))
    });
    if let Some(image) = image
        && let Err(error) = window.paint_image(
            bounds,
            bounds,
            Corners::default(),
            Arc::clone(image),
            0,
            false,
        )
    {
        tracing::warn!(%error, "cannot paint the client logo");
    }
}
