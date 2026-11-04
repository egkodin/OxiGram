//! "Speakers and Camera" (`Settings::Calls`): the output and input devices,
//! accepting calls here and the system's sound settings.
// shortcut: no microphone level test (it opens the microphone), no separate call devices or camera section; with M12's calls.

use gpui::{App, Context, IntoElement, ParentElement, Render, Styled, Task, Window, div};
use i18n::key;
use media::devices::{AudioDevice, Direction};
use model::sessions::SessionId;

use super::{Env, device_box, rows};
use crate::lang::tr;

/// macOS Settings' Sound pane (`OpenSystemSettings(Audio)`).
const SOUND_SETTINGS: &str = "x-apple.systempreferences:com.apple.Sound-Settings.extension";

pub(super) struct CallsPage {
    env: Env,
    outputs: Vec<AudioDevice>,
    inputs: Vec<AudioDevice>,
    /// This session and whether calls ring here; None while TDLib is asked.
    accepts_calls: Option<(SessionId, bool)>,
    _task: Task<()>,
}

impl CallsPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let task = crate::store::spawn(env.targets.store.read(cx).session().active_sessions(), cx);
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(list) => {
                let current = list.sessions.into_iter().find(|session| session.is_current);
                let _ = this.update(cx, |this, cx| {
                    this.accepts_calls =
                        current.map(|session| (session.id, session.can_accept_calls));
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the sessions failed"),
        });
        Self {
            env,
            outputs: media::devices::list(Direction::Output),
            inputs: media::devices::list(Direction::Input),
            accepts_calls: None,
            _task: load,
        }
    }

    fn set_accepts_calls(&mut self, id: SessionId, value: bool, cx: &mut Context<Self>) {
        self.accepts_calls = Some((id, value));
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .set_session_accepts_calls(id, value),
            cx,
        );
        self._task = cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "changing accept calls failed");
                let _ = this.update(cx, |this, cx| {
                    this.accepts_calls = Some((id, !value));
                    cx.notify();
                });
            }
        });
        cx.notify();
    }

    fn device_row(&self, direction: Direction, cx: &App) -> ui::SettingsButton {
        let devices = match direction {
            Direction::Output => &self.outputs,
            Direction::Input => &self.inputs,
        };
        let env = self.env.clone();
        let id = match direction {
            Direction::Output => "calls-output",
            Direction::Input => "calls-input",
        };
        rows::plain(id, tr(cx, device_box::title(direction)))
            .label(device_box::chosen_name(direction, devices, cx))
            .on_click(move |_, window, cx| device_box::show(&env, direction, window, cx))
    }
}

impl Render for CallsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accept = self.accepts_calls.map(|(id, value)| {
            let weak = cx.weak_entity();
            rows::plain("calls-accept", tr(cx, key::lng_settings_call_accept_calls))
                .toggle(value)
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.set_accepts_calls(id, !value, cx));
                })
        });
        let system = rows::plain(
            "calls-system",
            tr(cx, key::lng_settings_call_open_system_prefs),
        )
        .on_click(|_, _, cx| cx.open_url(SOUND_SETTINGS));
        div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(rows::subtitle(
                tr(cx, key::lng_settings_call_section_output),
                cx,
            ))
            .child(self.device_row(Direction::Output, cx))
            .child(rows::skip())
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(
                tr(cx, key::lng_settings_call_section_input),
                cx,
            ))
            .child(self.device_row(Direction::Input, cx))
            .child(rows::skip())
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(
                tr(cx, key::lng_settings_call_section_other),
                cx,
            ))
            .children(accept)
            .child(system)
            .child(rows::skip())
    }
}
