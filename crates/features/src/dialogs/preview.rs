//! The one-line message preview under a chat's name (`HistoryItem::toPreview`
//! and `MessageView` in tdesktop).

use std::ops::Range;

use i18n::{Key, Strings, key};
use model::chat::{Chat, ChatKind, Draft};
use model::chats::Chats;
use model::file::Minithumbnail;
use model::message::{Content, Media, Message, Sender};
use model::text::{Entity, EntityKind, FormattedText};

/// Preview text with the few styles the chat list keeps
/// (`DialogsPreviewText`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Preview {
    pub text: String,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    pub range: Range<usize>,
    pub kind: SpanKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpanKind {
    /// The sender, the media type or "Draft:", in the palette's link color.
    Colorized,
    /// Unread chat names in the archive row.
    Semibold,
    Italic,
    Underline,
    Strikethrough,
    Code,
    Spoiler,
}

impl Preview {
    /// The text with its spoilers covered, for lines that draw plain text.
    // shortcut: a spoiler becomes ░ marks; the particle animation comes with M3's spoilers.
    pub(crate) fn hiding_spoilers(self) -> String {
        let mut spoilers: Vec<_> = self
            .spans
            .iter()
            .filter(|span| span.kind == SpanKind::Spoiler)
            .map(|span| span.range.clone())
            .collect();
        // From the end, so the ranges before stay where they were.
        spoilers.sort_by_key(|range| std::cmp::Reverse(range.start));
        let mut text = self.text;
        for range in spoilers {
            let Some(hidden) = text.get(range.clone()) else {
                continue;
            };
            let covered: String = hidden
                .chars()
                .map(|c| if c.is_whitespace() { c } else { '░' })
                .collect();
            text.replace_range(range, &covered);
        }
        text
    }

    fn colorized(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            spans: vec![Span {
                range: 0..text.len(),
                kind: SpanKind::Colorized,
            }],
        }
    }

    /// Message text on one line, with only the styles the list shows.
    fn of_text(text: &FormattedText) -> Self {
        let spans = text
            .entities
            .iter()
            .filter_map(|entity| {
                let kind = match entity.kind {
                    EntityKind::Italic => SpanKind::Italic,
                    EntityKind::Underline => SpanKind::Underline,
                    EntityKind::Strikethrough => SpanKind::Strikethrough,
                    EntityKind::Code | EntityKind::Pre { .. } => SpanKind::Code,
                    EntityKind::Spoiler => SpanKind::Spoiler,
                    _ => return None,
                };
                Some(Span {
                    range: entity.range.clone(),
                    kind,
                })
            })
            .collect();
        Self {
            // The same byte length, so the spans stay valid.
            text: text.text.replace(['\n', '\r'], " "),
            spans,
        }
    }

    /// `self` after `prefix` and a space (`lng_dialogs_text_with_from`).
    fn after(self, prefix: Self) -> Self {
        let shift = prefix.text.len() + 1;
        let mut spans = prefix.spans;
        spans.extend(self.spans.into_iter().map(|span| Span {
            range: span.range.start + shift..span.range.end + shift,
            kind: span.kind,
        }));
        Self {
            text: format!("{} {}", prefix.text, self.text),
            spans,
        }
    }
}

/// Where the unsupported notice sends the user (`UnsupportedMessageText`).
// shortcut: tdesktop's own download page, as tdesktop links it; an rgram releases page once releases are published (owner's call).
const UNSUPPORTED_LINK: &str = "https://desktop.telegram.org";

/// The notice in place of a message TDLib cannot show: italic, with the
/// download page as a link.
pub(crate) fn unsupported_text(strings: &Strings) -> FormattedText {
    let template = strings.text(key::lng_message_unsupported);
    let text = i18n::fill(template, &[("link", UNSUPPORTED_LINK)]);
    let mut entities = vec![Entity {
        range: 0..text.len(),
        kind: EntityKind::Italic,
    }];
    if let Some(start) = text.find(UNSUPPORTED_LINK) {
        entities.push(Entity {
            range: start..start + UNSUPPORTED_LINK.len(),
            kind: EntityKind::Url,
        });
    }
    FormattedText { text, entities }
}

/// The preview of a message's content, and the image shown before it.
pub(crate) fn content<'a>(
    strings: &Strings,
    content: &'a Content,
) -> (Preview, Option<&'a Minithumbnail>) {
    let typed = |key: Key, caption: &FormattedText| with_caption(strings.text(key), caption);
    let media = |kind: &str, media: &'a Media| match &media.thumbnail {
        // With an image the caption stands alone (`hasMiniImages`).
        Some(thumbnail) if !media.caption.text.is_empty() => {
            (Preview::of_text(&media.caption), Some(thumbnail))
        }
        thumbnail => (with_caption(kind, &media.caption), thumbnail.as_ref()),
    };
    let empty = FormattedText::default();
    let preview = match content {
        Content::Text { text, .. } => Preview::of_text(text),
        Content::Photo(photo) => return media(strings.text(key::lng_in_dlg_photo), photo),
        Content::Video(video) => return media(strings.text(key::lng_in_dlg_video), video),
        Content::Animation(animation) => return media("GIF", animation),
        Content::Document { name, media: file } => {
            let kind = if name.is_empty() {
                strings.text(key::lng_in_dlg_file)
            } else {
                name.as_str()
            };
            return media(kind, file);
        }
        Content::Audio { name, caption } => {
            if name.is_empty() {
                typed(key::lng_in_dlg_audio_file, caption)
            } else {
                with_caption(name, caption)
            }
        }
        Content::Voice { caption, .. } => typed(key::lng_in_dlg_audio, caption),
        Content::VideoNote => typed(key::lng_in_dlg_video_message, &empty),
        Content::Sticker { emoji } if emoji.is_empty() => {
            Preview::colorized(strings.text(key::lng_in_dlg_sticker))
        }
        Content::Sticker { emoji } => Preview::colorized(&i18n::fill(
            strings.text(key::lng_in_dlg_sticker_emoji),
            &[("emoji", emoji)],
        )),
        Content::Emoji(emoji) => Preview::of_text(&FormattedText::plain(emoji.clone())),
        Content::Contact { .. } => typed(key::lng_in_dlg_contact, &empty),
        Content::Location(_) => typed(key::lng_maps_point, &empty),
        Content::LiveLocation(_) => typed(key::lng_live_location, &empty),
        Content::Venue { title, .. } => {
            typed(key::lng_maps_point, &FormattedText::plain(title.clone()))
        }
        // shortcut: no poll, game and invoice mini icons; they come with custom emoji in M5.
        Content::Poll { question } => Preview::of_text(question),
        Content::Game { title } | Content::Invoice { title } => {
            Preview::of_text(&FormattedText::plain(title.clone()))
        }
        Content::Story => typed(key::lng_in_dlg_story, &empty),
        Content::Call {
            is_video,
            is_missed,
        } => typed(call_key(*is_video, *is_missed), &empty),
        Content::Unsupported => Preview::of_text(&unsupported_text(strings)),
        // Rows build service texts with the chat at hand (`preview::service`).
        _ => Preview::default(),
    };
    (preview, None)
}

// shortcut: an outgoing or incoming missed call differs only by "Missed"; tdesktop's declined/cancelled wording comes with calls in M12.
fn call_key(is_video: bool, is_missed: bool) -> Key {
    match (is_video, is_missed) {
        (false, false) => key::lng_call_outgoing,
        (false, true) => key::lng_call_missed,
        (true, false) => key::lng_call_video_outgoing,
        (true, true) => key::lng_call_video_missed,
    }
}

/// `WithCaptionNotificationText`: "Photo" alone, or "Photo, caption" with
/// the type colorized.
fn with_caption(kind: &str, caption: &FormattedText) -> Preview {
    if caption.text.is_empty() {
        return Preview::colorized(kind);
    }
    let wrapped = format!("{kind},");
    let mut preview = Preview::of_text(caption).after(Preview::colorized(&wrapped));
    // Only the type itself is colorized, not the comma.
    if let Some(first) = preview.spans.first_mut() {
        first.range = 0..kind.len();
    }
    preview
}

/// Who the preview names before the text: other members in groups,
/// "You" for the user's own messages there, nobody in private chats and
/// channels.
pub(crate) fn sender(
    strings: &Strings,
    chats: &Chats,
    chat: &Chat,
    message: &Message,
) -> Option<String> {
    match chat.kind {
        ChatKind::Private { .. }
        | ChatKind::Secret { .. }
        | ChatKind::Supergroup {
            is_channel: true, ..
        } => return None,
        ChatKind::Group { .. }
        | ChatKind::Supergroup {
            is_channel: false, ..
        } => {}
    }
    if message.is_outgoing {
        return Some(strings.text(key::lng_from_you).to_owned());
    }
    match message.sender {
        Sender::User(user) => chats.user(user).map(|user| short_name(strings, user)),
        Sender::Chat(id) => chats.chat(id).map(|chat| chat.title.clone()),
    }
}

/// The first name, as tdesktop's `shortName()`.
pub(crate) fn short_name(strings: &Strings, user: &model::user::User) -> String {
    if user.kind == model::user::UserKind::Deleted {
        strings.text(key::lng_deleted).to_owned()
    } else if user.first_name.is_empty() {
        user.last_name.clone()
    } else {
        user.first_name.clone()
    }
}

/// The last message with its sender, as the row shows it.
pub(crate) fn message<'a>(
    strings: &Strings,
    sender: Option<&str>,
    message: &'a Message,
) -> (Preview, Option<&'a Minithumbnail>) {
    let (preview, thumbnail) = content(strings, &message.content);
    let Some(sender) = sender else {
        return (preview, thumbnail);
    };
    let from = i18n::fill(
        strings.text(key::lng_dialogs_text_from_wrapped),
        &[("from", sender)],
    );
    (preview.after(Preview::colorized(&from)), thumbnail)
}

/// A service message's text, all in the service color
/// (`HistoryService::toPreview`).
pub(crate) fn service(text: &str) -> Preview {
    Preview::colorized(text)
}

/// "Draft: text", with "Draft:" in the draft color.
pub(crate) fn draft(strings: &Strings, draft: &Draft) -> Preview {
    let from = i18n::fill(
        strings.text(key::lng_dialogs_text_from_wrapped),
        &[("from", strings.text(key::lng_from_draft))],
    );
    Preview::of_text(&draft.text).after(Preview::colorized(&from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::text::Entity;
    use model::{ChatId, MessageId, UserId};

    #[test]
    fn plain_lines_cover_spoilers() {
        let spoiler = |range| Span {
            range,
            kind: SpanKind::Spoiler,
        };
        let preview = Preview {
            text: "seats 12 and 13, gate é".to_owned(),
            spans: vec![spoiler(6..15), spoiler(22..24)],
        };
        assert_eq!(preview.hiding_spoilers(), "seats ░░ ░░░ ░░, gate ░");
    }

    fn english() -> Strings {
        Strings::english()
    }

    fn message_with(content: Content) -> Message {
        Message::new(MessageId(1), ChatId(1), Sender::User(UserId(2)), content)
    }

    #[test]
    fn photo_with_a_caption_and_no_image_reads_type_then_caption() {
        let content = Content::Photo(Media {
            caption: FormattedText::plain("At the sea"),
            thumbnail: None,
            dimensions: None,
            preview: None,
            duration: 0,
            file: None,
            has_spoiler: false,
        });
        let (preview, thumbnail) = super::content(&english(), &content);
        assert_eq!(preview.text, "Photo, At the sea");
        assert_eq!(preview.spans[0].range, 0..5);
        assert!(thumbnail.is_none());
    }

    #[test]
    fn a_mini_image_replaces_the_type() {
        let thumbnail = Minithumbnail {
            width: 1,
            height: 1,
            jpeg: vec![1],
        };
        let content = Content::Photo(Media {
            caption: FormattedText::plain("At the sea"),
            thumbnail: Some(thumbnail.clone()),
            dimensions: None,
            preview: None,
            duration: 0,
            file: None,
            has_spoiler: false,
        });
        let (preview, image) = super::content(&english(), &content);
        assert_eq!(preview.text, "At the sea");
        assert_eq!(image, Some(&thumbnail));
    }

    #[test]
    fn sender_prefix_shifts_the_text_styles() {
        let text = FormattedText {
            text: "hi\nthere".to_owned(),
            entities: vec![
                Entity {
                    range: 0..2,
                    kind: EntityKind::Italic,
                },
                Entity {
                    range: 3..8,
                    kind: EntityKind::Bold,
                },
            ],
        };
        let message = message_with(Content::text(text));
        let (preview, _) = super::message(&english(), Some("Ann"), &message);
        assert_eq!(preview.text, "Ann: hi there");
        assert_eq!(
            preview.spans,
            [
                Span {
                    range: 0..4,
                    kind: SpanKind::Colorized
                },
                Span {
                    range: 5..7,
                    kind: SpanKind::Italic
                },
            ]
        );
    }

    #[test]
    fn drafts_are_prefixed() {
        let draft = Draft {
            date: 0,
            text: FormattedText::plain("later"),
        };
        assert_eq!(super::draft(&english(), &draft).text, "Draft: later");
    }

    #[test]
    fn stickers_show_their_emoji() {
        let (preview, _) = super::content(
            &english(),
            &Content::Sticker {
                emoji: "😀".to_owned(),
            },
        );
        assert_eq!(preview.text, "😀 Sticker");
    }

    #[test]
    fn the_unsupported_notice_is_italic_with_its_link() {
        let text = unsupported_text(&Strings::english());
        let link = text
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Url)
            .map(|entity| &text.text[entity.range.clone()]);
        assert_eq!(link, Some(UNSUPPORTED_LINK));
        assert_eq!(text.entities[0].range, 0..text.text.len());
        assert!(!text.text.contains("{link}"));
    }
}
