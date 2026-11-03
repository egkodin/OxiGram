//! rgram's screens. Each follows a Telegram Desktop screen and reads the
//! model through the [`Store`]; none of them sees a TDLib type.

pub mod boxes;
mod columns;
mod connecting;
mod contacts_box;
mod dialogs;
mod downloads;
mod file_sizes;
mod hashtags;
mod history;
mod intro;
pub mod lang;
mod launch;
pub mod layers;
mod links;
mod main_menu;
mod main_view;
mod peer_list;
mod peer_menu;
mod player;
mod ranked;
mod recipients_box;
mod root;
pub mod settings;
mod settings_box;
mod status;
pub mod store;
pub mod toast;

use gpui::{App, Pixels, px};

pub use history::preset_voice_samples;
pub use lang::Lang;
pub use layers::Layers;
pub use root::Root;
pub use store::{Store, StoreEvent};

/// Installs the screens' globals and key bindings. Call after `ui::init`.
pub fn init(cx: &mut App) {
    cx.set_global(Lang::default());
    layers::bind_keys(cx);
    boxes::bind_keys(cx);
    history::bind_keys(cx);
    intro::bind_keys(cx);
    main_view::bind_keys(cx);
}

/// A style length in window pixels.
pub(crate) fn px_of(value: style::Px) -> Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    px(value.0 as f32)
}

/// Focuses a focusable view.
pub(crate) fn focus<V: gpui::Focusable>(
    view: &gpui::Entity<V>,
    window: &mut gpui::Window,
    cx: &mut App,
) {
    let handle = view.read(cx).focus_handle(cx);
    handle.focus(window, cx);
}
