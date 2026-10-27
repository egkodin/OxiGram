//! Text with formatting entities.

use std::ops::Range;

use td_api::enums::{DateTimeFormattingType, DateTimePartPrecision, TextEntityType};

use crate::ids::{CustomEmojiId, UserId};

/// Text with entities. Entity ranges are byte ranges into `text` on `char`
/// boundaries (TDLib counts UTF-16 code units; the conversion happens here).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormattedText {
    pub text: String,
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub range: Range<usize>,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityKind {
    Mention,
    Hashtag,
    Cashtag,
    BotCommand,
    Url,
    EmailAddress,
    PhoneNumber,
    BankCardNumber,
    TonAddress,
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Spoiler,
    Code,
    Pre {
        language: Option<String>,
    },
    BlockQuote(Quote),
    TextUrl {
        url: String,
    },
    MentionName {
        user: UserId,
    },
    CustomEmoji {
        id: CustomEmojiId,
    },
    MediaTimestamp {
        seconds: i32,
    },
    /// A date the client shows in its own words; `format` none keeps the
    /// text as sent.
    DateTime {
        unix_time: i32,
        format: Option<DateFormat>,
    },
}

/// How a date in text reads (`dateTimeFormattingType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// "in 5 minutes", "2 days ago".
    Relative,
    Absolute {
        time: Precision,
        date: Precision,
        has_day_of_week: bool,
    },
}

/// How much of a date or time part shows (`dateTimePartPrecision`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    None,
    Short,
    Long,
}

impl From<DateTimePartPrecision> for Precision {
    fn from(precision: DateTimePartPrecision) -> Self {
        match precision {
            DateTimePartPrecision::None(_) => Self::None,
            DateTimePartPrecision::Short(_) => Self::Short,
            DateTimePartPrecision::Long(_) => Self::Long,
        }
    }
}

impl From<DateTimeFormattingType> for DateFormat {
    fn from(format: DateTimeFormattingType) -> Self {
        match format {
            DateTimeFormattingType::Relative(_) => Self::Relative,
            DateTimeFormattingType::Absolute(absolute) => Self::Absolute {
                time: Precision::from(*absolute.time_precision),
                date: Precision::from(*absolute.date_precision),
                has_day_of_week: absolute.show_day_of_week,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
    Plain,
    Expandable,
}

impl FormattedText {
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            entities: Vec::new(),
        }
    }
}

impl From<td_api::types::FormattedText> for FormattedText {
    fn from(source: td_api::types::FormattedText) -> Self {
        let offsets = Utf16Offsets::new(&source.text);
        let entities = source
            .entities
            .into_iter()
            .filter_map(|entity| {
                let start = usize::try_from(entity.offset).ok()?;
                let end = start.checked_add(usize::try_from(entity.length).ok()?)?;
                let range = offsets.byte(start)?..offsets.byte(end)?;
                Some(Entity {
                    range,
                    kind: EntityKind::from(*entity.r#type),
                })
            })
            .collect();
        Self {
            text: source.text,
            entities,
        }
    }
}

impl From<TextEntityType> for EntityKind {
    fn from(kind: TextEntityType) -> Self {
        match kind {
            TextEntityType::Mention(_) => Self::Mention,
            TextEntityType::Hashtag(_) => Self::Hashtag,
            TextEntityType::Cashtag(_) => Self::Cashtag,
            TextEntityType::BotCommand(_) => Self::BotCommand,
            TextEntityType::Url(_) => Self::Url,
            TextEntityType::EmailAddress(_) => Self::EmailAddress,
            TextEntityType::PhoneNumber(_) => Self::PhoneNumber,
            TextEntityType::BankCardNumber(_) => Self::BankCardNumber,
            TextEntityType::TonAddress(_) => Self::TonAddress,
            TextEntityType::Bold(_) => Self::Bold,
            TextEntityType::Italic(_) => Self::Italic,
            TextEntityType::Underline(_) => Self::Underline,
            TextEntityType::Strikethrough(_) => Self::Strikethrough,
            TextEntityType::Spoiler(_) => Self::Spoiler,
            TextEntityType::Code(_) => Self::Code,
            TextEntityType::Pre(_) => Self::Pre { language: None },
            TextEntityType::PreCode(pre) => Self::Pre {
                language: Some(pre.language).filter(|language| !language.is_empty()),
            },
            TextEntityType::BlockQuote(_) => Self::BlockQuote(Quote::Plain),
            TextEntityType::ExpandableBlockQuote(_) => Self::BlockQuote(Quote::Expandable),
            TextEntityType::TextUrl(link) => Self::TextUrl { url: link.url },
            TextEntityType::MentionName(mention) => Self::MentionName {
                user: UserId(mention.user_id),
            },
            TextEntityType::CustomEmoji(emoji) => Self::CustomEmoji {
                id: CustomEmojiId(emoji.custom_emoji_id),
            },
            TextEntityType::MediaTimestamp(timestamp) => Self::MediaTimestamp {
                seconds: timestamp.media_timestamp,
            },
            TextEntityType::DateTime(date) => Self::DateTime {
                unix_time: date.unix_time,
                format: date.formatting_type.map(|format| DateFormat::from(*format)),
            },
        }
    }
}

/// Maps UTF-16 offsets to byte offsets. Offsets inside a surrogate pair or
/// past the end map to `None`.
struct Utf16Offsets {
    /// `(utf16 offset, byte offset)` of every char boundary, plus the end.
    boundaries: Vec<(usize, usize)>,
}

impl Utf16Offsets {
    fn new(text: &str) -> Self {
        let mut boundaries = Vec::with_capacity(text.len() + 1);
        let mut utf16 = 0;
        for (byte, c) in text.char_indices() {
            boundaries.push((utf16, byte));
            utf16 += c.len_utf16();
        }
        boundaries.push((utf16, text.len()));
        Self { boundaries }
    }

    fn byte(&self, utf16: usize) -> Option<usize> {
        let index = self
            .boundaries
            .binary_search_by_key(&utf16, |(offset, _)| *offset)
            .ok()?;
        Some(self.boundaries[index].1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn td_text(json: &str) -> td_api::types::FormattedText {
        serde_json::from_str(json).expect("fixture is a valid formattedText")
    }

    #[test]
    fn converts_utf16_offsets_to_byte_ranges() {
        // "😀 bold é": the emoji is two UTF-16 units and four bytes.
        let text = FormattedText::from(td_text(
            r#"{"@type":"formattedText","text":"😀 bold é","entities":[
                {"@type":"textEntity","offset":3,"length":4,"type":{"@type":"textEntityTypeBold"}},
                {"@type":"textEntity","offset":8,"length":1,"type":{"@type":"textEntityTypeTextUrl","url":"https://t.me"}}
            ]}"#,
        ));
        assert_eq!(&text.text[text.entities[0].range.clone()], "bold");
        assert_eq!(text.entities[0].kind, EntityKind::Bold);
        assert_eq!(&text.text[text.entities[1].range.clone()], "é");
        assert_eq!(
            text.entities[1].kind,
            EntityKind::TextUrl {
                url: "https://t.me".to_owned()
            }
        );
    }

    #[test]
    fn drops_entities_that_split_a_surrogate_pair_or_overflow() {
        let text = FormattedText::from(td_text(
            r#"{"@type":"formattedText","text":"😀a","entities":[
                {"@type":"textEntity","offset":1,"length":1,"type":{"@type":"textEntityTypeBold"}},
                {"@type":"textEntity","offset":2,"length":5,"type":{"@type":"textEntityTypeItalic"}}
            ]}"#,
        ));
        assert!(text.entities.is_empty(), "{:?}", text.entities);
    }
}
