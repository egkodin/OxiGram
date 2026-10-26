use gpui::{App, Global, Hsla, Rgba};
use style::{Color, ColorRef, Palette};

/// The palette every widget draws with. Themes replace it as a whole.
#[derive(Debug, Clone, Default)]
pub struct Theme {
    pub palette: Palette,
}

impl Global for Theme {}

/// `color` under the current theme.
#[must_use]
pub fn color(cx: &App, color: ColorRef) -> Hsla {
    hsla(color.resolve(&cx.global::<Theme>().palette))
}

#[must_use]
pub fn hsla(color: Color) -> Hsla {
    Hsla::from(Rgba {
        r: f32::from(color.r) / 255.0,
        g: f32::from(color.g) / 255.0,
        b: f32::from(color.b) / 255.0,
        a: f32::from(color.a) / 255.0,
    })
}
