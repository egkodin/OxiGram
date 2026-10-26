//! rgram's widgets. Each one follows a lib_ui widget of Telegram Desktop and
//! takes its sizes and colors from the matching `style` struct.

pub mod anim;
mod button;
mod checkbox;
mod font;
mod icon;
mod input;
mod label;
mod layer;
mod qr;
mod radial;
mod ripple;
mod theme;

use std::borrow::Cow;

use gpui::App;

pub use button::{IconButton, LinkButton, RoundButton};
pub use checkbox::{Checkbox, Checked};
pub use font::{FONT_FAMILY, font, font_height, text_style};
pub use icon::{IconView, icon, icon_size};
pub use input::{Corrector, InputEvent, InputField, InputMode};
pub use label::{FlatLabel, Markup};
pub use layer::BoxFrame;
pub use qr::{QrCode, QrData, QrError};
pub use radial::{InfiniteRadial, RadialState, paint_arc};
pub use theme::{Theme, color, hsla};

/// The fonts tdesktop bundles (`lib_ui/fonts`); Vazirmatn covers Arabic script.
const FONTS: [&[u8]; 6] = [
    include_bytes!("../fonts/OpenSans-Regular.ttf"),
    include_bytes!("../fonts/OpenSans-Italic.ttf"),
    include_bytes!("../fonts/OpenSans-SemiBold.ttf"),
    include_bytes!("../fonts/OpenSans-SemiBoldItalic.ttf"),
    include_bytes!("../fonts/Vazirmatn-UI-NL-Regular.ttf"),
    include_bytes!("../fonts/Vazirmatn-UI-NL-SemiBold.ttf"),
];

/// The text system rejected a bundled font.
#[derive(Debug, thiserror::Error)]
#[error("cannot load the bundled fonts")]
pub struct FontError(#[source] Box<dyn std::error::Error + Send + Sync>);

/// Loads the fonts, the default theme and the widgets' key bindings.
///
/// # Errors
///
/// Fails if the text system rejects a bundled font.
pub fn init(cx: &mut App) -> Result<(), FontError> {
    cx.text_system()
        .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
        .map_err(|error| FontError(error.into()))?;
    cx.set_global(Theme::default());
    input::bind_keys(cx);
    Ok(())
}
