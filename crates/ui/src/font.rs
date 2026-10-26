use gpui::{FontStyle, FontWeight, Hsla, Pixels, TextStyle, px};

/// tdesktop's interface font, bundled so text measures the same on every OS.
pub const FONT_FAMILY: &str = "Open Sans";

/// Open Sans `hhea` ascender and descender over its 2048 units per em; Qt
/// derives tdesktop's font heights from them.
const ASCENT: f32 = 2189.0 / 2048.0;
const DESCENT: f32 = 600.0 / 2048.0;

/// The gpui font for a style font. tdesktop draws `bold` as semibold.
#[must_use]
pub fn font(font: style::Font) -> gpui::Font {
    let family = if font.family.is_empty() {
        FONT_FAMILY
    } else {
        font.family
    };
    gpui::Font {
        family: family.into(),
        weight: if font.flags.contains(style::FontFlags::BOLD) {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        },
        style: if font.flags.contains(style::FontFlags::ITALIC) {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..gpui::font(FONT_FAMILY)
    }
}

/// The line height tdesktop uses for `font` (`font->height`): Qt's rounded
/// ascent plus descent.
#[must_use]
pub fn font_height(font: style::Font) -> Pixels {
    #[expect(clippy::cast_precision_loss, reason = "font sizes are small")]
    let size = font.size.0 as f32;
    px((size * (ASCENT + DESCENT)).round())
}

/// A text style drawing `font` in `color`, one `font_height` per line.
#[must_use]
pub fn text_style(font: style::Font, color: Hsla) -> TextStyle {
    let gpui_font = self::font(font);
    #[expect(clippy::cast_precision_loss, reason = "font sizes are small")]
    let size = font.size.0 as f32;
    TextStyle {
        color,
        font_family: gpui_font.family,
        font_features: gpui_font.features,
        font_fallbacks: gpui_font.fallbacks,
        font_size: px(size).into(),
        line_height: font_height(font).into(),
        font_weight: gpui_font.weight,
        font_style: gpui_font.style,
        ..TextStyle::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_match_qt() {
        // st::normalFont (13px) is 18px high in tdesktop; introTitle (17px) 23px.
        assert_eq!(
            font_height(style::Font::new(13, style::FontFlags::NONE, "")),
            px(18.0)
        );
        assert_eq!(
            font_height(style::Font::new(17, style::FontFlags::BOLD, "")),
            px(23.0)
        );
    }
}
