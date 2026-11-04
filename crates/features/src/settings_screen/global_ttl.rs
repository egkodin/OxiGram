//! "Auto-Delete Messages" (`Settings::GlobalTTL`): how long messages in
//! new chats live.
// shortcut: no Lottie timer at the top, no "Set Custom Time" picker (`TTLMenu::TTLBox`), and the "apply to existing chats" link is plain text; add them with Lottie, a duration picker and a chat picker. With row "Auto-delete messages".

use std::rc::Rc;

use gpui::{
    App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, Task, Window, div,
};
use i18n::key;
use style::layers::DEFAULT_BOX_CHECKBOX;
use style::widgets::{DEFAULT_RADIO, DEFAULT_TOAST};
use ui::{Checkbox, Checked, Markup};

use super::{Env, rows};
use crate::boxes::{Callback, ConfirmArgs};
use crate::lang::{tr, tr_plural, tr_with};
use crate::px_of;

const HOUR: i32 = 3600;
const DAY: i32 = 86_400;
/// The choices but a custom one (`RebuildButtons`).
const CHOICES: [i32; 4] = [0, DAY, 7 * DAY, 31 * DAY];

/// "1 day", "2 weeks" (`Ui::FormatTTLAfter`).
fn ttl_after(seconds: i32, cx: &App) -> String {
    let (unit, count) = if seconds <= 23 * HOUR {
        (key::lng_settings_ttl_after_hours, seconds / HOUR)
    } else if seconds <= 6 * DAY {
        (key::lng_settings_ttl_after_days, seconds / DAY)
    } else if seconds <= 3 * 7 * DAY {
        (key::lng_settings_ttl_after_weeks, seconds / (7 * DAY))
    } else if seconds <= 11 * 31 * DAY {
        (key::lng_settings_ttl_after_months, seconds / (31 * DAY))
    } else {
        let years = (f64::from(seconds) / f64::from(365 * DAY)).round();
        #[expect(clippy::cast_possible_truncation, reason = "a few years at most")]
        (key::lng_settings_ttl_after_years, years as i32)
    };
    tr_plural(cx, unit, i64::from(count), &[])
}

/// The choices with the current one when it is custom.
fn choices(current: i32) -> Vec<i32> {
    let mut all = CHOICES.to_vec();
    if !all.contains(&current) {
        all.push(current);
        all.sort_unstable();
    }
    all
}

pub(super) struct GlobalTtlPage {
    env: Env,
    /// None while Telegram is asked.
    current: Option<i32>,
    _load: Option<Task<()>>,
}

impl GlobalTtlPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let task = crate::store::spawn(
            env.targets.store.read(cx).session().default_auto_delete(),
            cx,
        );
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(seconds) => {
                let _ = this.update(cx, |this, cx| {
                    this.current = Some(seconds);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "the default auto-delete time failed"),
        });
        Self {
            env,
            current: None,
            _load: Some(load),
        }
    }

    /// Off applies at once, a period after the question unless one is on
    /// already (`showSure`).
    fn choose(&self, seconds: i32, window: &mut Window, cx: &mut Context<Self>) {
        if self.current == Some(seconds) {
            return;
        }
        let weak = cx.weak_entity();
        let apply: Callback = Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| this.apply(seconds, cx));
        });
        if seconds == 0 || self.current.is_some_and(|current| current != 0) {
            apply(window, cx);
            return;
        }
        let text = tr_with(
            cx,
            key::lng_settings_ttl_after_sure,
            &[("after_duration", &ttl_after(seconds, cx))],
        );
        let mut args = ConfirmArgs::new(Markup::plain(text));
        args.confirm_text = Some(tr(cx, key::lng_sure_enable));
        args.cancel_text = Some(tr(cx, key::lng_cancel));
        args.confirmed = Some(apply);
        crate::boxes::confirm(&self.env.targets.layers, args, window, cx);
    }

    fn apply(&mut self, seconds: i32, cx: &mut Context<Self>) {
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .set_default_auto_delete(seconds),
            cx,
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "setting the default auto-delete time failed");
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.current = Some(seconds);
                if seconds != 0 {
                    let text = tr_with(
                        cx,
                        key::lng_settings_ttl_after_toast,
                        &[("after_duration", &ttl_after(seconds, cx))],
                    );
                    this.env.targets.layers.update(cx, |layers, cx| {
                        let duration = crate::toast::DEFAULT_DURATION;
                        layers.show_toast(text.into(), &DEFAULT_TOAST, duration, cx);
                    });
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// A choice: its text and the radio at the right.
    fn choice(&self, seconds: i32, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let text = if seconds == 0 {
            tr(cx, key::lng_settings_ttl_after_off)
        } else {
            let after = ttl_after(seconds, cx);
            tr_with(
                cx,
                key::lng_settings_ttl_after,
                &[("after_duration", &after)],
            )
            .into()
        };
        let checked = if self.current == Some(seconds) {
            Checked::Yes
        } else {
            Checked::No
        };
        let id = usize::try_from(seconds).unwrap_or_default();
        let padding = style::settings::SETTINGS_BUTTON_NO_ICON.padding;
        let weak = cx.weak_entity();
        div()
            .id(("ttl-choice", id))
            .relative()
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.choose(seconds, window, cx));
            })
            .child(rows::plain(("ttl-choice-row", id), text))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px_of(padding.left))
                    .flex()
                    .items_center()
                    .child(Checkbox::radio(
                        ("ttl-choice-radio", id),
                        &DEFAULT_BOX_CHECKBOX,
                        &DEFAULT_RADIO,
                        "",
                        checked,
                    )),
            )
    }
}

impl Render for GlobalTtlPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(current) = self.current else {
            return div().child(rows::divider_text(tr(cx, key::lng_contacts_loading), cx));
        };
        let buttons: Vec<_> = choices(current)
            .into_iter()
            .map(|seconds| self.choice(seconds, cx))
            .collect();
        let link = tr(cx, key::lng_settings_ttl_after_about_link);
        let about = tr_with(cx, key::lng_settings_ttl_after_about, &[("link", &link)]);
        div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(rows::subtitle(
                tr(cx, key::lng_settings_ttl_after_subtitle),
                cx,
            ))
            .children(buttons)
            .child(rows::skip())
            .child(rows::divider_text(about, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_custom_period_joins_the_choices_in_order() {
        assert_eq!(choices(DAY), CHOICES.to_vec());
        assert_eq!(choices(2 * DAY), vec![0, DAY, 2 * DAY, 7 * DAY, 31 * DAY]);
    }
}
