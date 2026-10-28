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
    IconView {
        icon,
        color: None,
        side: None,
    }
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
    /// A square box the icon is scaled into.
    side: Option<Pixels>,
}

impl IconView {
    /// Draws every part in `color` instead (`icon.paint(..., color)`).
    #[must_use]
    pub fn color(mut self, color: ColorRef) -> Self {
        self.color = Some(color);
        self
    }

    /// Scales the icon into a `side`-wide square, as tdesktop scales a large
    /// mask down (`DownloadBar`'s `Make`).
    #[must_use]
    pub fn side(mut self, side: Pixels) -> Self {
        self.side = Some(side);
        self
    }
}

impl RenderOnce for IconView {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let box_size = self
            .side
            .map_or_else(|| icon_size(self.icon), |side| size(side, side));
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
    let natural = icon_size(view.icon);
    let zoom = view
        .side
        .map_or(1.0, |side| side / natural.width.max(px(1.0)));
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
                ) * zoom,
            size(px(width as f32 * zoom), px(height as f32 * zoom)),
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
    let (x1, x2, x3) = match style::icon_files(name)? {
        IconFiles::Png { x1, x2, x3 } => (x1, x2, x3),
        IconFiles::Svg(svg) => {
            let (width, height) = logical_size(mask)?;
            return render_svg(svg, width, height, scale);
        }
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
    let mut size = match style::icon_files(name)? {
        // The IHDR chunk holds the width and height at bytes 16..24.
        IconFiles::Png { x1, .. } => (
            i32::try_from(u32::from_be_bytes(x1.get(16..20)?.try_into().ok()?)).ok()?,
            i32::try_from(u32::from_be_bytes(x1.get(20..24)?.try_into().ok()?)).ok()?,
        ),
        IconFiles::Svg(svg) => svg_size(svg)?,
    };
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

/// An SVG mask at `width`x`height` logical pixels, as `QSvgRenderer` draws
/// it in `createIconImage`; the masks are white, so any channel is the mask.
fn render_svg(svg: &[u8], width: i32, height: i32, scale: u8) -> Option<RgbaImage> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let factor = u32::from(scale);
    let (width, height) = (
        u32::try_from(width).ok()? * factor,
        u32::try_from(height).ok()? * factor,
    );
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let size = tree.size();
    #[expect(clippy::cast_precision_loss, reason = "icon sizes are small")]
    let transform = resvg::tiny_skia::Transform::from_scale(
        width as f32 / size.width(),
        height as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let pixels = pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| [255, 255, pixel[3], 255])
        .collect();
    RgbaImage::from_raw(width, height, pixels)
}

/// The size an SVG declares: its `width` and `height`, or its `viewBox`.
fn svg_size(svg: &[u8]) -> Option<(i32, i32)> {
    let text = std::str::from_utf8(svg).ok()?;
    let tag = &text[text.find("<svg")?..];
    let tag = &tag[..tag.find('>')?];
    let attribute = |name: &str| -> Option<&str> {
        let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
        tag[start..].split('"').next()
    };
    let number = |value: &str| -> Option<i32> {
        let value: f32 = value.trim_end_matches("px").parse().ok()?;
        #[expect(clippy::cast_possible_truncation, reason = "icon sizes are small")]
        Some(value.round() as i32)
    };
    if let (Some(width), Some(height)) = (attribute("width"), attribute("height"))
        && let (Some(width), Some(height)) = (number(width), number(height))
    {
        return Some((width, height));
    }
    let mut view_box = attribute("viewBox")?.split_whitespace().skip(2);
    Some((number(view_box.next()?)?, number(view_box.next()?)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_svg_sizes_from_attributes_or_the_view_box() {
        assert_eq!(
            svg_size(br#"<svg width="72px" height="36px">"#),
            Some((72, 36))
        );
        assert_eq!(svg_size(br#"<svg viewBox="0 0 24 20">"#), Some((24, 20)));
        assert_eq!(svg_size(b"<svg>"), None);
    }

    #[test]
    fn renders_svg_masks_at_the_modifier_size() {
        let size = icon_size(style::dialogs::DIALOGS_UNREAD_MENTION.icon);
        assert_eq!(size, gpui::size(px(18.0), px(18.0)));
        let IconMask::File { name, .. } = style::dialogs::DIALOGS_UNREAD_MENTION.icon.parts[0].mask
        else {
            panic!("the mention icon is a file");
        };
        let Some(IconFiles::Svg(svg)) = style::icon_files(name) else {
            panic!("the mention icon is an SVG");
        };
        let mask = render_svg(svg, 18, 18, 2).expect("the SVG renders");
        assert_eq!((mask.width(), mask.height()), (36, 36));
        assert!(mask.pixels().any(|pixel| mask_value(pixel.0) > 0));
    }

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
