//! Photos cut to a circle at the exact device size, as tdesktop prepares
//! userpics (`Image::pixCircled`): gpui scales images on the GPU without
//! mipmaps, which aliases a 160 px photo drawn at 46, and clips children
//! only to rectangles.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{App, Global, RenderImage};
use image::{Frame, RgbaImage, imageops::FilterType};

/// Prepared images kept before the cache starts over.
// shortcut: the whole cache drops when full and gpui keeps the dropped textures; an LRU that calls `drop_image` when profiles show many more userpics (M6).
const MAX_IMAGES: usize = 512;

#[derive(Default)]
struct RoundImages {
    images: HashMap<(PathBuf, u32), Option<Arc<RenderImage>>>,
}

impl Global for RoundImages {}

/// The image at `path` filled into a `side` px square and cut to a circle,
/// `None` if it cannot be read.
// shortcut: decodes on the main thread; userpics are small (160 px JPEGs), move it to the background executor if a profile shows large ones (M6).
pub(crate) fn round_file(path: &Path, side: u32, cx: &mut App) -> Option<Arc<RenderImage>> {
    let key = (path.to_path_buf(), side);
    if let Some(image) = cx
        .try_global::<RoundImages>()
        .and_then(|c| c.images.get(&key))
    {
        return image.clone();
    }
    let image = load(path, side);
    let cache = cx.default_global::<RoundImages>();
    if cache.images.len() >= MAX_IMAGES {
        cache.images.clear();
    }
    cache.images.insert(key, image.clone());
    image
}

fn load(path: &Path, side: u32) -> Option<Arc<RenderImage>> {
    let decoded = match image::open(path) {
        Ok(decoded) => decoded,
        Err(error) => {
            tracing::warn!(%error, "cannot decode a userpic");
            return None;
        }
    };
    let filled = decoded
        .resize_to_fill(side, side, FilterType::Lanczos3)
        .to_rgba8();
    Some(Arc::new(RenderImage::new([Frame::new(circled(&filled))])))
}

/// BGRA with straight alpha, as gpui's sprites expect, and an antialiased
/// circle: each pixel keeps the share of it inside the circle.
fn circled(square: &RgbaImage) -> RgbaImage {
    #[expect(clippy::cast_precision_loss, reason = "userpics are small")]
    let radius = square.width().min(square.height()) as f32 / 2.0;
    let mut out = RgbaImage::new(square.width(), square.height());
    for (x, y, pixel) in square.enumerate_pixels() {
        #[expect(clippy::cast_precision_loss, reason = "userpics are small")]
        let (dx, dy) = (x as f32 + 0.5 - radius, y as f32 + 0.5 - radius);
        let coverage = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
        let [r, g, b, a] = pixel.0;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "coverage is within 0..=1"
        )]
        let alpha = (f32::from(a) * coverage).round() as u8;
        out.put_pixel(x, y, image::Rgba([b, g, r, alpha]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_middle_and_clears_the_corners() {
        let square = RgbaImage::from_pixel(40, 40, image::Rgba([10, 20, 30, 255]));
        let round = circled(&square);
        assert_eq!(round.get_pixel(20, 20).0, [30, 20, 10, 255]);
        assert_eq!(round.get_pixel(0, 0).0[3], 0);
        // On the edge the pixel is partly covered.
        let edge = round.get_pixel(0, 20).0[3];
        assert!(edge > 0 && edge < 255, "edge alpha {edge}");
    }
}
