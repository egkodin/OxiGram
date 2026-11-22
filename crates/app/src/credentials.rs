//! Telegram API credentials (ADR 0004). Release builds supply OxiGram's
//! credentials at compile time. Source builds can supply their own at runtime.

use std::env;
use std::fmt;

pub(crate) const API_ID_VAR: &str = "RGRAM_API_ID";
pub(crate) const API_HASH_VAR: &str = "RGRAM_API_HASH";

/// A validated `api_id` and `api_hash`.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ApiCredentials {
    pub(crate) id: i32,
    pub(crate) hash: String,
}

impl fmt::Debug for ApiCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiCredentials")
            .field("id", &self.id)
            .field("hash", &"<redacted>")
            .finish()
    }
}

/// Why the credentials cannot be used. The messages are shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CredentialsError {
    #[error(
        "Telegram API credentials are missing. Get an api_id and api_hash at \
         https://my.telegram.org, set {API_ID_VAR} and {API_HASH_VAR}, and start rgram again."
    )]
    Missing,
    #[error("{API_ID_VAR} must be the positive number shown at https://my.telegram.org.")]
    InvalidId,
    #[error(
        "{API_HASH_VAR} must be the 32-character hexadecimal hash shown at https://my.telegram.org."
    )]
    InvalidHash,
}

/// Reads a runtime override or uses the bundled app credentials.
pub(crate) fn from_env() -> Result<ApiCredentials, CredentialsError> {
    resolve(
        (
            env::var(API_ID_VAR).ok().as_deref(),
            env::var(API_HASH_VAR).ok().as_deref(),
        ),
        RELEASE_DEFAULTS,
    )
}

const RELEASE_DEFAULTS: (Option<&str>, Option<&str>) = (
    option_env!("OXIGRAM_RELEASE_API_ID"),
    option_env!("OXIGRAM_RELEASE_API_HASH"),
);

pub(crate) fn check_release_configuration() -> Result<(), CredentialsError> {
    parse(RELEASE_DEFAULTS.0, RELEASE_DEFAULTS.1).map(|_| ())
}

fn resolve(
    runtime: (Option<&str>, Option<&str>),
    release: (Option<&str>, Option<&str>),
) -> Result<ApiCredentials, CredentialsError> {
    let (id, hash) = if runtime == (None, None) {
        release
    } else {
        runtime
    };
    parse(id, hash)
}

fn parse(id: Option<&str>, hash: Option<&str>) -> Result<ApiCredentials, CredentialsError> {
    let (Some(id), Some(hash)) = (non_empty(id), non_empty(hash)) else {
        return Err(CredentialsError::Missing);
    };
    let id = id
        .parse::<i32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or(CredentialsError::InvalidId)?;
    let is_valid_hash = hash.len() == 32 && hash.bytes().all(|b| b.is_ascii_hexdigit());
    if !is_valid_hash {
        return Err(CredentialsError::InvalidHash);
    }
    Ok(ApiCredentials {
        id,
        hash: hash.to_ascii_lowercase(),
    })
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0123456789abcdef0123456789ABCDEF";

    #[test]
    fn release_defaults_apply_only_without_a_runtime_override() {
        let defaults = (Some("12345"), Some(HASH));
        assert_eq!(resolve((None, None), defaults).unwrap().id, 12345);
        assert_eq!(
            resolve((Some("67890"), Some(HASH)), defaults).unwrap().id,
            67890
        );
        assert_eq!(
            resolve((Some("67890"), None), defaults),
            Err(CredentialsError::Missing)
        );
        assert_eq!(
            resolve((None, None), (None, None)),
            Err(CredentialsError::Missing)
        );
    }

    #[test]
    fn accepts_valid_credentials() {
        let credentials = parse(Some(" 12345 "), Some(HASH)).unwrap();
        assert_eq!(credentials.id, 12345);
        assert_eq!(credentials.hash, HASH.to_ascii_lowercase());
    }

    #[test]
    fn missing_or_empty_values_are_missing() {
        assert_eq!(parse(None, Some(HASH)), Err(CredentialsError::Missing));
        assert_eq!(parse(Some("1"), None), Err(CredentialsError::Missing));
        assert_eq!(
            parse(Some("  "), Some(HASH)),
            Err(CredentialsError::Missing)
        );
    }

    #[test]
    fn rejects_malformed_values() {
        assert_eq!(
            parse(Some("-1"), Some(HASH)),
            Err(CredentialsError::InvalidId)
        );
        assert_eq!(
            parse(Some("abc"), Some(HASH)),
            Err(CredentialsError::InvalidId)
        );
        assert_eq!(
            parse(Some("1"), Some("xyz")),
            Err(CredentialsError::InvalidHash)
        );
    }

    #[test]
    fn debug_output_hides_the_hash() {
        let credentials = parse(Some("1"), Some(HASH)).unwrap();
        assert!(!format!("{credentials:?}").contains("0123456789"));
    }
}
