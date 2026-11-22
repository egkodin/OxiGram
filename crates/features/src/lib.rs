//! oxigram's screens. Each follows a Telegram Desktop screen and reads the
//! model through the [`Store`]; none of them sees a TDLib type.

pub mod accounts;
pub mod boxes;
mod call_history;
mod columns;
mod connecting;
mod contacts_box;
mod create_chat;
mod currency;
mod dialogs;
mod downloads;
mod file_sizes;
mod hashtags;
mod history;
mod info;
mod intro;
pub mod lang;
mod launch;
pub mod layers;
mod links;
mod lottie;
mod main_menu;
mod main_view;
mod payments;
mod peer_list;
mod peer_menu;
mod player;
mod power_saving;
mod presence;
mod ranked;
mod recipients_box;
mod root;
pub mod settings;
mod settings_screen;
mod speed;
mod status;
pub mod store;
mod streaming;
mod swipe;
pub mod title_bar;

/// What the app calls itself in its title bar and menu footer.
pub const APP_NAME: &str = "OxiGram";
pub mod toast;

use gpui::{App, Pixels, px};

pub use accounts::{AccountId, Accounts};
pub use history::{preset_message_abilities, preset_voice_samples};
pub use lang::Lang;
pub use layers::Layers;
pub use lottie::{Stats as LottieStats, stats as lottie_stats};
pub use root::Root;
pub use settings_screen::preset_premium_offer;
pub use store::{Store, StoreEvent};

gpui::actions!(
    oxigram,
    [
        /// Quits as the Quit item does, then starts oxigram again (`Core::Restart`).
        RestartApp
    ]
);

/// Installs the screens' globals and key bindings. Call after `ui::init`.
pub fn init(cx: &mut App) {
    cx.set_global(Lang::default());
    history::init_picture_cache(cx);
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
