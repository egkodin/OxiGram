//! Countries and phone number formatting for the phone number screen.

use td_api::functions::GetPhoneNumberInfoSync;

use crate::session::RequestError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country {
    /// ISO 3166-1 alpha-2 code.
    pub code: String,
    /// The name in the current language.
    pub name: String,
    pub english_name: String,
    pub flag: String,
    /// Hidden countries are not listed but still match typed numbers.
    pub is_hidden: bool,
    /// Calling codes without `+`.
    pub calling_codes: Vec<String>,
}

/// What a phone number prefix says about the number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberInfo {
    pub country: Option<Country>,
    /// The calling code found in the prefix, without `+`.
    pub calling_code: String,
    /// The rest of the number formatted for its country; digits still
    /// expected are `-`.
    pub formatted: String,
    /// A Fragment anonymous number.
    pub is_anonymous: bool,
}

/// Looks up `prefix` (digits, with or without the calling code) without a
/// round trip; country names follow `language`, once `Session::countries` has
/// loaded them for it.
///
/// # Errors
///
/// Returns TDLib's error, such as for a prefix that is not a phone number.
pub fn number_info(language: &str, prefix: &str) -> Result<NumberInfo, RequestError> {
    let info = tg::execute(&GetPhoneNumberInfoSync {
        language_code: language.to_owned(),
        phone_number_prefix: prefix.to_owned(),
    })?;
    Ok(NumberInfo {
        country: info.country.map(|country| Country::from(*country)),
        calling_code: info.country_calling_code,
        formatted: info.formatted_phone_number,
        is_anonymous: info.is_anonymous,
    })
}

impl From<td_api::types::CountryInfo> for Country {
    fn from(country: td_api::types::CountryInfo) -> Self {
        Self {
            code: country.country_code,
            name: country.name,
            english_name: country.english_name,
            flag: country.flag_emoji,
            is_hidden: country.is_hidden,
            calling_codes: country.calling_codes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_numbers_with_tdlib() {
        // TDLib logs its built-in country list at the default verbosity.
        tg::execute(&td_api::functions::SetLogVerbosityLevel {
            new_verbosity_level: 1,
        })
        .expect("setLogVerbosityLevel runs synchronously");
        let info = number_info("en", "1212555").expect("TDLib formats US numbers offline");
        assert_eq!(info.calling_code, "1");
        assert_eq!(
            info.country.map(|country| country.code).as_deref(),
            Some("US")
        );
        assert!(info.formatted.starts_with("212 555"), "{}", info.formatted);
        assert!(info.formatted.contains('-'), "{}", info.formatted);
    }
}
