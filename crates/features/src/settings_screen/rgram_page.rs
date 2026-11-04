//! "rgram settings": the app's own options, where tdesktop forks keep
//! theirs. How messages send is also in Chat Settings, as tdesktop has it.

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use i18n::key;
use style::settings::{SETTINGS_SEND_TYPE, SETTINGS_SEND_TYPE_PADDING};
use style::widgets::DEFAULT_RADIO;
use ui::{Checkbox, Checked};

use super::rows;
use crate::lang::tr;
use crate::px_of;
use crate::settings::{self, SendBy};

pub(super) struct RgramPage;

/// One of the send-by radios (`SetupMessages`).
pub(super) fn send_by_choice<V: 'static>(send_by: SendBy, cx: &mut Context<V>) -> impl IntoElement {
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
    let padding = SETTINGS_SEND_TYPE_PADDING;
    div()
        .pl(px_of(padding.left))
        .pr(px_of(padding.right))
        .pt(px_of(padding.top))
        .pb(px_of(padding.bottom))
        .child(
            Checkbox::radio(
                id,
                &SETTINGS_SEND_TYPE,
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

impl Render for RgramPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(rows::subtitle(tr(cx, key::lng_settings_messages), cx))
            .child(send_by_choice(SendBy::Enter, cx))
            .child(send_by_choice(SendBy::CtrlEnter, cx))
            .child(rows::skip())
    }
}
