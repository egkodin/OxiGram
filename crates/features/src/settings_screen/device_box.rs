//! "Output device" / "Input device" (`ChooseMediaDeviceBox`): Default and
//! the system's devices; a choice applies at once, OK closes.

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Window, div,
};
use i18n::key;
use media::devices::{AudioDevice, Direction};
use style::layers::{BOX_OPTION_LIST_SKIP, BOX_PADDING, DEFAULT_BOX, DEFAULT_BOX_CHECKBOX};
use style::widgets::DEFAULT_RADIO;
use ui::{BoxFrame, Checkbox, Checked, RoundButton};

use super::Env;
use crate::lang::tr;
use crate::px_of;
use crate::settings;

/// The row's title and the box's.
pub(super) fn title(direction: Direction) -> i18n::Key {
    match direction {
        Direction::Output => key::lng_settings_call_output_device,
        Direction::Input => key::lng_settings_call_input_device,
    }
}

/// The chosen device's name, "Default" for the system's or one unplugged
/// (`DeviceNameValue`).
pub(super) fn chosen_name(
    direction: Direction,
    devices: &[AudioDevice],
    cx: &App,
) -> gpui::SharedString {
    settings::audio_device(direction, cx)
        .and_then(|id| devices.iter().find(|device| device.id == id))
        .map_or_else(
            || tr(cx, key::lng_settings_call_device_default),
            |device| device.name.clone().into(),
        )
}

pub(super) struct DeviceBox {
    env: Env,
    direction: Direction,
    devices: Vec<AudioDevice>,
    focus: FocusHandle,
}

impl Focusable for DeviceBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(env: &Env, direction: Direction, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| DeviceBox {
        env: env.clone(),
        direction,
        devices: media::devices::list(direction),
        focus: cx.focus_handle(),
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl DeviceBox {
    fn radio(
        &self,
        index: usize,
        id: Option<String>,
        name: gpui::SharedString,
        cx: &mut Context<Self>,
    ) -> Checkbox {
        let checked = if settings::audio_device(self.direction, cx) == id {
            Checked::Yes
        } else {
            Checked::No
        };
        let direction = self.direction;
        Checkbox::radio(
            ("audio-device", index),
            &DEFAULT_BOX_CHECKBOX,
            &DEFAULT_RADIO,
            name,
            checked,
        )
        .on_toggle(move |window, cx| {
            settings::set_audio_device(direction, id.clone(), cx);
            window.refresh();
        })
    }
}

impl Render for DeviceBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let default = self.radio(0, None, tr(cx, key::lng_settings_call_device_default), cx);
        let devices: Vec<_> = self
            .devices
            .clone()
            .into_iter()
            .enumerate()
            .map(|(index, device)| self.radio(index + 1, Some(device.id), device.name.into(), cx))
            .collect();
        let content = div()
            .flex()
            .flex_col()
            .pl(px_of(BOX_PADDING.left))
            .pr(px_of(BOX_PADDING.right))
            .pb(px_of(BOX_PADDING.bottom))
            .gap(px_of(BOX_OPTION_LIST_SKIP))
            .child(default)
            .children(devices);
        let layers = self.env.targets.layers.clone();
        div().track_focus(&self.focus).child(
            BoxFrame::new("audio-device-box", &DEFAULT_BOX)
                .title(tr(cx, title(self.direction)))
                .row(content)
                .button(
                    RoundButton::new(
                        "audio-device-ok",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_box_ok),
                    )
                    .on_click(move |_, window, cx| {
                        layers.update(cx, |layers, cx| layers.close(window, cx));
                    }),
                ),
        )
    }
}
