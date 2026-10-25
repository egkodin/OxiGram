use std::fmt;

/// Why a TDLib request failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TdError {
    /// TDLib answered with an `error` object.
    #[error("TDLib error {code}: {message}")]
    Td { code: i32, message: String },
    /// The client or the runtime closed before the response arrived.
    #[error("the TDLib client is closed")]
    Closed,
    /// `td_execute` returned nothing.
    #[error("TDLib returned no response")]
    NoResponse,
    /// The request could not be serialized.
    #[error("cannot encode a {0} request")]
    Encode(&'static str),
    /// The response did not match the expected type.
    #[error("cannot decode the response to {function}: {error}")]
    Decode {
        function: &'static str,
        error: DecodeError,
    },
}

impl TdError {
    /// Converts an object whose `@type` is `error`.
    pub(crate) fn from_error_object(object: &str) -> Self {
        match serde_json::from_str::<td_api::types::Error>(object) {
            Ok(error) => Self::Td {
                code: error.code,
                message: error.message,
            },
            Err(error) => Self::Decode {
                function: "error",
                error: DecodeError::from(&error),
            },
        }
    }
}

/// Where a JSON object failed to decode, without any of its content.
///
/// `serde_json::Error` quotes the offending value, which may be message text or
/// a phone number, so it never reaches errors or logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError {
    category: serde_json::error::Category,
    line: usize,
    column: usize,
}

impl From<&serde_json::Error> for DecodeError {
    fn from(error: &serde_json::Error) -> Self {
        Self {
            category: error.classify(),
            line: error.line(),
            column: error.column(),
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.category {
            serde_json::error::Category::Io => "I/O error",
            serde_json::error::Category::Syntax => "syntax error",
            serde_json::error::Category::Data => "unexpected data",
            serde_json::error::Category::Eof => "unexpected end",
        };
        write!(f, "{kind} at line {} column {}", self.line, self.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_errors_do_not_quote_the_input() {
        let error = serde_json::from_str::<td_api::types::Error>(
            r#"{"@type":"error","code":"+15551234567","message":"x"}"#,
        )
        .unwrap_err();
        let text = DecodeError::from(&error).to_string();
        assert!(!text.contains("5551234567"), "{text}");
        assert!(text.starts_with("unexpected data at line 1"), "{text}");
    }

    #[test]
    fn error_objects_become_td_errors() {
        let error = TdError::from_error_object(
            r#"{"@type":"error","code":400,"message":"CHAT_NOT_FOUND","@extra":3,"@client_id":1}"#,
        );
        assert_eq!(
            error,
            TdError::Td {
                code: 400,
                message: "CHAT_NOT_FOUND".to_owned()
            }
        );
    }
}
