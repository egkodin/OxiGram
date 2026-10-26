use std::fmt;

/// Text that must never reach logs: phone numbers, codes, passwords, tokens.
/// `Debug` prints `<redacted>`; read the value with [`Secret::expose`].
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value itself, for sending it or showing it to its owner.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_value() {
        let secret = Secret::new("+15551234567");
        assert_eq!(format!("{secret:?}"), "<redacted>");
        assert_eq!(secret.expose(), "+15551234567");
    }
}
