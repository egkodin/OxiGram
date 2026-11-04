//! "Clear payment info" (`ClearPaymentInfoBoxBuilder`): what to forget,
//! then a second question before it goes.

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Window, div,
};
use i18n::key;
use style::layers::{
    ATTENTION_BOX_BUTTON, BOX_LABEL, BOX_ROW_PADDING, DEFAULT_BOX, DEFAULT_BOX_CHECKBOX,
};
use style::widgets::DEFAULT_CHECK;
use ui::{BoxFrame, Checkbox, Checked, FlatLabel, Markup, RoundButton};

use super::Env;
use crate::lang::tr;
use crate::px_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Choose,
    /// "Are you sure?" with the choices made.
    Confirm,
}

pub(super) struct PaymentBox {
    env: Env,
    step: Step,
    shipping: Checked,
    payment: Checked,
    focus: FocusHandle,
}

impl Focusable for PaymentBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(env: &Env, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| PaymentBox {
        env: env.clone(),
        step: Step::Choose,
        shipping: Checked::Yes,
        payment: Checked::Yes,
        focus: cx.focus_handle(),
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

fn flip(checked: &mut Checked) {
    *checked = if *checked == Checked::Yes {
        Checked::No
    } else {
        Checked::Yes
    };
}

impl PaymentBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.step == Step::Choose {
            self.step = Step::Confirm;
            cx.notify();
            return;
        }
        let session = self.env.targets.store.read(cx).session().clone();
        if self.shipping == Checked::Yes {
            crate::peer_menu::send(
                session.delete_saved_order_info(),
                "clearing shipping info",
                cx,
            );
        }
        if self.payment == Checked::Yes {
            crate::peer_menu::send(
                session.delete_saved_credentials(),
                "clearing payment info",
                cx,
            );
        }
        self.close(window, cx);
    }

    fn checkbox(&self, is_shipping: bool, cx: &mut Context<Self>) -> gpui::Div {
        let (id, text, checked) = if is_shipping {
            (
                "payment-shipping",
                key::lng_clear_payment_info_shipping,
                self.shipping,
            )
        } else {
            (
                "payment-payment",
                key::lng_clear_payment_info_payment,
                self.payment,
            )
        };
        let weak = cx.weak_entity();
        div()
            .pl(px_of(BOX_ROW_PADDING.left))
            .pr(px_of(BOX_ROW_PADDING.right))
            .pt(px_of(BOX_ROW_PADDING.left))
            .child(
                Checkbox::new(
                    id,
                    &DEFAULT_BOX_CHECKBOX,
                    &DEFAULT_CHECK,
                    tr(cx, text),
                    checked,
                )
                .on_toggle(move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        flip(if is_shipping {
                            &mut this.shipping
                        } else {
                            &mut this.payment
                        });
                        cx.notify();
                    });
                }),
            )
    }
}

impl Render for PaymentBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = match self.step {
            Step::Choose => key::lng_clear_payment_info_sure,
            Step::Confirm => key::lng_clear_payment_info_confirm,
        };
        let mut content = div()
            .flex()
            .flex_col()
            .px(px_of(BOX_ROW_PADDING.left))
            .child(FlatLabel::new(
                "payment-text",
                &BOX_LABEL,
                Markup::plain(tr(cx, text).to_string()),
            ));
        if self.step == Step::Choose {
            content = div()
                .flex()
                .flex_col()
                .child(content)
                .child(self.checkbox(true, cx))
                .child(self.checkbox(false, cx));
        }
        let nothing_chosen = self.shipping == Checked::No && self.payment == Checked::No;
        let (clear, cancel) = (cx.weak_entity(), cx.weak_entity());
        div().track_focus(&self.focus).child(
            BoxFrame::new("payment-box", &DEFAULT_BOX)
                .title(tr(cx, key::lng_clear_payment_info_title))
                .row(content.pb(px_of(BOX_ROW_PADDING.left)))
                .button(
                    RoundButton::new(
                        "payment-clear",
                        &ATTENTION_BOX_BUTTON,
                        tr(cx, key::lng_clear_payment_info_clear),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = clear.update(cx, |this, cx| {
                            if nothing_chosen {
                                this.close(window, cx);
                            } else {
                                this.clear(window, cx);
                            }
                        });
                    }),
                )
                .button(
                    RoundButton::new(
                        "payment-cancel",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_cancel),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                    }),
                ),
        )
    }
}
