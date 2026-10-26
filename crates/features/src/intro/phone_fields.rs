//! The phone number fields' correctors (`special_fields.cpp`): the country
//! code keeps `+` and four digits, and the rest of the number is grouped by
//! its country's pattern.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::SharedString;
use ui::Corrector;

/// `kMaxPhoneTailLength`.
const MAX_TAIL_DIGITS: usize = 32;
/// The digits the country code field keeps.
const MAX_CODE_DIGITS: usize = 4;
/// The placeholder for digits still to type (`QChar(0x2212)`).
const MINUS: char = '\u{2212}';

/// The longest valid calling code `code` starts with
/// (`Countries::validPhoneCode`).
pub(crate) type ValidCode = fn(&str) -> String;

/// The phone groups for a number prefix with its calling code; `None` when
/// the calling code is not `code` (`_groupsCallback`).
pub(crate) type Groups = fn(code: &str, digits: &str) -> Vec<usize>;

/// `CountryCodeInput::correctValue`. Digits past the code go to the number
/// field through `overflow`.
pub(crate) struct CodeCorrector {
    pub valid_code: ValidCode,
    pub overflow: Rc<RefCell<String>>,
}

impl Corrector for CodeCorrector {
    fn correct(&mut self, _: &str, _: usize, now: &mut String, now_cursor: &mut usize) {
        let corrected = correct_code(now, *now_cursor, self.valid_code);
        *now = corrected.text;
        *now_cursor = corrected.cursor;
        if !corrected.overflow.is_empty() {
            self.overflow.borrow_mut().push_str(&corrected.overflow);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CodeCorrection {
    text: String,
    cursor: usize,
    overflow: String,
}

fn correct_code(now: &str, cursor: usize, valid_code: ValidCode) -> CodeCorrection {
    let mut text = String::from("+");
    let mut overflow = String::new();
    let mut new_cursor = None;
    let mut rest = now;
    if let Some(after_plus) = now.strip_prefix('+') {
        if cursor == 0 {
            new_cursor = Some(0);
        }
        rest = after_plus;
    }
    let offset = now.len() - rest.len();
    let mut digits = 0;
    for (index, ch) in rest.char_indices() {
        if offset + index == cursor && new_cursor.is_none() {
            new_cursor = Some(text.len());
        }
        if ch.is_ascii_digit() {
            if digits < MAX_CODE_DIGITS {
                text.push(ch);
                digits += 1;
            } else {
                overflow.push(ch);
            }
        }
    }
    if !overflow.is_empty() {
        let code = valid_code(&text[1..]);
        overflow = format!("{}{overflow}", &text[1 + code.len().min(text.len() - 1)..]);
        text = format!("+{code}");
    }
    let cursor = new_cursor.unwrap_or(text.len()).min(text.len());
    CodeCorrection {
        text,
        cursor,
        overflow,
    }
}

/// `PhonePartInput::correctValue`: groups the digits by the pattern of the
/// country `code` names, and hints the digits still to type.
pub(crate) struct PhoneCorrector {
    pub code: Rc<RefCell<String>>,
    pub groups: Groups,
    pattern: Vec<usize>,
}

impl PhoneCorrector {
    pub fn new(code: Rc<RefCell<String>>, groups: Groups) -> Self {
        Self {
            code,
            groups,
            pattern: Vec::new(),
        }
    }
}

impl Corrector for PhoneCorrector {
    fn correct(&mut self, _: &str, _: usize, now: &mut String, now_cursor: &mut usize) {
        let digits: String = now.chars().filter(char::is_ascii_digit).collect();
        self.pattern = (self.groups)(&self.code.borrow(), &digits);
        let (text, cursor) = correct_phone(now, *now_cursor, &self.pattern);
        *now = text;
        *now_cursor = cursor;
    }

    fn trailing_hint(&self, text: &str) -> Option<SharedString> {
        let placeholder = placeholder(&self.pattern);
        let rest: String = placeholder.chars().skip(text.chars().count()).collect();
        (!rest.is_empty()).then(|| rest.into())
    }
}

/// `_additionalPlaceholder`: " −−− −−− −−−−" for a 3-3-4 pattern.
fn placeholder(pattern: &[usize]) -> String {
    pattern
        .iter()
        .map(|part| format!(" {}", MINUS.to_string().repeat(*part)))
        .collect()
}

fn correct_phone(now: &str, cursor: usize, pattern: &[usize]) -> (String, usize) {
    let mut digit_count = now
        .chars()
        .filter(char::is_ascii_digit)
        .count()
        .min(MAX_TAIL_DIGITS);
    let mut in_part = !pattern.is_empty();
    let mut part: Option<usize> = None;
    let mut left_in_part = 0;
    let mut text = String::with_capacity(now.len());
    let mut new_cursor = None;
    for (index, ch) in now.char_indices() {
        if index == cursor && new_cursor.is_none() {
            new_cursor = Some(text.len());
        }
        if ch.is_ascii_digit() {
            if digit_count == 0 {
                break;
            }
            digit_count -= 1;
            if in_part {
                if left_in_part > 0 {
                    left_in_part -= 1;
                } else {
                    let next = part.map_or(0, |part| part + 1);
                    part = Some(next);
                    in_part = next < pattern.len();
                    // A group starts with a space; none is added at the end.
                    if in_part {
                        // The cursor sat before this digit, so it moves past the space.
                        if new_cursor == Some(text.len()) && index == cursor {
                            new_cursor = Some(text.len() + 1);
                        }
                        text.push(' ');
                    }
                    left_in_part = if in_part { pattern[next] - 1 } else { 0 };
                }
            }
            text.push(ch);
        } else if matches!(ch, ' ' | '-' | '(' | ')') {
            if in_part {
                if left_in_part == 0 {
                    text.push(ch);
                    let next = part.map_or(0, |part| part + 1);
                    part = Some(next);
                    in_part = next < pattern.len();
                    left_in_part = if in_part { pattern[next] } else { 0 };
                }
            } else {
                text.push(ch);
            }
        }
    }
    let trimmed = text.trim_end().len();
    text.truncate(trimmed);
    let cursor = new_cursor.unwrap_or(text.len()).min(text.len());
    (text, cursor)
}

/// The groups of a TDLib-formatted number tail ("212 555 ----" is 3-3-4).
pub(crate) fn groups_of(formatted: &str) -> Vec<usize> {
    formatted
        .split(' ')
        .filter(|group| !group.is_empty())
        .map(|group| group.chars().count())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn us_code(code: &str) -> String {
        if code.starts_with('1') {
            "1".to_owned()
        } else {
            code.to_owned()
        }
    }

    #[test]
    fn code_keeps_plus_and_four_digits() {
        let corrected = correct_code("7a9", 3, us_code);
        assert_eq!(corrected.text, "+79");
        assert_eq!(corrected.overflow, "");
        assert_eq!(corrected.cursor, 3);
    }

    #[test]
    fn code_overflow_moves_past_the_valid_code() {
        // Typing a whole US number into the code field.
        let corrected = correct_code("+12125", 6, us_code);
        assert_eq!(corrected.text, "+1");
        assert_eq!(corrected.overflow, "2125");
    }

    #[test]
    fn phone_is_grouped_by_the_pattern() {
        let pattern = [3, 3, 4];
        assert_eq!(
            correct_phone("2125550100", 10, &pattern),
            (" 212 555 0100".to_owned(), 13)
        );
        assert_eq!(
            correct_phone("21255", 5, &pattern),
            (" 212 55".to_owned(), 7)
        );
        // Digits past the pattern stay unformatted.
        assert_eq!(
            correct_phone("212555010099", 12, &pattern).0,
            " 212 555 010099"
        );
    }

    #[test]
    fn phone_without_pattern_keeps_digits_and_separators() {
        assert_eq!(correct_phone("12-3x4", 6, &[]), ("12-34".to_owned(), 5));
    }

    #[test]
    fn hint_shows_the_digits_still_to_type() {
        assert_eq!(
            placeholder(&[3, 3]),
            " \u{2212}\u{2212}\u{2212} \u{2212}\u{2212}\u{2212}"
        );
        assert_eq!(groups_of("212 555 ----"), vec![3, 3, 4]);
        assert_eq!(groups_of(""), Vec::<usize>::new());
    }
}
