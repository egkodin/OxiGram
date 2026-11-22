//! "Send a Gift" (`ChooseStarGiftRecipient`, `ShowStarGiftBox`): a contact
//! to gift, then the Premium periods with their prices; a period goes on to
//! the payment form, where oxigram stops.
// shortcut: no Star gifts (they are paid from the balance at once, with no form to stop at), no cover with the userpic and mini stars, no gift stickers or discount badges on the periods, no birthdays on top of the chooser; with M13.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Task, Window, div,
};
use i18n::key;
use model::UserId;
use model::premium::GiftOption;
use style::credits::CREDITS_TOPUP_BUTTON;
use style::layers::{BOX_ROW_PADDING, DEFAULT_BOX};
use style::widgets::BOX_DIVIDER_LABEL;
use ui::{BoxFrame, FlatLabel, Markup, RoundButton, SettingsButton};

use super::{Env, PageId, rows};
use crate::lang::{tr, tr_plural};
use crate::px_of;

pub(super) fn show(env: &Env, window: &mut Window, cx: &mut App) {
    let chosen = env.clone();
    crate::contacts_box::show_above(
        &env.targets.store,
        &env.targets.layers,
        tr(cx, key::lng_gift_premium_or_stars),
        Rc::new(move |user, window, cx| open(&chosen, user, window, cx)),
        window,
        cx,
    );
}

/// `lng_gift_premium_about` with the name in bold and "See Features" as
/// its link.
fn about(template: &str, name: &str, features: &str) -> Markup {
    let template = template.replace("{name}", &format!("**{name}**"));
    match template.split_once("{features}") {
        Some((before, after)) => Markup::rich(before).link(features).text(after),
        None => Markup::rich(&template),
    }
}

struct GiftBox {
    env: Env,
    user: UserId,
    /// None while TDLib is asked.
    options: Option<Vec<GiftOption>>,
    focus: FocusHandle,
    _load: Task<()>,
}

impl Focusable for GiftBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn open(env: &Env, user: UserId, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx: &mut Context<GiftBox>| {
        let task = crate::store::spawn(
            env.targets.store.read(cx).session().premium_gift_options(),
            cx,
        );
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(options) => {
                let _ = this.update(cx, |this, cx| {
                    this.options = Some(options);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the Premium gift options failed"),
        });
        GiftBox {
            env: env.clone(),
            user,
            options: None,
            focus: cx.focus_handle(),
            _load: load,
        }
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl GiftBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn buy(&self, option: &GiftOption, window: &mut Window, cx: &mut App) {
        self.close(window, cx);
        crate::payments::open(option.link(self.user), &self.env.targets, window, cx);
    }

    fn row(&self, index: usize, option: &GiftOption, cx: &mut Context<Self>) -> SettingsButton {
        let months = tr_plural(cx, key::lng_months, i64::from(option.months), &[]);
        let price = crate::currency::format(option.amount, &option.currency);
        let weak = cx.weak_entity();
        let chosen = option.clone();
        SettingsButton::new(("gift-option", index), &CREDITS_TOPUP_BUTTON, months)
            .label(price)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.buy(&chosen, window, cx));
            })
    }

    fn about(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let name = self
            .env
            .targets
            .store
            .read(cx)
            .state()
            .chats
            .user(self.user)
            .map(|user| user.first_name.clone())
            .unwrap_or_default();
        let markup = about(
            &tr(cx, key::lng_gift_premium_about),
            &name,
            &tr(cx, key::lng_gift_premium_features),
        );
        let weak = cx.weak_entity();
        div().px(px_of(BOX_ROW_PADDING.left)).child(
            FlatLabel::new("gift-about", &BOX_DIVIDER_LABEL, markup).on_link(
                move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.close(window, cx);
                        this.env.open(PageId::Premium, window, cx);
                    });
                },
            ),
        )
    }
}

impl Render for GiftBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let options = self.options.clone().unwrap_or_default();
        let shown: Vec<_> = options
            .iter()
            .enumerate()
            .map(|(index, option)| self.row(index, option, cx))
            .collect();
        let content = div()
            .flex()
            .flex_col()
            .child(self.about(cx))
            .child(rows::skip())
            .children(shown)
            .child(rows::skip());
        let weak = cx.weak_entity();
        div().track_focus(&self.focus).child(
            BoxFrame::new("gift-box", &DEFAULT_BOX)
                .title(tr(cx, key::lng_gift_premium_subtitle))
                .row(content)
                .button(
                    RoundButton::new("gift-close", &DEFAULT_BOX.button, tr(cx, key::lng_close))
                        .on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |this, cx| this.close(window, cx));
                        }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_is_bold_and_the_features_a_link() {
        let markup = about("Give {name} Premium. {features}", "Ann", "See Features >");
        assert_eq!(markup.as_str(), "Give Ann Premium. See Features >");
    }
}
