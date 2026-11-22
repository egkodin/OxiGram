//! How the intro shows sign-in failures, and the boxes it opens for them
//! (`intro_*.cpp`, `phone_banned_box.cpp`, `Widget::resetAccount`).

use std::rc::Rc;
use std::time::Duration;

use gpui::{App, Entity, SharedString, Window};
use i18n::key;
use model::auth;
use style::layers::ATTENTION_BOX_BUTTON;
use ui::Markup;

use super::step::Shared;
use crate::boxes::{self, ConfirmArgs};
use crate::lang::{tr, tr_plural, tr_with};
use crate::layers::Layers;

/// The text a step shows under its fields for `error`.
pub(crate) fn text(error: &auth::Error, cx: &App) -> SharedString {
    let key = match error {
        auth::Error::Flood { .. } => key::lng_flood_error,
        auth::Error::PhoneNumberInvalid => key::lng_bad_phone,
        auth::Error::PhoneNumberFlood => key::lng_error_phone_flood,
        auth::Error::CodeInvalid => key::lng_bad_code,
        auth::Error::PasswordInvalid => key::lng_signin_bad_password,
        auth::Error::FirstNameInvalid | auth::Error::LastNameInvalid => key::lng_bad_name,
        auth::Error::EmailInvalid => key::lng_cloud_password_bad_email,
        auth::Error::EmailNotAllowed => key::lng_settings_error_email_not_alowed,
        auth::Error::ResetCanceled => key::lng_signin_reset_cancelled,
        // tdesktop shows the server's error type as is.
        auth::Error::Server { message, .. } => return message.clone().into(),
        other => return other.to_string().into(),
    };
    tr(cx, key)
}

/// `ShowPhoneBannedError`: OK, or Help, which writes to Telegram's login
/// support.
pub(crate) fn phone_banned(
    layers: &Entity<Layers>,
    phone: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let phone = phone.to_owned();
    let mut args = ConfirmArgs::new(Markup::plain(
        tr(cx, key::lng_signin_banned_text).to_string(),
    ));
    args.confirm_text = Some(tr(cx, key::lng_box_ok));
    args.cancel_text = Some(tr(cx, key::lng_signin_banned_help));
    args.cancelled = Some(Rc::new(move |_, cx| cx.open_url(&banned_help_url(&phone))));
    args.strict_cancel = true;
    boxes::confirm(layers, args, window, cx);
}

fn banned_help_url(phone: &str) -> String {
    let subject = format!("Banned phone number: {phone}");
    let body = format!(
        "I'm trying to use my mobile phone number: {phone}\nBut Telegram says it's banned. \
         Please help.\n\nApp version: OxiGram {}\nOS version: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
    );
    format!(
        "mailto:?to={}&subject={}&body={}",
        url_encode("login@stel.com"),
        url_encode(&subject),
        url_encode(&body)
    )
}

/// Percent-encodes everything but RFC 3986's unreserved characters.
fn url_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// TDLib's `authorizationStateWaitPremiumPurchase`: desktop clients cannot
/// sell Premium, so the box only explains where to buy it.
pub(crate) fn premium_required(
    layers: &Entity<Layers>,
    support_email: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let text = tr_with(
        cx,
        key::oxigram_intro_premium_required,
        &[("email", support_email)],
    );
    boxes::inform(layers, Markup::plain(text), None, window, cx);
}

/// `Widget::resetAccount`: confirms, then deletes the account so the number
/// can sign up again.
pub(crate) fn reset_account(shared: &Shared, window: &mut Window, cx: &mut App) {
    let layers = shared.layers.clone();
    let shared = shared.clone();
    let mut args = ConfirmArgs::new(Markup::plain(
        tr(cx, key::lng_signin_sure_reset).to_string(),
    ));
    args.confirm_text = Some(tr(cx, key::lng_signin_reset));
    args.confirm_style = &ATTENTION_BOX_BUTTON;
    args.cancel_text = Some(tr(cx, key::lng_cancel));
    args.confirmed = Some(Rc::new(move |window, cx| {
        let task = shared.store.read(cx).auth(auth::Action::ResetAccount, cx);
        let shared = shared.clone();
        window
            .spawn(cx, async move |cx| {
                let Err(error) = task.await else {
                    return;
                };
                cx.update(|window, cx| reset_failed(&shared, &error, window, cx))
                    .ok();
            })
            .detach();
    }));
    boxes::confirm(&layers, args, window, cx);
}

fn reset_failed(shared: &Shared, error: &auth::Error, window: &mut Window, cx: &mut App) {
    let text = match error {
        auth::Error::ResetWait { wait } => {
            let phone = shared.data.read(cx).phone.expose().to_owned();
            let when = reset_wait_text(*wait, cx);
            tr_with(
                cx,
                key::lng_signin_reset_wait,
                &[("phone_number", &phone), ("when", &when)],
            )
        }
        other => text(other, cx).to_string(),
    };
    boxes::inform(&shared.layers, Markup::plain(text), None, window, cx);
}

/// "6 days 23 hours 59 minutes", rounded up to the minute as tdesktop does.
fn reset_wait_text(wait: Duration, cx: &App) -> String {
    let seconds = i64::try_from(wait.as_secs())
        .unwrap_or(i64::MAX)
        .saturating_add(59);
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let minutes_text = tr_plural(cx, key::lng_minutes, minutes, &[]);
    if days > 0 {
        let days_text = tr_plural(cx, key::lng_days, days, &[]);
        let hours_text = tr_plural(cx, key::lng_hours, hours, &[]);
        tr_with(
            cx,
            key::lng_signin_reset_in_days,
            &[
                ("days_count", &days_text),
                ("hours_count", &hours_text),
                ("minutes_count", &minutes_text),
            ],
        )
    } else if hours > 0 {
        let hours_text = tr_plural(cx, key::lng_hours, hours, &[]);
        tr_with(
            cx,
            key::lng_signin_reset_in_hours,
            &[
                ("hours_count", &hours_text),
                ("minutes_count", &minutes_text),
            ],
        )
    } else {
        minutes_text
    }
}

/// A duration as days, hours or minutes, for the email reset texts.
pub(crate) fn duration_text(duration: Duration, cx: &App) -> String {
    reset_wait_text(duration, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_mail_is_url_encoded() {
        let url = banned_help_url("+1 555");
        assert!(url.starts_with(
            "mailto:?to=login%40stel.com&subject=Banned%20phone%20number%3A%20%2B1%20555"
        ));
        assert!(!url.contains(' '));
        assert!(!url.contains('\n'));
    }
}
