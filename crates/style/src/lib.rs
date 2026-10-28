//! Style values taken from Telegram Desktop by `tools/style-gen`.
//!
//! The generated modules mirror tdesktop's `.style` files ([`basic`],
//! [`widgets`], [`intro`]); the types they use are defined here. Sizes are
//! logical pixels at 100 % interface scale, and colors name palette entries so
//! a theme can replace them.

pub mod basic;
pub mod boxes;
pub mod calls;
pub mod chat;
pub mod chat_helpers;
pub mod chat_style;
pub mod choose_date_time;
pub mod country_select_box;
pub mod dialogs;
pub mod dialogs_layout;
pub mod dialogs_widget;
pub mod emoji;
pub mod filter_icons;
pub mod history_view_voice_record_bar;
mod icons;
pub mod info;
pub mod intro;
pub mod layers;
pub mod media_player;
pub mod media_stories;
pub mod media_view;
pub mod menu_icons;
pub mod overview;
mod palette;
pub mod polls;
pub mod premium;
pub mod premium_limits;
pub mod settings;
pub mod userpic_button;
pub mod widgets;
pub mod window;
pub mod window_main_menu;

pub use icons::icon_files;
pub use palette::{ColorId, Palette};

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

/// A color in a style value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorRef {
    /// A palette entry, which themes can change.
    Palette(ColorId),
    /// A fixed color: `transparent`, or a field the style left unset (black).
    Fixed(Color),
}

impl ColorRef {
    /// The color under `palette`.
    #[must_use]
    pub const fn resolve(self, palette: &Palette) -> Color {
        match self {
            Self::Palette(id) => palette.get(id),
            Self::Fixed(color) => color,
        }
    }
}

/// Logical pixels at 100 % interface scale (`Npx` in a style file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Px(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Point {
    pub x: Px,
    pub y: Px,
}

impl Point {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x: Px(x), y: Px(y) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size {
    pub width: Px,
    pub height: Px,
}

impl Size {
    #[must_use]
    pub const fn new(width: i32, height: i32) -> Self {
        Self {
            width: Px(width),
            height: Px(height),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Margins {
    pub left: Px,
    pub top: Px,
    pub right: Px,
    pub bottom: Px,
}

impl Margins {
    #[must_use]
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left: Px(left),
            top: Px(top),
            right: Px(right),
            bottom: Px(bottom),
        }
    }
}

/// `align(...)`: where content sits in its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Align {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Center,
    Justify,
}

/// `bold`/`semibold`, `italic` and `underline` in `font(...)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FontFlags(u8);

impl FontFlags {
    pub const NONE: Self = Self(0);
    /// Drawn semibold: tdesktop maps both `bold` and `semibold` to it.
    pub const BOLD: Self = Self(0x01);
    pub const ITALIC: Self = Self(0x02);
    pub const UNDERLINE: Self = Self(0x04);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// `font(size flags "family")`; an empty family means the default, Open Sans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Font {
    pub size: Px,
    pub flags: FontFlags,
    pub family: &'static str,
}

impl Font {
    #[must_use]
    pub const fn new(size: i32, flags: FontFlags, family: &'static str) -> Self {
        Self {
            size: Px(size),
            flags,
            family,
        }
    }
}

/// An icon: one or more single-color masks drawn on top of each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon {
    pub parts: &'static [MonoIcon],
}

impl Icon {
    pub const EMPTY: Self = Self { parts: &[] };

    #[must_use]
    pub const fn new(parts: &'static [MonoIcon]) -> Self {
        Self { parts }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonoIcon {
    pub mask: IconMask,
    pub color: ColorRef,
    /// Offset of the mask inside the icon's box.
    pub padding: Margins,
}

impl MonoIcon {
    #[must_use]
    pub const fn new(mask: IconMask, color: ColorRef, padding: Margins) -> Self {
        Self {
            mask,
            color,
            padding,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconMask {
    /// An image from [`icon_files`]; its alpha is the mask. Modifiers such as
    /// `flip_horizontal` or `24x24` apply in order.
    File {
        name: &'static str,
        modifiers: &'static [&'static str],
    },
    /// A filled rectangle.
    Rect(Size),
}

/// The embedded images of one icon mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconFiles {
    /// Bitmaps for 1×, 2× and 3× scale; tdesktop ships at least the first.
    Png {
        x1: &'static [u8],
        x2: Option<&'static [u8]>,
        x3: Option<&'static [u8]>,
    },
    Svg(&'static [u8]),
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
        assert_eq!(palette.get(ColorId::WindowBg), palette.window_bg);
    }

    #[test]
    fn intro_styles_match_tdesktop() {
        // intro.style: `introNextButton: RoundButton(defaultActiveButton) { width: 300px; ... }`
        assert_eq!(intro::INTRO_NEXT_BUTTON.width, Px(300));
        assert_eq!(intro::INTRO_NEXT_BUTTON.radius, Px(6));
        // Inherited from defaultActiveButton in widgets.style.
        assert_eq!(
            intro::INTRO_NEXT_BUTTON.text_bg,
            ColorRef::Palette(ColorId::ActiveButtonBg)
        );
        assert_eq!(intro::INTRO_NEXT_BUTTON.style.font.flags, FontFlags::BOLD);
        // `font(boxFontSize semibold)` with `boxFontSize: 14px` from basic.style.
        assert_eq!(intro::INTRO_NEXT_BUTTON.style.font.size, Px(14));
        // `introPhone: InputField(introCountry) { textMargins: ...; width: 225px; }`
        assert_eq!(intro::INTRO_PHONE.width, Px(225));
        assert_eq!(intro::INTRO_PHONE.height_min, Px(61));
        assert_eq!(intro::INTRO_PHONE.text_margins, Margins::new(12, 27, 12, 6));
        assert!(icon_files("intro_country_dropdown").is_some());
    }
}
