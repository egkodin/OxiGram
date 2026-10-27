//! The settings reachable before the settings screen: how messages send
//! (`SetupMessages`'s Enter or Ctrl/Cmd+Enter) and logging out.

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, Styled, WeakEntity, Window, div, prelude::*,
};
use i18n::key;
use style::layers::{
    ATTENTION_BOX_BUTTON, DEFAULT_BOX, DEFAULT_BOX_CHECKBOX, DEFAULT_SUBSECTION_TITLE,
    DEFAULT_SUBSECTION_TITLE_PADDING,
};
use style::widgets::DEFAULT_RADIO;
use ui::{BoxFrame, Checkbox, Checked, FlatLabel, Markup, RoundButton};

use crate::boxes::Callback;
use crate::lang::tr;
use crate::layers::{Dismiss, Layers};
use crate::settings::{self, SendBy};

/// `settingsSendTypePadding`, which lives in settings.style.
// shortcut: the settings style is not generated yet; it comes with the settings screen in M7, with `settingsCheckbox`'s wider text offset.
const SEND_TYPE_PADDING: (i32, i32) = (22, 5);

pub(crate) struct SettingsBox {
    layers: WeakEntity<Layers>,
    on_logout: Callback,
    focus: FocusHandle,
}

impl std::fmt::Debug for SettingsBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsBox").finish_non_exhaustive()
    }
}

impl Focusable for SettingsBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

// shortcut: a box stands in for the settings screen until M7, which moves these into "Chat Settings".
pub(crate) fn show(
    layers: &Entity<Layers>,
    on_logout: Callback,
    window: &mut Window,
    cx: &mut App,
) {
    let weak = layers.downgrade();
    let view = cx.new(|cx| SettingsBox {
        layers: weak,
        on_logout,
        focus: cx.focus_handle(),
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx);
    });
}

fn close(layers: &WeakEntity<Layers>, window: &mut Window, cx: &mut App) {
    if let Some(layers) = layers.upgrade() {
        layers.update(cx, |layers, cx| layers.close(window, cx));
    }
}

impl SettingsBox {
    fn send_by_choice(send_by: SendBy, cx: &mut Context<Self>) -> impl IntoElement {
        let text = match send_by {
            SendBy::Enter => key::lng_settings_send_enter,
            SendBy::CtrlEnter if cfg!(target_os = "macos") => key::lng_settings_send_cmdenter,
            SendBy::CtrlEnter => key::lng_settings_send_ctrlenter,
        };
        let checked = if settings::send_by(cx) == send_by {
            Checked::Yes
        } else {
            Checked::No
        };
        let id = match send_by {
            SendBy::Enter => "send-by-enter",
            SendBy::CtrlEnter => "send-by-ctrl-enter",
        };
        let entity = cx.entity();
        let (left, vertical) = SEND_TYPE_PADDING;
        div().pl(px_of(left)).py(px_of(vertical)).child(
            Checkbox::radio(
                id,
                &DEFAULT_BOX_CHECKBOX,
                &DEFAULT_RADIO,
                tr(cx, text),
                checked,
            )
            .on_toggle(move |_, cx| {
                settings::set_send_by(send_by, cx);
                entity.update(cx, |_, cx| cx.notify());
            }),
        )
    }
}

impl Render for SettingsBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let padding = DEFAULT_SUBSECTION_TITLE_PADDING;
        let title = div()
            .pl(px_of(padding.left.0))
            .pt(px_of(padding.top.0))
            .pb(px_of(padding.bottom.0))
            .child(FlatLabel::new(
                "settings-messages",
                &DEFAULT_SUBSECTION_TITLE,
                Markup::plain(tr(cx, key::lng_settings_messages).to_string()),
            ));
        let choices = div()
            .pb(px_of(padding.bottom.0))
            .child(Self::send_by_choice(SendBy::Enter, cx))
            .child(Self::send_by_choice(SendBy::CtrlEnter, cx));
        let layers = self.layers.clone();
        let on_logout = self.on_logout.clone();
        let logout = RoundButton::new(
            "settings-logout",
            &ATTENTION_BOX_BUTTON,
            tr(cx, key::lng_settings_logout),
        )
        .on_click({
            let layers = layers.clone();
            move |_, window, cx| {
                close(&layers, window, cx);
                on_logout(window, cx);
            }
        });
        let close = RoundButton::new(
            "settings-close",
            &DEFAULT_BOX.button,
            tr(cx, key::lng_close),
        )
        .on_click(move |_, window, cx| close(&layers, window, cx));
        div().track_focus(&self.focus).child(
            BoxFrame::new("settings-box", &DEFAULT_BOX)
                .title(tr(cx, key::lng_menu_settings))
                .row(title)
                .row(choices)
                .button(close)
                .button(logout),
        )
    }
}

fn px_of(value: i32) -> gpui::Pixels {
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    gpui::px(value as f32)
}
