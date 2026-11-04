//! Interface language packs from Telegram's servers (tdesktop's target).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{LanguagePackStringValue, OptionValue};
use td_api::functions::{GetLanguagePackStrings, GetLocalizationTargetInfo, GetOption};

use crate::session::{RequestError, Session};

/// One string of a pack; `None` when the pack deleted the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackString {
    pub key: String,
    pub value: Option<PackValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackValue {
    Text(String),
    /// Zero, one, two, few, many and other, in CLDR order.
    Plural([String; 6]),
}

/// A language the interface can switch to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguagePack {
    pub id: String,
    pub name: String,
    pub native_name: String,
    /// Telegram's own translation, not one made on the platform by users.
    pub is_official: bool,
}

impl Session {
    /// The languages Telegram offers and those added from links
    /// (`CloudManager::requestLanguageList`).
    pub fn language_packs(
        &self,
    ) -> impl Future<Output = Result<Vec<LanguagePack>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetLocalizationTargetInfo { only_local: false })
            .map(|result| {
                result
                    .map(|info| {
                        let packs = info.language_packs.into_iter();
                        packs
                            .filter(|pack| pack.is_official || pack.is_installed)
                            .map(|pack| LanguagePack {
                                id: pack.id,
                                name: pack.name,
                                native_name: pack.native_name,
                                is_official: pack.is_official,
                            })
                            .collect()
                    })
                    .map_err(RequestError::from)
            })
    }

    /// Strings of `pack` (all of them when `keys` is empty).
    pub fn language_pack_strings(
        &self,
        pack: &str,
        keys: Vec<String>,
    ) -> impl Future<Output = Result<Vec<PackString>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetLanguagePackStrings {
                language_pack_id: pack.to_owned(),
                keys,
            })
            .map(|result| {
                result
                    .map(|strings| strings.strings.into_iter().map(PackString::from).collect())
                    .map_err(RequestError::from)
            })
    }

    /// The chosen language pack (`language_pack_id`); empty for the
    /// built-in English.
    pub fn language(&self) -> impl Future<Output = Result<String, RequestError>> + Send + use<> {
        self.client()
            .request(&GetOption {
                name: "language_pack_id".to_owned(),
            })
            .map(|result| match result {
                Ok(OptionValue::String(value)) => Ok(value.value),
                Ok(_) => Ok(String::new()),
                Err(error) => Err(RequestError::from(error)),
            })
    }
}

impl From<td_api::types::LanguagePackString> for PackString {
    fn from(string: td_api::types::LanguagePackString) -> Self {
        let value = string.value.and_then(|value| match *value {
            LanguagePackStringValue::Ordinary(text) => Some(PackValue::Text(text.value)),
            LanguagePackStringValue::Pluralized(forms) => Some(PackValue::Plural([
                forms.zero_value,
                forms.one_value,
                forms.two_value,
                forms.few_value,
                forms.many_value,
                forms.other_value,
            ])),
            LanguagePackStringValue::Deleted(_) => None,
        });
        Self {
            key: string.key,
            value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_plural_strings_in_cldr_order() {
        let string: td_api::types::LanguagePackString = serde_json::from_str(
            r#"{"key":"lng_days","value":{"@type":"languagePackStringValuePluralized","zero_value":"0","one_value":"1","two_value":"2","few_value":"f","many_value":"m","other_value":"o"}}"#,
        )
        .expect("fixture is a valid string");
        let PackString { key, value } = PackString::from(string);
        assert_eq!(key, "lng_days");
        let Some(PackValue::Plural(forms)) = value else {
            panic!("a plural value");
        };
        assert_eq!(forms[1], "1");
        assert_eq!(forms[5], "o");
    }
}
