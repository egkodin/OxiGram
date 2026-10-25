//! Style values taken from Telegram Desktop by `tools/style-gen`.

mod palette;

pub use palette::Palette;

/// An sRGB color with straight (not premultiplied) alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_palette_matches_tdesktop() {
        let palette = Palette::default();
        assert_eq!(palette.window_bg, Color::rgba(0xff, 0xff, 0xff, 0xff));
        // `windowFgOver: windowFg;` resolves to windowFg's value.
        assert_eq!(palette.window_fg_over, palette.window_fg);
        // `shadowFg: #00000018;` keeps its alpha.
        assert_eq!(palette.shadow_fg, Color::rgba(0, 0, 0, 0x18));
    }
}
