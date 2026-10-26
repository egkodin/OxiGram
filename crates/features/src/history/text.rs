//! Message text with its entities (`Ui::Text::String`): blocks for code
//! and quotes, styled runs inside them, and what each link opens.

use std::ops::Range;

use gpui::{FontStyle, FontWeight, Hsla, StrikethroughStyle, TextRun, UnderlineStyle, px};
use model::UserId;
use model::text::{EntityKind, FormattedText};

/// A paragraph run of the text: plain, a code block or a quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Block {
    pub range: Range<usize>,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BlockKind {
    Text,
    Pre { language: Option<String> },
    Quote,
}

/// What clicking a link does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    Url(String),
    /// A link whose text hides its address, so opening it asks first.
    HiddenUrl(String),
    Email(String),
    Username(String),
    User(UserId),
    Hashtag(String),
    Spoiler,
}

/// The colors text runs take.
#[derive(Debug, Clone, Copy)]
pub(super) struct Palette {
    pub fg: Hsla,
    pub link: Hsla,
    pub mono: Hsla,
    pub spoiler: Hsla,
}

/// Splits `text` at its code blocks and quotes; text between them, with
/// the newlines around blocks dropped, forms plain blocks.
pub(super) fn blocks(text: &FormattedText) -> Vec<Block> {
    let mut special: Vec<Block> = text
        .entities
        .iter()
        .filter_map(|entity| {
            let kind = match &entity.kind {
                EntityKind::Pre { language } => BlockKind::Pre {
                    language: language.clone(),
                },
                EntityKind::BlockQuote(_) => BlockKind::Quote,
                _ => return None,
            };
            Some(Block {
                range: entity.range.clone(),
                kind,
            })
        })
        .collect();
    special.sort_by_key(|block| block.range.start);
    let mut blocks = Vec::new();
    let mut at = 0;
    for block in special {
        if block.range.start < at {
            continue;
        }
        push_text(&mut blocks, text, at..block.range.start);
        at = block.range.end;
        blocks.push(block);
    }
    push_text(&mut blocks, text, at..text.text.len());
    blocks
}

fn push_text(blocks: &mut Vec<Block>, text: &FormattedText, range: Range<usize>) {
    let part = &text.text[range.clone()];
    let start = range.start + (part.len() - part.trim_start_matches('\n').len());
    let end = range.end - (part.len() - part.trim_end_matches('\n').len());
    if start < end {
        blocks.push(Block {
            range: start..end,
            kind: BlockKind::Text,
        });
    }
}

fn is_link(kind: &EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Url
            | EntityKind::TextUrl { .. }
            | EntityKind::EmailAddress
            | EntityKind::PhoneNumber
            | EntityKind::BankCardNumber
            | EntityKind::Mention
            | EntityKind::MentionName { .. }
            | EntityKind::Hashtag
            | EntityKind::Cashtag
            | EntityKind::BotCommand
            | EntityKind::MediaTimestamp { .. }
    )
}

/// The runs for `range` of `text`, styled by the entities over each piece.
/// Unrevealed spoilers are blocks of the spoiler color.
pub(super) fn runs(
    text: &FormattedText,
    range: Range<usize>,
    font: &gpui::Font,
    mono_family: &str,
    palette: Palette,
    is_spoiler_revealed: bool,
) -> Vec<TextRun> {
    let mut cuts = vec![range.start, range.end];
    for entity in &text.entities {
        for cut in [entity.range.start, entity.range.end] {
            if range.contains(&cut) {
                cuts.push(cut);
            }
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .map(|piece| {
            let piece = piece[0]..piece[1];
            let covering = text
                .entities
                .iter()
                .filter(|entity| entity.range.start <= piece.start && piece.end <= entity.range.end)
                .map(|entity| &entity.kind);
            run(
                piece.len(),
                covering,
                font,
                mono_family,
                palette,
                is_spoiler_revealed,
            )
        })
        .collect()
}

fn run<'a>(
    len: usize,
    kinds: impl Iterator<Item = &'a EntityKind>,
    font: &gpui::Font,
    mono_family: &str,
    palette: Palette,
    is_spoiler_revealed: bool,
) -> TextRun {
    let mut run = TextRun {
        len,
        font: font.clone(),
        color: palette.fg,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut is_hidden = false;
    for kind in kinds {
        match kind {
            EntityKind::Bold => run.font.weight = FontWeight::SEMIBOLD,
            EntityKind::Italic => run.font.style = FontStyle::Italic,
            EntityKind::Underline => {
                run.underline = Some(UnderlineStyle {
                    thickness: px(1.0),
                    ..UnderlineStyle::default()
                });
            }
            EntityKind::Strikethrough => {
                run.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.0),
                    ..StrikethroughStyle::default()
                });
            }
            EntityKind::Code | EntityKind::Pre { .. } => {
                run.font.family = mono_family.to_owned().into();
                run.color = palette.mono;
            }
            EntityKind::Spoiler => is_hidden = !is_spoiler_revealed,
            kind if is_link(kind) => run.color = palette.link,
            _ => {}
        }
    }
    // shortcut: a hidden spoiler is a solid block; the particle animation comes with M8 polish.
    if is_hidden {
        run.color = palette.spoiler;
        run.background_color = Some(palette.spoiler);
    }
    run
}

/// tdesktop adds a scheme to bare addresses (`UrlClickHandler::Url`).
fn with_scheme(url: &str) -> String {
    if url.contains("://") {
        url.to_owned()
    } else {
        format!("https://{url}")
    }
}

/// The clickable ranges inside `range`, shifted to start at 0, with what
/// each opens.
// shortcut: phone numbers, bank cards, cashtags, bot commands and timestamps look like links but do nothing until sending (M4) and media (M5).
pub(super) fn links(
    text: &FormattedText,
    range: Range<usize>,
    is_spoiler_revealed: bool,
) -> Vec<(Range<usize>, Link)> {
    text.entities
        .iter()
        .filter(|entity| range.start <= entity.range.start && entity.range.end <= range.end)
        .filter_map(|entity| {
            let shown = &text.text[entity.range.clone()];
            let link = match &entity.kind {
                EntityKind::Spoiler if !is_spoiler_revealed => Link::Spoiler,
                EntityKind::Url => Link::Url(with_scheme(shown)),
                EntityKind::TextUrl { url } if url == shown => Link::Url(url.clone()),
                EntityKind::TextUrl { url } => Link::HiddenUrl(url.clone()),
                EntityKind::EmailAddress => Link::Email(shown.to_owned()),
                EntityKind::Mention => Link::Username(shown.trim_start_matches('@').to_owned()),
                EntityKind::MentionName { user } => Link::User(*user),
                EntityKind::Hashtag => Link::Hashtag(shown.to_owned()),
                _ => return None,
            };
            let shifted = entity.range.start - range.start..entity.range.end - range.start;
            Some((shifted, link))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::text::{Entity, Quote};

    fn text(text: &str, entities: &[(Range<usize>, EntityKind)]) -> FormattedText {
        FormattedText {
            text: text.to_owned(),
            entities: entities
                .iter()
                .map(|(range, kind)| Entity {
                    range: range.clone(),
                    kind: kind.clone(),
                })
                .collect(),
        }
    }

    #[test]
    fn code_blocks_and_quotes_split_the_text() {
        let message = text(
            "intro\nfn main() {}\nmiddle\nquoted\n",
            &[
                (
                    6..18,
                    EntityKind::Pre {
                        language: Some("rust".to_owned()),
                    },
                ),
                (26..32, EntityKind::BlockQuote(Quote::Plain)),
                (0..5, EntityKind::Bold),
            ],
        );
        let kinds: Vec<_> = blocks(&message)
            .into_iter()
            .map(|block| (message.text[block.range].to_owned(), block.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("intro".to_owned(), BlockKind::Text),
                (
                    "fn main() {}".to_owned(),
                    BlockKind::Pre {
                        language: Some("rust".to_owned())
                    }
                ),
                ("middle".to_owned(), BlockKind::Text),
                ("quoted".to_owned(), BlockKind::Quote),
            ]
        );
    }

    #[test]
    fn nested_entities_combine_in_each_piece() {
        let message = text(
            "bold and italic",
            &[(0..15, EntityKind::Bold), (9..15, EntityKind::Italic)],
        );
        let palette = Palette {
            fg: Hsla::default(),
            link: Hsla::default(),
            mono: Hsla::default(),
            spoiler: Hsla::default(),
        };
        let runs = runs(
            &message,
            0..15,
            &gpui::font("Open Sans"),
            "Menlo",
            palette,
            false,
        );
        assert_eq!(runs.iter().map(|run| run.len).collect::<Vec<_>>(), [9, 6]);
        assert_eq!(runs[0].font.weight, FontWeight::SEMIBOLD);
        assert_eq!(runs[0].font.style, FontStyle::Normal);
        assert_eq!(runs[1].font.style, FontStyle::Italic);
    }

    #[test]
    fn links_open_what_they_show_or_ask_first() {
        let message = text(
            "see t.me or docs",
            &[
                (4..8, EntityKind::Url),
                (
                    12..16,
                    EntityKind::TextUrl {
                        url: "https://example.org".to_owned(),
                    },
                ),
            ],
        );
        assert_eq!(
            links(&message, 0..16, false),
            [
                (4..8, Link::Url("https://t.me".to_owned())),
                (12..16, Link::HiddenUrl("https://example.org".to_owned())),
            ]
        );
    }
}
