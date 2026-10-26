//! Icons: single-color masks from tdesktop's PNG files, tinted when drawn
//! (lib_ui's `style::Icon` and `colorizeImage`).

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, Bounds, Corners, Global, IntoElement, Pixels, RenderImage, RenderOnce, Size, Styled,
    Window, canvas, point, px, size,
};
use image::{Frame, ImageFormat, RgbaImage, imageops};
use style::{Color, ColorRef, IconFiles, IconMask, MonoIcon};

use crate::theme::Theme;

/// Draws `icon` in its own colors.
#[must_use]
pub fn icon(icon: style::Icon) -> IconView {
    IconView { icon, color: None }
}

/// The size of `icon`'s box at 1x, as tdesktop computes it from the 1x mask.
#[must_use]
pub fn icon_size(icon: style::Icon) -> Size<Pixels> {
    icon.parts
        .iter()
        .fold(size(px(0.0), px(0.0)), |total, part| {
            let (width, height) = logical_size(part.mask).unwrap_or((0, 0));
            let width = part.padding.left.0 + width + part.padding.right.0;
            let height = part.padding.top.0 + height + part.padding.bottom.0;
            #[expect(clippy::cast_precision_loss, reason = "icon sizes are small")]
            size(
                total.width.max(px(width as f32)),
                total.height.max(px(height as f32)),
            )
        })
}

/// An icon element; sized to the icon unless styled otherwise.
#[derive(Debug, Clone, IntoElement)]
pub struct IconView {
    icon: style::Icon,
    color: Option<ColorRef>,
}

impl IconView {
    /// Draws every part in `color` instead (`icon.paint(..., color)`).
    #[must_use]
    pub fn color(mut self, color: ColorRef) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for IconView {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let box_size = icon_size(self.icon);
        canvas(
            |_, _, _| {},
            move |bounds, (), window, cx| paint(&self, bounds, window, cx),
        )
        .w(box_size.width)
        .h(box_size.height)
    }
}

fn paint(view: &IconView, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let scale = mask_scale(window.scale_factor());
    for part in view.icon.parts {
        let color = view
            .color
            .unwrap_or(part.color)
            .resolve(&cx.global::<Theme>().palette);
        let Some((width, height)) = logical_size(part.mask) else {
            continue;
        };
        let Some(image) = cx.default_global::<IconCache>().image(part, scale, color) else {
            continue;
        };
        #[expect(clippy::cast_precision_loss, reason = "icon sizes are small")]
        let image_bounds = Bounds::new(
            bounds.origin
                + point(
                    px(part.padding.left.0 as f32),
                    px(part.padding.top.0 as f32),
                ),
            size(px(width as f32), px(height as f32)),
        );
        if let Err(error) =
            window.paint_image(bounds, image_bounds, Corners::default(), image, 0, false)
        {
            tracing::warn!(%error, "cannot paint an icon");
        }
    }
}

/// tdesktop's mask resolutions: 1x, 2x and 3x.
fn mask_scale(scale_factor: f32) -> u8 {
    if scale_factor <= 1.0 {
        1
    } else if scale_factor <= 2.0 {
        2
    } else {
        3
    }
}

/// Decoded masks and their tinted images.
#[derive(Default)]
struct IconCache {
    images: HashMap<(IconMask, u8, Color), Option<Arc<RenderImage>>>,
}

impl Global for IconCache {}

impl IconCache {
    fn image(&mut self, part: &MonoIcon, scale: u8, color: Color) -> Option<Arc<RenderImage>> {
        self.images
            .entry((part.mask, scale, color))
            .or_insert_with(|| {
                let mask = load_mask(part.mask, scale);
                if mask.is_none() {
                    tracing::warn!(mask = ?part.mask, "cannot load an icon mask");
                }
                mask.map(|mask| Arc::new(colorize(&mask, color)))
            })
            .clone()
    }
}

/// The mask's alpha as tdesktop reads it: the first byte of premultiplied
/// ARGB, which is alpha for the white masks it ships.
fn load_mask(mask: IconMask, scale: u8) -> Option<RgbaImage> {
    let (name, modifiers) = match mask {
        IconMask::File { name, modifiers } => (name, modifiers),
        IconMask::Rect(rect) => {
            let factor = u32::from(scale);
            let width = u32::try_from(rect.width.0).ok()? * factor;
            let height = u32::try_from(rect.height.0).ok()? * factor;
            return Some(RgbaImage::from_pixel(width, height, image::Rgba([255; 4])));
        }
    };
    let IconFiles::Png { x1, x2, x3 } = style::icon_files(name)? else {
        // shortcut: no SVG masks are embedded yet; add resvg when one is.
        return None;
    };
    let bytes = match scale {
        1 => x1,
        2 => x2.unwrap_or(x1),
        _ => x3.or(x2).unwrap_or(x1),
    };
    let mut image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .ok()?
        .to_rgba8();
    for modifier in modifiers {
        image = apply_modifier(image, modifier, scale)?;
    }
    Some(image)
}

fn apply_modifier(image: RgbaImage, modifier: &str, scale: u8) -> Option<RgbaImage> {
    Some(match modifier {
        "flip_horizontal" => imageops::flip_horizontal(&image),
        "flip_vertical" => imageops::flip_vertical(&image),
        "rotate_cw" => imageops::rotate90(&image),
        "rotate_ccw" => imageops::rotate270(&image),
        "invert" => {
            let mut image = image;
            for pixel in image.pixels_mut() {
                pixel.0 = [255, 255, 255, 255 - mask_value(pixel.0)];
            }
            image
        }
        size => {
            let (width, height) = parse_size(size)?;
            let factor = u32::from(scale);
            imageops::resize(
                &image,
                width * factor,
                height * factor,
                imageops::FilterType::Triangle,
            )
        }
    })
}

fn parse_size(modifier: &str) -> Option<(u32, u32)> {
    let (width, height) = modifier.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn mask_value([_, _, blue, alpha]: [u8; 4]) -> u8 {
    let premultiplied = u16::from(blue) * u16::from(alpha) / 255;
    u8::try_from(premultiplied).unwrap_or(u8::MAX)
}

/// BGRA with straight alpha, as gpui's sprites expect.
fn colorize(mask: &RgbaImage, color: Color) -> RenderImage {
    let mut tinted = RgbaImage::new(mask.width(), mask.height());
    for (target, source) in tinted.pixels_mut().zip(mask.pixels()) {
        let alpha = u16::from(mask_value(source.0)) * u16::from(color.a) / 255;
        target.0 = [
            color.b,
            color.g,
            color.r,
            u8::try_from(alpha).unwrap_or(u8::MAX),
        ];
    }
    RenderImage::new([Frame::new(tinted)])
}

/// The 1x size of a mask, read from the PNG header and the modifiers.
fn logical_size(mask: IconMask) -> Option<(i32, i32)> {
    let (name, modifiers) = match mask {
        IconMask::File { name, modifiers } => (name, modifiers),
        IconMask::Rect(rect) => return Some((rect.width.0, rect.height.0)),
    };
    let IconFiles::Png { x1, .. } = style::icon_files(name)? else {
        return None;
    };
    // The IHDR chunk holds the width and height at bytes 16..24.
    let width = i32::try_from(u32::from_be_bytes(x1.get(16..20)?.try_into().ok()?)).ok()?;
    let height = i32::try_from(u32::from_be_bytes(x1.get(20..24)?.try_into().ok()?)).ok()?;
    let mut size = (width, height);
    for modifier in modifiers {
        size = match *modifier {
            "rotate_cw" | "rotate_ccw" => (size.1, size.0),
            "flip_horizontal" | "flip_vertical" | "invert" => size,
            other => {
                let (width, height) = parse_size(other)?;
                (i32::try_from(width).ok()?, i32::try_from(height).ok()?)
            }
        };
    }
    Some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sizes_from_the_1x_mask() {
        // intro_country_dropdown.png is 10x6 in tdesktop.
        let size = icon_size(style::intro::INTRO_COUNTRY_ICON);
        assert!(size.width > px(0.0) && size.height > px(0.0), "{size:?}");
        let mask = load_mask(style::intro::INTRO_COUNTRY_ICON.parts[0].mask, 2)
            .expect("the dropdown mask decodes");
        assert_eq!(f32::from(size.width) * 2.0, mask.width() as f32);
    }

    #[test]
    fn colorizes_with_the_mask_alpha() {
        let mask = RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 128]));
        let image = colorize(&mask, Color::rgba(10, 20, 30, 255));
        assert_eq!(image.as_bytes(0), Some(&[30, 20, 10, 128][..]));
    }
}
