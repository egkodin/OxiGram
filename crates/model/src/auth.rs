//! Sign-in: where TDLib's authorization stands, what the user can do next, and
//! why a step failed.

mod action;
mod error;

use std::time::Duration;

use td_api::enums::{AuthenticationCodeType, AuthorizationState, EmailAddressResetState};
use td_api::types::AuthenticationCodeInfo;

use crate::secret::Secret;
use crate::text::FormattedText;

pub use action::{Action, Registration, TermsId};
pub use error::Error;

/// TDLib's authorization state. Every change arrives as an event; the screens
/// follow it rather than the responses to their requests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum State {
    /// TDLib has not reported a state yet.
    #[default]
    Unknown,
    /// TDLib waits for its parameters; the app sends them.
    WaitParameters,
    WaitPhoneNumber,
    /// The server requires an in-store Premium purchase, which desktop clients
    /// cannot make.
    WaitPremiumPurchase {
        support_email: String,
    },
    /// The server asks for a login email before sending a code.
    WaitEmailAddress,
    WaitEmailCode(EmailCode),
    WaitCode(Code),
    /// A QR code login is waiting for another device to scan `link`.
    WaitQrScan {
        link: Secret,
    },
    WaitRegistration {
        terms: Terms,
    },
    WaitPassword(Password),
    Ready,
    LoggingOut,
    Closing,
    Closed,
}

/// A login code that was sent, and how the next one would be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    pub phone_number: Secret,
    pub delivery: Delivery,
    pub next_delivery: Option<Delivery>,
    /// How long until the code can be sent again with `next_delivery`.
    pub resend_after: Duration,
}

/// How a login code reaches the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    /// A message in Telegram on the user's other devices.
    Telegram {
        length: usize,
    },
    Sms {
        length: usize,
    },
    /// An SMS with a word; the code is the word starting with `first_letter`.
    SmsWord {
        first_letter: String,
    },
    /// An SMS with a phrase starting with `first_word`.
    SmsPhrase {
        first_word: String,
    },
    Call {
        length: usize,
    },
    /// A call whose caller number matches `pattern`.
    FlashCall {
        pattern: String,
    },
    /// A missed call; the code is the last digits of the caller's number.
    MissedCall {
        number_prefix: String,
        length: usize,
    },
    /// The Anonymous Numbers section on Fragment.
    Fragment {
        url: String,
        length: usize,
    },
    /// Firebase verification on a phone; desktop clients never receive it, as
    /// they do not offer Firebase settings.
    Firebase {
        length: usize,
    },
}

impl Delivery {
    /// The number of characters that completes the code, if it is fixed.
    #[must_use]
    pub fn code_length(&self) -> Option<usize> {
        match self {
            Self::Telegram { length }
            | Self::Sms { length }
            | Self::Call { length }
            | Self::MissedCall { length, .. }
            | Self::Fragment { length, .. }
            | Self::Firebase { length } => Some(*length).filter(|length| *length > 0),
            Self::SmsWord { .. } | Self::SmsPhrase { .. } | Self::FlashCall { .. } => None,
        }
    }
}

/// A code sent to the login email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailCode {
    /// The address with most of it masked, as the server shows it.
    pub email_pattern: String,
    pub length: usize,
    pub reset: Option<EmailReset>,
}

/// Whether the login email can be replaced by a code to the phone number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailReset {
    /// A reset can be requested; it takes effect after `wait`.
    Available { wait: Duration },
    /// A reset was requested and completes after `remaining`.
    Pending { remaining: Duration },
}

/// The cloud password (2FA) check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Password {
    pub hint: String,
    pub recovery: Recovery,
    /// Recovering the password by email deletes Telegram Passport data.
    pub has_passport_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    /// No recovery email; only an account reset can replace the password.
    None,
    /// A code can be sent to the email matching `pattern`.
    Email { pattern: String },
}

/// Terms of service to accept on sign-up or after an update.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Terms {
    pub text: FormattedText,
    /// The minimum age, or 0 when there is none.
    pub min_age: i32,
    /// Show the terms in a box the user must accept.
    pub show_popup: bool,
}

impl State {
    /// True once sign-in has finished.
    #[must_use]
    pub fn is_signed_in(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl From<AuthorizationState> for State {
    fn from(state: AuthorizationState) -> Self {
        match state {
            AuthorizationState::WaitTdlibParameters(_) => Self::WaitParameters,
            AuthorizationState::WaitPhoneNumber(_) => Self::WaitPhoneNumber,
            AuthorizationState::WaitPremiumPurchase(wait) => Self::WaitPremiumPurchase {
                support_email: wait.support_email_address,
            },
            AuthorizationState::WaitEmailAddress(_) => Self::WaitEmailAddress,
            AuthorizationState::WaitEmailCode(wait) => Self::WaitEmailCode(EmailCode {
                email_pattern: wait.code_info.email_address_pattern,
                length: count(wait.code_info.length),
                reset: wait.email_address_reset_state.map(|reset| match *reset {
                    EmailAddressResetState::Available(available) => EmailReset::Available {
                        wait: seconds(available.wait_period),
                    },
                    EmailAddressResetState::Pending(pending) => EmailReset::Pending {
                        remaining: seconds(pending.reset_in),
                    },
                }),
            }),
            AuthorizationState::WaitCode(wait) => Self::WaitCode(Code::from(*wait.code_info)),
            AuthorizationState::WaitOtherDeviceConfirmation(wait) => Self::WaitQrScan {
                link: Secret::new(wait.link),
            },
            AuthorizationState::WaitRegistration(wait) => Self::WaitRegistration {
                terms: Terms::from(*wait.terms_of_service),
            },
            AuthorizationState::WaitPassword(wait) => Self::WaitPassword(Password {
                hint: wait.password_hint,
                recovery: if wait.has_recovery_email_address {
                    Recovery::Email {
                        pattern: wait.recovery_email_address_pattern,
                    }
                } else {
                    Recovery::None
                },
                has_passport_data: wait.has_passport_data,
            }),
            AuthorizationState::Ready(_) => Self::Ready,
            AuthorizationState::LoggingOut(_) => Self::LoggingOut,
            AuthorizationState::Closing(_) => Self::Closing,
            AuthorizationState::Closed(_) => Self::Closed,
        }
    }
}

impl From<AuthenticationCodeInfo> for Code {
    fn from(info: AuthenticationCodeInfo) -> Self {
        Self {
            phone_number: Secret::new(info.phone_number),
            delivery: Delivery::from(*info.r#type),
            next_delivery: info.next_type.map(|next| Delivery::from(*next)),
            resend_after: seconds(info.timeout),
        }
    }
}

impl From<AuthenticationCodeType> for Delivery {
    fn from(kind: AuthenticationCodeType) -> Self {
        match kind {
            AuthenticationCodeType::TelegramMessage(code) => Self::Telegram {
                length: count(code.length),
            },
            AuthenticationCodeType::Sms(code) => Self::Sms {
                length: count(code.length),
            },
            AuthenticationCodeType::SmsWord(code) => Self::SmsWord {
                first_letter: code.first_letter,
            },
            AuthenticationCodeType::SmsPhrase(code) => Self::SmsPhrase {
                first_word: code.first_word,
            },
            AuthenticationCodeType::Call(code) => Self::Call {
                length: count(code.length),
            },
            AuthenticationCodeType::FlashCall(code) => Self::FlashCall {
                pattern: code.pattern,
            },
            AuthenticationCodeType::MissedCall(code) => Self::MissedCall {
                number_prefix: code.phone_number_prefix,
                length: count(code.length),
            },
            AuthenticationCodeType::Fragment(code) => Self::Fragment {
                url: code.url,
                length: count(code.length),
            },
            AuthenticationCodeType::FirebaseAndroid(code) => Self::Firebase {
                length: count(code.length),
            },
            AuthenticationCodeType::FirebaseIos(code) => Self::Firebase {
                length: count(code.length),
            },
        }
    }
}

impl From<td_api::types::TermsOfService> for Terms {
    fn from(terms: td_api::types::TermsOfService) -> Self {
        Self {
            text: FormattedText::from(*terms.text),
            min_age: terms.min_user_age,
            show_popup: terms.show_popup,
        }
    }
}

/// TDLib's non-negative `int32` counts; a negative one is treated as 0.
fn count(value: i32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

fn seconds(value: i32) -> Duration {
    Duration::from_secs(u64::try_from(value).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(json: &str) -> State {
        let state: AuthorizationState =
            serde_json::from_str(json).expect("fixture is a valid authorizationState");
        State::from(state)
    }

    #[test]
    fn converts_code_states() {
        let state = state(
            r#"{"@type":"authorizationStateWaitCode","code_info":{
                "@type":"authenticationCodeInfo","phone_number":"+15551234567",
                "type":{"@type":"authenticationCodeTypeTelegramMessage","length":5},
                "next_type":{"@type":"authenticationCodeTypeSms","length":5},
                "timeout":60}}"#,
        );
        let State::WaitCode(code) = state else {
            panic!("expected WaitCode, got {state:?}");
        };
        assert_eq!(code.delivery, Delivery::Telegram { length: 5 });
        assert_eq!(code.next_delivery, Some(Delivery::Sms { length: 5 }));
        assert_eq!(code.resend_after, Duration::from_secs(60));
        assert_eq!(code.delivery.code_length(), Some(5));
        assert!(!format!("{code:?}").contains("5551234567"));
    }

    #[test]
    fn converts_password_and_email_states() {
        let password = state(
            r#"{"@type":"authorizationStateWaitPassword","password_hint":"cat",
                "has_recovery_email_address":true,"has_passport_data":false,
                "recovery_email_address_pattern":"a***@e***.com"}"#,
        );
        assert_eq!(
            password,
            State::WaitPassword(Password {
                hint: "cat".to_owned(),
                recovery: Recovery::Email {
                    pattern: "a***@e***.com".to_owned()
                },
                has_passport_data: false,
            })
        );
        let email = state(
            r#"{"@type":"authorizationStateWaitEmailCode","allow_apple_id":false,
                "allow_google_id":false,"code_info":{"@type":"emailAddressAuthenticationCodeInfo",
                "email_address_pattern":"a***@e***.com","length":6},
                "email_address_reset_state":{"@type":"emailAddressResetStateAvailable","wait_period":604800}}"#,
        );
        assert_eq!(
            email,
            State::WaitEmailCode(EmailCode {
                email_pattern: "a***@e***.com".to_owned(),
                length: 6,
                reset: Some(EmailReset::Available {
                    wait: Duration::from_secs(604_800)
                }),
            })
        );
    }

    #[test]
    fn qr_links_are_secret() {
        let state = state(
            r#"{"@type":"authorizationStateWaitOtherDeviceConfirmation","link":"tg://login?token=abc"}"#,
        );
        assert!(!format!("{state:?}").contains("token"));
    }

    #[test]
    fn word_codes_have_no_fixed_length() {
        let delivery = Delivery::SmsWord {
            first_letter: "a".to_owned(),
        };
        assert_eq!(delivery.code_length(), None);
        assert_eq!(Delivery::Sms { length: 0 }.code_length(), None);
    }
}
