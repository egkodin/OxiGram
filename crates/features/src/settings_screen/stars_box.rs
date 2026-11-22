//! "Top Up Balance" (`FillCreditOptions`): Telegram's Stars packages with
//! their prices; a package goes on to the payment form, where oxigram stops.
// shortcut: one star glyph in place of tdesktop's growing stacks of stars (`GenerateStars`); with M13.

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Task, Window, div,
};
use i18n::key;
use model::stars::TopUpOption;
use style::credits::CREDITS_TOPUP_BUTTON;
use style::layers::DEFAULT_BOX;
use ui::{BoxFrame, RoundButton, SettingsButton};

use super::{Env, rows};
use crate::lang::{tr, tr_plural_decimal};

const STAR: char = '\u{2B50}';

pub(super) struct StarsBox {
    env: Env,
    /// None while TDLib is asked.
    options: Option<Vec<TopUpOption>>,
    is_extended: bool,
    focus: FocusHandle,
    _load: Task<()>,
}

impl Focusable for StarsBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(env: &Env, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx: &mut Context<StarsBox>| {
        let task = crate::store::spawn(
            env.targets.store.read(cx).session().star_top_up_options(),
            cx,
        );
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(options) => {
                let _ = this.update(cx, |this, cx| {
                    this.options = Some(options);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the Stars packages failed"),
        });
        StarsBox {
            env: env.clone(),
            options: None,
            is_extended: false,
            focus: cx.focus_handle(),
            _load: load,
        }
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl StarsBox {
    fn buy(&self, option: &TopUpOption, window: &mut Window, cx: &mut App) {
        let targets = self.env.targets.clone();
        targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
        crate::payments::open(option.link(), &targets, window, cx);
    }

    fn row(&self, index: usize, option: &TopUpOption, cx: &mut Context<Self>) -> SettingsButton {
        let stars = tr_plural_decimal(
            cx,
            key::lng_credits_summary_options_credits,
            option.star_count,
        );
        let price = crate::currency::format(option.amount, &option.currency);
        let weak = cx.weak_entity();
        let chosen = option.clone();
        SettingsButton::new(
            ("stars-option", index),
            &CREDITS_TOPUP_BUTTON,
            format!("{STAR} {stars}"),
        )
        .label(price)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.buy(&chosen, window, cx));
        })
    }
}

impl Render for StarsBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let options = self.options.clone().unwrap_or_default();
        let has_more = !self.is_extended && options.iter().any(|option| option.is_extended);
        let shown: Vec<_> = options
            .iter()
            .filter(|option| self.is_extended || !option.is_extended)
            .enumerate()
            .map(|(index, option)| self.row(index, option, cx))
            .collect();
        let weak = cx.weak_entity();
        let more = has_more.then(|| {
            rows::plain("stars-more", tr(cx, key::lng_credits_summary_options_more)).on_click(
                move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.is_extended = true;
                        cx.notify();
                    });
                },
            )
        });
        let content = div()
            .flex()
            .flex_col()
            .children(shown)
            .children(more)
            .child(rows::skip());
        let layers = self.env.targets.layers.clone();
        div().track_focus(&self.focus).child(
            BoxFrame::new("stars-box", &DEFAULT_BOX)
                .title(tr(cx, key::lng_credits_summary_options_subtitle))
                .row(content)
                .button(
                    RoundButton::new("stars-close", &DEFAULT_BOX.button, tr(cx, key::lng_close))
                        .on_click(move |_, window, cx| {
                            layers.update(cx, |layers, cx| layers.close(window, cx));
                        }),
                ),
        )
    }
}
