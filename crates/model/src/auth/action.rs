use std::future::Future;

use futures::FutureExt as _;
use futures::future::BoxFuture;
use td_api::enums::{EmailAddressAuthentication, ResendCodeReason};
use td_api::functions::{
    AcceptTermsOfService, CheckAuthenticationCode, CheckAuthenticationEmailCode,
    CheckAuthenticationPassword, CheckAuthenticationPasswordRecoveryCode, DeleteAccount, LogOut,
    RecoverAuthenticationPassword, RegisterUser, RequestAuthenticationPasswordRecovery,
    RequestQrCodeAuthentication, ResendAuthenticationCode, ResetAuthenticationEmailAddress,
    SetAuthenticationEmailAddress, SetAuthenticationPhoneNumber,
};
use td_api::types::{
    EmailAddressAuthenticationCode, PhoneNumberAuthenticationSettings, ResendCodeReasonUserRequest,
};
use tg::TdError;

use super::Error;
use crate::secret::Secret;

/// The reason tdesktop sends with an account reset (`intro_widget.cpp`).
const RESET_REASON: &str = "Forgot password";
/// The reason tdesktop sends when the user declines updated terms
/// (`Session::termsDeleteNow`).
const DECLINE_TERMS_REASON: &str = "Decline ToS update";

/// A sign-in step the user takes. Each one is a TDLib request; its outcome
/// arrives as the next authorization state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Sends a login code to the number, in international format.
    SetPhoneNumber(Secret),
    /// Sends the code again the next way (`Code::next_delivery`).
    ResendCode,
    CheckCode(Secret),
    /// Switches to QR code login.
    RequestQrCode,
    CheckPassword(Secret),
    /// Sends a password recovery code to the recovery email.
    RequestPasswordRecovery,
    /// Checks a recovery code without using it.
    CheckRecoveryCode(Secret),
    /// Signs in with a recovery code, replacing the password with
    /// `new_password` or removing it when that is empty.
    RecoverPassword {
        code: Secret,
        new_password: Secret,
        hint: String,
    },
    /// Deletes the account so the number can sign up again; for a forgotten
    /// password.
    ResetAccount,
    Register(Registration),
    SetEmail(Secret),
    CheckEmailCode(Secret),
    /// Replaces the login email with a code to the phone number.
    ResetEmail,
    AcceptTerms(TermsId),
    /// Deletes the account of a user who declined updated terms.
    DeclineTerms,
    LogOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub first_name: String,
    pub last_name: String,
}

/// Identifies a terms of service update.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TermsId(pub String);

impl Action {
    /// Sends the request now; the future reports whether TDLib accepted it.
    pub(crate) fn send(self, client: &tg::Client) -> BoxFuture<'static, Result<(), Error>> {
        match self {
            Self::SetPhoneNumber(number) => ok(client.request(&SetAuthenticationPhoneNumber {
                phone_number: number.expose().to_owned(),
                // tdesktop's codeSettings: no flash or missed calls, no tokens.
                settings: Some(Box::new(PhoneNumberAuthenticationSettings {
                    allow_flash_call: false,
                    allow_missed_call: false,
                    is_current_phone_number: false,
                    has_unknown_phone_number: false,
                    allow_sms_retriever_api: false,
                    firebase_authentication_settings: None,
                    authentication_tokens: Vec::new(),
                })),
            })),
            Self::ResendCode => ok(client.request(&ResendAuthenticationCode {
                reason: Some(Box::new(ResendCodeReason::UserRequest(
                    ResendCodeReasonUserRequest {},
                ))),
            })),
            Self::CheckCode(code) => ok(client.request(&CheckAuthenticationCode {
                code: code.expose().to_owned(),
            })),
            Self::RequestQrCode => ok(client.request(&RequestQrCodeAuthentication {
                other_user_ids: Vec::new(),
            })),
            Self::CheckPassword(password) => ok(client.request(&CheckAuthenticationPassword {
                password: password.expose().to_owned(),
            })),
            Self::RequestPasswordRecovery => {
                ok(client.request(&RequestAuthenticationPasswordRecovery {}))
            }
            Self::CheckRecoveryCode(code) => {
                ok(client.request(&CheckAuthenticationPasswordRecoveryCode {
                    recovery_code: code.expose().to_owned(),
                }))
            }
            Self::RecoverPassword {
                code,
                new_password,
                hint,
            } => ok(client.request(&RecoverAuthenticationPassword {
                recovery_code: code.expose().to_owned(),
                new_password: new_password.expose().to_owned(),
                new_hint: hint,
            })),
            Self::ResetAccount => ok(client.request(&DeleteAccount {
                reason: RESET_REASON.to_owned(),
                password: String::new(),
            })),
            Self::Register(registration) => ok(client.request(&RegisterUser {
                first_name: registration.first_name,
                last_name: registration.last_name,
                disable_notification: false,
            })),
            Self::SetEmail(email) => ok(client.request(&SetAuthenticationEmailAddress {
                email_address: email.expose().to_owned(),
            })),
            Self::CheckEmailCode(code) => ok(client.request(&CheckAuthenticationEmailCode {
                code: Box::new(EmailAddressAuthentication::Code(
                    EmailAddressAuthenticationCode {
                        code: code.expose().to_owned(),
                    },
                )),
            })),
            Self::ResetEmail => ok(client.request(&ResetAuthenticationEmailAddress {})),
            Self::AcceptTerms(id) => ok(client.request(&AcceptTermsOfService {
                terms_of_service_id: id.0,
            })),
            Self::DeclineTerms => ok(client.request(&DeleteAccount {
                reason: DECLINE_TERMS_REASON.to_owned(),
                password: String::new(),
            })),
            Self::LogOut => ok(client.request(&LogOut {})),
        }
    }
}

fn ok<T>(
    response: impl Future<Output = Result<T, TdError>> + Send + 'static,
) -> BoxFuture<'static, Result<(), Error>> {
    response
        .map(|result| result.map(drop).map_err(Error::from))
        .boxed()
}
