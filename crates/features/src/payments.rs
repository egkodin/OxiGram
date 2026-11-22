//! Where oxigram's buy buttons stop until checkout lands (M13): an invoice
//! or a Stars package gets its payment form from TDLib and nothing is
//! paid; a bot link is not followed, as starting the bot sends it a
//! message.
// shortcut: no checkout panel (`Payments::CheckoutProcess`); M13 shows the form TDLib returns here.

use gpui::{App, Window};
use i18n::key;
use model::premium::PaymentLink;
use style::widgets::DEFAULT_TOAST;

use crate::lang::tr;
use crate::links::Targets;

pub(crate) fn open(link: PaymentLink, targets: &Targets, window: &mut Window, cx: &mut App) {
    let Some(form) = targets.store.read(cx).session().open_payment_form(link) else {
        later(targets, cx);
        return;
    };
    let task = crate::store::spawn(form, cx);
    let targets = targets.clone();
    window
        .spawn(cx, async move |cx| {
            let result = task.await;
            let _ = cx.update(|_, cx| match result {
                Ok(()) => later(&targets, cx),
                Err(error) => {
                    tracing::warn!(%error, "reading the payment form failed");
                    toast(&targets, error.user_message(), cx);
                }
            });
        })
        .detach();
}

fn later(targets: &Targets, cx: &mut App) {
    let text = tr(cx, key::oxigram_payment_form_later).to_string();
    toast(targets, text, cx);
}

pub(crate) fn toast(targets: &Targets, text: String, cx: &mut App) {
    targets.layers.update(cx, |layers, cx| {
        layers.show_toast(
            text.into(),
            &DEFAULT_TOAST,
            crate::toast::DEFAULT_DURATION,
            cx,
        );
    });
}
