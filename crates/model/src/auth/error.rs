use std::time::Duration;

use tg::TdError;

const FLOOD_PREFIX: &str = "Too Many Requests: retry after ";
const RESET_WAIT_PREFIX: &str = "2FA_CONFIRM_WAIT_";

/// Why a sign-in step failed, sorted the way tdesktop's intro screens react
/// to server errors (`intro_*.cpp`).
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// FLOOD_WAIT: too many attempts.
    #[error("too many attempts, retry after {}s", retry_after.as_secs())]
    Flood { retry_after: Duration },
    #[error("invalid phone number")]
    PhoneNumberInvalid,
    #[error("the phone number is banned")]
    PhoneNumberBanned,
    /// The account was deleted and re-created too often.
    #[error("the phone number signed up too often")]
    PhoneNumberFlood,
    /// The phone number is taken; the sign-up has to start over.
    #[error("the phone number is already registered")]
    PhoneNumberOccupied,
    #[error("invalid code")]
    CodeInvalid,
    /// The sign-in has to start over from the phone number.
    #[error("the code expired")]
    CodeExpired,
    #[error("wrong password")]
    PasswordInvalid,
    #[error("invalid first name")]
    FirstNameInvalid,
    #[error("invalid last name")]
    LastNameInvalid,
    #[error("invalid email address")]
    EmailInvalid,
    #[error("this email address is not allowed")]
    EmailNotAllowed,
    /// The recovery email cannot receive a code.
    #[error("password recovery is unavailable")]
    RecoveryUnavailable,
    #[error("the recovery code expired")]
    RecoveryExpired,
    /// The account is protected by a password and can be reset after `wait`.
    #[error("the account can be reset in {}s", wait.as_secs())]
    ResetWait { wait: Duration },
    /// The account's owner recently canceled a reset.
    #[error("the account reset was canceled by its owner")]
    ResetCanceled,
    /// The server cannot send the code another way.
    #[error("no other way to send the code")]
    SendCodeUnavailable,
    #[error("the TDLib client is closed")]
    Closed,
    /// Any other error, shown as the server's text, as tdesktop does.
    #[error("error {code}")]
    Server { code: i32, message: String },
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl From<TdError> for Error {
    fn from(error: TdError) -> Self {
        match error {
            TdError::Td { code, message } => Self::from_server(code, message),
            TdError::Closed => Self::Closed,
            other => Self::Server {
                code: 0,
                message: other.to_string(),
            },
        }
    }
}

impl Error {
    fn from_server(code: i32, message: String) -> Self {
        if let Some(seconds) = message.strip_prefix(FLOOD_PREFIX)
            && let Ok(seconds) = seconds.parse::<u64>()
        {
            return Self::Flood {
                retry_after: Duration::from_secs(seconds),
            };
        }
        if let Some(seconds) = message.strip_prefix(RESET_WAIT_PREFIX)
            && let Ok(seconds) = seconds.parse::<u64>()
        {
            return Self::ResetWait {
                wait: Duration::from_secs(seconds),
            };
        }
        match message.as_str() {
            "PHONE_NUMBER_INVALID" => Self::PhoneNumberInvalid,
            "PHONE_NUMBER_BANNED" => Self::PhoneNumberBanned,
            "PHONE_NUMBER_FLOOD" => Self::PhoneNumberFlood,
            "PHONE_NUMBER_OCCUPIED" => Self::PhoneNumberOccupied,
            "PHONE_CODE_EMPTY" | "PHONE_CODE_INVALID" | "CODE_INVALID" | "EMAIL_CODE_INVALID" => {
                Self::CodeInvalid
            }
            "PHONE_CODE_EXPIRED" => Self::CodeExpired,
            "PASSWORD_HASH_INVALID" | "SRP_PASSWORD_CHANGED" => Self::PasswordInvalid,
            "FIRSTNAME_INVALID" => Self::FirstNameInvalid,
            "LASTNAME_INVALID" => Self::LastNameInvalid,
            "EMAIL_INVALID" => Self::EmailInvalid,
            "EMAIL_NOT_ALLOWED" => Self::EmailNotAllowed,
            "PASSWORD_RECOVERY_NA" => Self::RecoveryUnavailable,
            "PASSWORD_RECOVERY_EXPIRED" => Self::RecoveryExpired,
            "2FA_RECENT_CONFIRM" => Self::ResetCanceled,
            "SEND_CODE_UNAVAILABLE" => Self::SendCodeUnavailable,
            _ => Self::Server { code, message },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn td(code: i32, message: &str) -> Error {
        Error::from(TdError::Td {
            code,
            message: message.to_owned(),
        })
    }

    #[test]
    fn sorts_server_errors() {
        assert_eq!(
            td(429, "Too Many Requests: retry after 75"),
            Error::Flood {
                retry_after: Duration::from_secs(75)
            }
        );
        assert_eq!(
            td(420, "2FA_CONFIRM_WAIT_604800"),
            Error::ResetWait {
                wait: Duration::from_secs(604_800)
            }
        );
        assert_eq!(td(400, "PHONE_CODE_INVALID"), Error::CodeInvalid);
        assert_eq!(td(400, "PASSWORD_HASH_INVALID"), Error::PasswordInvalid);
        assert_eq!(
            td(400, "SOMETHING_NEW"),
            Error::Server {
                code: 400,
                message: "SOMETHING_NEW".to_owned()
            }
        );
        assert_eq!(Error::from(TdError::Closed), Error::Closed);
    }

    #[test]
    fn diagnostics_omit_server_text() {
        let error = td(400, "private fixture");
        assert_eq!(error.to_string(), "error 400");
        assert_eq!(format!("{error:?}"), "error 400");
        assert!(matches!(error, Error::Server { message, .. } if message == "private fixture"));
    }

    #[test]
    fn a_malformed_flood_error_stays_a_server_error() {
        assert!(matches!(
            td(429, "Too Many Requests: retry after soon"),
            Error::Server { code: 429, .. }
        ));
    }
}
