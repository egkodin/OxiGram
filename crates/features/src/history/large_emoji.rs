//! Up to three emoji sent alone (`LargeEmoji` in `UnwrappedMedia`): drawn
//! big without a bubble, the time on its pill beside them.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use gpui::prelude::FluentBuilder as _;
use gpui::{App, Div, ParentElement as _, RenderImage, Styled as _, div, img};
use model::CustomEmojiId;
use model::message::{Content, Message};
use model::sticker::{StickerKind, StickerPicture};
use model::text::{EntityKind, FormattedText};
use style::chat::{LARGE_EMOJI_OUTLINE, LARGE_EMOJI_SIZE, LARGE_EMOJI_SKIP, MSG_DATE_IMG_DELTA};
use style::emoji::EMOJI_SECTIONS;

use super::bubble::Item;
use crate::px_of;

/// `kIsolatedEmojiLimit`.
const LIMIT: usize = 3;
const VARIATION: char = '\u{FE0F}';

/// Every emoji tdesktop knows, by its chars without variation selectors,
/// and the longest one's length in chars.
fn known() -> &'static (HashMap<String, &'static str>, usize) {
    static KNOWN: OnceLock<(HashMap<String, &'static str>, usize)> = OnceLock::new();
    KNOWN.get_or_init(|| {
        let map: HashMap<String, &'static str> = EMOJI_SECTIONS
            .iter()
            .flat_map(|section| section.iter())
            .map(|emoji| (emoji.chars().filter(|c| *c != VARIATION).collect(), *emoji))
            .collect();
        let longest = map
            .keys()
            .map(|emoji| emoji.chars().count())
            .max()
            .unwrap_or(1);
        (map, longest)
    })
}

/// A skin tone after an emoji (`EmojiModifier`).
fn is_tone(c: char) -> bool {
    ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
}

/// The emoji when `text` is one to three of them and nothing else
/// (`toIsolatedEmoji`), each as tdesktop lists it, so a heart keeps its
/// selector.
pub(super) fn isolated(text: &str) -> Option<Vec<String>> {
    let (known, longest) = known();
    let chars: Vec<char> = text.trim().chars().filter(|c| *c != VARIATION).collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        if found.len() == LIMIT {
            return None;
        }
        let last = chars.len().min(at + longest);
        let (mut end, mut emoji) = (at + 1..=last).rev().find_map(|end| {
            let key: String = chars[at..end].iter().collect();
            known.get(&key).map(|emoji| (end, (*emoji).to_owned()))
        })?;
        if let Some(tone) = chars.get(end).copied().filter(|c| is_tone(*c)) {
            emoji = emoji.trim_end_matches(VARIATION).to_owned();
            emoji.push(tone);
            end += 1;
        }
        found.push(emoji);
        at = end;
    }
    (!found.is_empty()).then_some(found)
}

/// Whether the sticker can be drawn: a still or TGS one (the Lottie
/// player), or a video one with a still.
pub(super) fn is_drawable(sticker: &StickerPicture) -> bool {
    sticker.kind != StickerKind::Video || sticker.still.is_some()
}

/// One emoji a message shows big, and the custom emoji drawn for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Big {
    pub emoji: String,
    pub custom: Option<CustomEmojiId>,
}

/// How a big emoji is drawn.
pub(super) enum Face {
    Glyph(String),
    /// A custom emoji's still, by path.
    Picture(String),
    /// A custom emoji's Lottie frame.
    Frame(Arc<RenderImage>),
}

/// The emoji a message shows big: one sent alone that has no sticker to
/// draw, or a text of one to three emoji, custom ones among them.
pub(super) fn of(message: &Message) -> Option<Vec<Big>> {
    let glyphs = |emoji: Vec<String>| {
        emoji
            .into_iter()
            .map(|emoji| Big {
                emoji,
                custom: None,
            })
            .collect()
    };
    match &message.content {
        Content::Emoji { emoji, sticker } if !sticker.as_deref().is_some_and(is_drawable) => Some(
            glyphs(isolated(emoji).unwrap_or_else(|| vec![emoji.clone()])),
        ),
        Content::Text { text, .. } if text.entities.is_empty() => isolated(&text.text).map(glyphs),
        Content::Text { text, .. } => with_custom(text),
        _ => None,
    }
}

/// A text of custom emoji only: each fallback emoji under its entity
/// (`CustomEmojiIsolated`).
fn with_custom(text: &FormattedText) -> Option<Vec<Big>> {
    let mut entities: Vec<_> = text.entities.iter().collect();
    entities.sort_by_key(|entity| entity.range.start);
    let ids: Vec<CustomEmojiId> = entities
        .iter()
        .map(|entity| match entity.kind {
            EntityKind::CustomEmoji { id } => Some(id),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let emoji = isolated(&text.text)?;
    (emoji.len() == ids.len()).then(|| {
        emoji
            .into_iter()
            .zip(ids)
            .map(|(emoji, id)| Big {
                emoji,
                custom: Some(id),
            })
            .collect()
    })
}

/// The emoji in a row, the time on its pill after them, or over their
/// corner on the right side (`calculateFullRight`).
// shortcut: a selected message keeps its emoji untinted; tdesktop colors them with msgStickerOverlay. With M8 polish.
pub(super) fn bubble(item: &Item<'_>, out: bool, emoji: Vec<Face>, cx: &App) -> Div {
    let side = px_of(LARGE_EMOJI_SIZE) + px_of(LARGE_EMOJI_OUTLINE) * 2.0;
    let skip = px_of(LARGE_EMOJI_SKIP) - px_of(LARGE_EMOJI_OUTLINE) * 2.0;
    let row = div()
        .flex()
        .gap(skip)
        .text_size(px_of(LARGE_EMOJI_SIZE))
        .line_height(side)
        .children(emoji.into_iter().map(|face| {
            let square = div().size(side).flex().justify_center();
            match face {
                Face::Glyph(emoji) => square.child(emoji),
                Face::Picture(path) => {
                    square.child(img(std::path::PathBuf::from(path)).size_full())
                }
                Face::Frame(frame) => square.child(img(frame).size_full()),
            }
        }));
    let pill = super::photo::info_pill(item, out, cx);
    div().relative().flex().items_end().child(row).map(|this| {
        if out {
            this.child(
                pill.absolute()
                    .right(px_of(MSG_DATE_IMG_DELTA))
                    .bottom(px_of(MSG_DATE_IMG_DELTA)),
            )
        } else {
            this.child(pill.mb(px_of(MSG_DATE_IMG_DELTA)))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_to_three_known_emoji_alone_are_isolated() {
        assert_eq!(isolated("😀"), Some(vec!["😀".to_owned()]));
        // The selector comes back even when the text left it out.
        assert_eq!(
            isolated(" ☺😀 "),
            Some(vec!["☺\u{fe0f}".to_owned(), "😀".to_owned()])
        );
        // A skin tone stays with its emoji.
        assert_eq!(isolated("👍🏽"), Some(vec!["👍🏽".to_owned()]));
        assert_eq!(isolated("😀😀😀😀"), None);
        assert_eq!(isolated("hi 😀"), None);
        assert_eq!(isolated(""), None);
    }

    #[test]
    fn a_text_of_custom_emoji_alone_shows_them_big() {
        use model::text::Entity;
        let custom = |start: usize, end: usize, id: i64| Entity {
            range: start..end,
            kind: EntityKind::CustomEmoji {
                id: CustomEmojiId(id),
            },
        };
        let text = FormattedText {
            text: "😀👍".to_owned(),
            entities: vec![custom(2, 4, 2), custom(0, 2, 1)],
        };
        let big = with_custom(&text).expect("two custom emoji alone");
        assert_eq!(big[0].custom, Some(CustomEmojiId(1)));
        assert_eq!(big[1].emoji, "👍");
        let bold = FormattedText {
            entities: vec![Entity {
                range: 0..2,
                kind: EntityKind::Bold,
            }],
            ..text.clone()
        };
        assert_eq!(with_custom(&bold), None);
        let words = FormattedText {
            text: "hi 😀".to_owned(),
            entities: vec![custom(3, 5, 1)],
        };
        assert_eq!(with_custom(&words), None);
    }
}
