//! Messages as the chat list previews them. The full message model lands
//! with the history in M3.

use td_api::enums::{CallDiscardReason, MessageContent, MessageSender};

use crate::file::Minithumbnail;
use crate::ids::{ChatId, MessageId, UserId};
use crate::text::FormattedText;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub chat: ChatId,
    pub sender: Sender,
    pub is_outgoing: bool,
    /// Unix time the message was sent.
    pub date: i32,
    pub content: Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sender {
    User(UserId),
    Chat(ChatId),
}

/// What a message holds, reduced to what its preview shows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Content {
    Text(FormattedText),
    Photo(Media),
    Video(Media),
    /// A GIF.
    Animation(Media),
    Document {
        name: String,
        media: Media,
    },
    Audio {
        /// The file name; may be empty.
        name: String,
        caption: FormattedText,
    },
    Voice {
        caption: FormattedText,
    },
    VideoNote,
    Sticker {
        emoji: String,
    },
    /// An emoji sent alone, shown big; also dice.
    Emoji(String),
    Contact {
        name: String,
    },
    Location,
    LiveLocation,
    Venue {
        title: String,
    },
    Poll {
        question: FormattedText,
    },
    Game {
        title: String,
    },
    Invoice {
        title: String,
    },
    Story,
    Call {
        is_video: bool,
        is_missed: bool,
    },
    /// A service message ("X joined the group"); its text comes with M3.
    // shortcut: previews of service messages show a generic text; M3 builds each service text.
    Service,
    Unsupported,
}

/// A photo, video or file with its caption and a blurred preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Media {
    pub caption: FormattedText,
    pub thumbnail: Option<Minithumbnail>,
}

impl From<td_api::types::Message> for Message {
    fn from(message: td_api::types::Message) -> Self {
        Self {
            id: MessageId(message.id),
            chat: ChatId(message.chat_id),
            sender: Sender::from(*message.sender_id),
            is_outgoing: message.is_outgoing,
            date: message.date,
            content: Content::from(*message.content),
        }
    }
}

impl From<MessageSender> for Sender {
    fn from(sender: MessageSender) -> Self {
        match sender {
            MessageSender::User(user) => Self::User(UserId(user.user_id)),
            MessageSender::Chat(chat) => Self::Chat(ChatId(chat.chat_id)),
        }
    }
}

fn media(
    caption: td_api::types::FormattedText,
    thumbnail: Option<Box<td_api::types::Minithumbnail>>,
) -> Media {
    Media {
        caption: FormattedText::from(caption),
        thumbnail: thumbnail.and_then(|thumbnail| Minithumbnail::from_td(*thumbnail)),
    }
}

impl From<MessageContent> for Content {
    fn from(content: MessageContent) -> Self {
        match content {
            MessageContent::MessageText(text) => Self::Text(FormattedText::from(*text.text)),
            MessageContent::MessagePhoto(photo) => {
                Self::Photo(media(*photo.caption, photo.photo.minithumbnail))
            }
            MessageContent::MessageVideo(video) => {
                Self::Video(media(*video.caption, video.video.minithumbnail))
            }
            MessageContent::MessageAnimation(animation) => {
                Self::Animation(media(*animation.caption, animation.animation.minithumbnail))
            }
            MessageContent::MessageDocument(document) => Self::Document {
                media: media(*document.caption, document.document.minithumbnail),
                name: document.document.file_name,
            },
            MessageContent::MessageAudio(audio) => Self::Audio {
                name: audio.audio.file_name,
                caption: FormattedText::from(*audio.caption),
            },
            MessageContent::MessageVoiceNote(voice) => Self::Voice {
                caption: FormattedText::from(*voice.caption),
            },
            MessageContent::MessageVideoNote(_) => Self::VideoNote,
            MessageContent::MessageSticker(sticker) => Self::Sticker {
                emoji: sticker.sticker.emoji,
            },
            MessageContent::MessageAnimatedEmoji(emoji) => Self::Emoji(emoji.emoji),
            MessageContent::MessageDice(dice) => Self::Emoji(dice.emoji),
            MessageContent::MessageContact(contact) => Self::Contact {
                name: [contact.contact.first_name, contact.contact.last_name]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            },
            MessageContent::MessageLocation(_) => Self::Location,
            MessageContent::MessageLiveLocation(_) => Self::LiveLocation,
            MessageContent::MessageVenue(venue) => Self::Venue {
                title: venue.venue.title,
            },
            MessageContent::MessagePoll(poll) => Self::Poll {
                question: FormattedText::from(*poll.poll.question),
            },
            MessageContent::MessageGame(game) => Self::Game {
                title: game.game.title,
            },
            MessageContent::MessageInvoice(invoice) => Self::Invoice {
                title: invoice.product_info.title,
            },
            MessageContent::MessageStory(_) => Self::Story,
            MessageContent::MessageCall(call) => Self::Call {
                is_video: call.is_video,
                is_missed: matches!(*call.discard_reason, CallDiscardReason::Missed(_)),
            },
            MessageContent::MessageUnsupported(_) => Self::Unsupported,
            _ => Self::Service,
        }
    }
}
