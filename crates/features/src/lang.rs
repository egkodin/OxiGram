//! The interface language: tdesktop's English strings, or a pack from
//! Telegram's servers over them.

use gpui::{App, Global, SharedString};
use i18n::{Key, Strings, Value};
use model::Session;
use model::language::{PackString, PackValue};

/// tdesktop's default language pack.
pub const ENGLISH: &str = "en";

/// The current strings; replaced as a whole when the language changes.
#[derive(Debug, Clone, Default)]
pub struct Lang {
    strings: Strings,
}

impl Global for Lang {}

impl Lang {
    /// `language`'s pack over the English strings.
    #[must_use]
    pub fn from_pack(language: &str, pack: Vec<PackString>) -> Self {
        let values = pack.into_iter().filter_map(|string| {
            let value = match string.value? {
                PackValue::Text(text) => Value::Text(text),
                PackValue::Plural(forms) => Value::Plural(forms),
            };
            Some((string.key, value))
        });
        Self {
            strings: Strings::with_pack(language, values),
        }
    }

    #[must_use]
    pub fn language(&self) -> &str {
        self.strings.language()
    }
}

/// The text of `key` in the current language.
#[must_use]
pub fn tr(cx: &App, key: Key) -> SharedString {
    SharedString::from(strings(cx).text(key).to_owned())
}

/// The text of `key` with each `{name}` replaced.
#[must_use]
pub fn tr_with(cx: &App, key: Key, args: &[(&str, &str)]) -> String {
    i18n::fill(strings(cx).text(key), args)
}

/// The form of plural `key` for `count`, with `{count}` and `args` filled.
#[must_use]
pub fn tr_plural(cx: &App, key: Key, count: i64, args: &[(&str, &str)]) -> String {
    let count_text = count.to_string();
    let mut all = vec![("count", count_text.as_str())];
    all.extend_from_slice(args);
    i18n::fill(strings(cx).plural(key, count), &all)
}

/// As [`tr_plural`], with the count's digits grouped (`lt_count_decimal`).
#[must_use]
pub fn tr_plural_decimal(cx: &App, key: Key, count: i64) -> String {
    i18n::fill(
        strings(cx).plural(key, count),
        &[("count", &group_digits(count))],
    )
}

/// `48210` as "48,210" (`FormatCountDecimal`).
// shortcut: English grouping for every language; locale number formats come with M7.
pub(crate) fn group_digits(number: i64) -> String {
    let digits = number.unsigned_abs().to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if number < 0 {
        grouped.push('-');
    }
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// A phone number as "+1 555 0100" when its country's format is known
/// (`Ui::FormatPhone`), else "+" and the digits.
pub(crate) fn format_phone(digits: &str) -> String {
    let digits = digits.trim_start_matches('+');
    if digits.is_empty() {
        return String::new();
    }
    match model::phone::number_info("en", digits) {
        Ok(info) if !info.formatted.contains('-') => {
            format!("+{} {}", info.calling_code, info.formatted)
        }
        _ => format!("+{digits}"),
    }
}

/// Loads the language TDLib remembers, at startup.
pub fn load(session: &Session, cx: &mut App) {
    let session = session.clone();
    let task = cx.background_executor().spawn(async move {
        let pack = session.language().await?;
        if pack.is_empty() || pack == ENGLISH {
            return Ok((pack, Vec::new()));
        }
        let strings = session.language_pack_strings(&pack, Vec::new()).await?;
        Ok::<_, model::RequestError>((pack, strings))
    });
    apply(task, cx);
}

/// Switches the interface to `pack` and remembers it in TDLib
/// (`CloudManager::switchToLanguage`); an empty pack is English.
pub fn switch_to(session: &Session, pack: &str, cx: &mut App) {
    let set = session.set_option("language_pack_id", pack);
    let pack = pack.to_owned();
    let strings = (!pack.is_empty() && pack != ENGLISH)
        .then(|| session.language_pack_strings(&pack, Vec::new()));
    let task = cx.background_executor().spawn(async move {
        set.await?;
        let strings = match strings {
            Some(strings) => strings.await?,
            None => Vec::new(),
        };
        Ok::<_, model::RequestError>((pack, strings))
    });
    apply(task, cx);
}

fn apply(task: gpui::Task<Result<(String, Vec<PackString>), model::RequestError>>, cx: &mut App) {
    cx.spawn(async move |cx| match task.await {
        Ok((pack, strings)) => {
            let language = if pack.is_empty() {
                ENGLISH
            } else {
                pack.as_str()
            };
            let lang = Lang::from_pack(language, strings);
            cx.update(|cx| cx.set_global(lang));
        }
        Err(error) => tracing::warn!(%error, "loading the language pack failed"),
    })
    .detach();
}

pub(crate) fn strings(cx: &App) -> &Strings {
    &cx.global::<Lang>().strings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_digits_by_thousands() {
        assert_eq!(group_digits(9), "9");
        assert_eq!(group_digits(48_210), "48,210");
        assert_eq!(group_digits(1_000_000), "1,000,000");
        assert_eq!(group_digits(-1234), "-1,234");
    }

    #[test]
    fn packs_replace_english_and_skip_deleted_keys() {
        let lang = Lang::from_pack(
            "de",
            vec![
                PackString {
                    key: "lng_intro_next".to_owned(),
                    value: Some(PackValue::Text("Weiter".to_owned())),
                },
                PackString {
                    key: "lng_start_msgs".to_owned(),
                    value: None,
                },
            ],
        );
        assert_eq!(lang.strings.text(i18n::key::lng_intro_next), "Weiter");
        assert_eq!(
            lang.strings.text(i18n::key::lng_start_msgs),
            "Start Messaging"
        );
        assert_eq!(lang.language(), "de");
    }
}
