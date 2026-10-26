//! Messages as the chat list previews them and the history shows them.

use td_api::enums::{
    CallDiscardReason, MessageContent, MessageOrigin, MessageReplyTo, MessageSender,
    MessageSendingState,
};

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
    /// Unix time of the last edit; 0 when never edited.
    pub edit_date: i32,
    pub content: Content,
    pub reply_to: Option<ReplyTo>,
    pub forward: Option<Forward>,
    /// Set while an outgoing message is not on the server yet.
    pub sending: Option<Sending>,
    /// Channel post views; 0 when not counted.
    pub views: i32,
    /// The post author's name in channels that sign posts.
    pub author_signature: String,
    /// Messages of one album share it.
    pub album: Option<i64>,
    pub is_channel_post: bool,
}

/// The message a message replies to.
// shortcut: replies to stories and the replied message's origin and content in other chats are dropped; external replies show as "deleted message" until M5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTo {
    pub chat: ChatId,
    pub message: MessageId,
    /// The quoted part of the replied message, when the sender picked one.
    pub quote: Option<FormattedText>,
}

/// Where a forwarded message first came from, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forward {
    pub origin: Origin,
    pub date: i32,
}

/// Who first sent a forwarded message (`MessageOrigin`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    User(UserId),
    /// A user who hides their account in forwards; only the name is known.
    HiddenUser(String),
    Chat {
        chat: ChatId,
        signature: String,
    },
    Channel {
        chat: ChatId,
        message: MessageId,
        signature: String,
    },
}

/// Where an outgoing message is on its way to the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sending {
    Pending,
    Failed,
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
    Text {
        text: FormattedText,
        preview: Option<Box<LinkPreview>>,
    },
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
    /// A service message ("X joined the group").
    Service(Service),
    Unsupported,
}

/// The page a text message links to, as its preview shows it
/// (`linkPreview`).
// shortcut: no photo, video or type-specific button yet; media come with M5.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkPreview {
    pub url: String,
    pub site_name: String,
    pub title: String,
    pub description: FormattedText,
    /// Shown above the message text instead of below it.
    pub is_above_text: bool,
    /// The text shows the URL, so opening it asks nothing.
    pub is_url_in_text: bool,
}

impl From<td_api::types::LinkPreview> for LinkPreview {
    fn from(preview: td_api::types::LinkPreview) -> Self {
        Self {
            url: preview.url,
            site_name: preview.site_name,
            title: preview.title,
            description: FormattedText::from(*preview.description),
            is_above_text: preview.show_above_text,
            is_url_in_text: preview.skip_confirmation,
        }
    }
}

impl Content {
    /// A text message without a link preview.
    #[must_use]
    pub fn text(text: FormattedText) -> Self {
        Self::Text {
            text,
            preview: None,
        }
    }
}

/// What a service message reports (`HistoryService::setServiceText`).
// shortcut: the most common actions; gifts, payments, giveaways, topics, boosts and the rest show as `Other` until their milestones (M9-M13).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Service {
    GroupCreated {
        title: String,
    },
    /// A supergroup or channel created; which one the chat tells.
    SupergroupCreated {
        title: String,
    },
    TitleChanged {
        title: String,
    },
    PhotoChanged,
    PhotoDeleted,
    MembersAdded(Vec<UserId>),
    JoinedByLink,
    JoinedByRequest,
    MemberRemoved(UserId),
    /// The basic group became a supergroup (`lng_action_group_migrate`).
    Upgraded,
    Pinned(MessageId),
    ScreenshotTaken,
    ContactRegistered,
    /// Messages now auto-delete after this many seconds; 0 turned it off.
    AutoDelete {
        seconds: i32,
    },
    VideoChatStarted,
    VideoChatEnded {
        seconds: i32,
    },
    /// Text a bot or the server set.
    Custom(String),
    Other,
}

impl Service {
    fn from_td(content: MessageContent) -> Option<Self> {
        let users = |ids: Vec<i64>| ids.into_iter().map(UserId).collect();
        Some(match content {
            MessageContent::MessageBasicGroupChatCreate(created) => Self::GroupCreated {
                title: created.title,
            },
            MessageContent::MessageSupergroupChatCreate(created) => Self::SupergroupCreated {
                title: created.title,
            },
            MessageContent::MessageChatChangeTitle(changed) => Self::TitleChanged {
                title: changed.title,
            },
            MessageContent::MessageChatChangePhoto(_) => Self::PhotoChanged,
            MessageContent::MessageChatDeletePhoto(_) => Self::PhotoDeleted,
            MessageContent::MessageChatAddMembers(added) => {
                Self::MembersAdded(users(added.member_user_ids))
            }
            MessageContent::MessageChatJoinByLink(_) => Self::JoinedByLink,
            MessageContent::MessageChatJoinByRequest(_) => Self::JoinedByRequest,
            MessageContent::MessageChatDeleteMember(removed) => {
                Self::MemberRemoved(UserId(removed.user_id))
            }
            MessageContent::MessageChatUpgradeTo(_) | MessageContent::MessageChatUpgradeFrom(_) => {
                Self::Upgraded
            }
            MessageContent::MessagePinMessage(pin) => Self::Pinned(MessageId(pin.message_id)),
            MessageContent::MessageScreenshotTaken(_) => Self::ScreenshotTaken,
            MessageContent::MessageContactRegistered(_) => Self::ContactRegistered,
            MessageContent::MessageChatSetMessageAutoDeleteTime(time) => Self::AutoDelete {
                seconds: time.message_auto_delete_time,
            },
            MessageContent::MessageVideoChatStarted(_) => Self::VideoChatStarted,
            MessageContent::MessageVideoChatEnded(ended) => Self::VideoChatEnded {
                seconds: ended.duration,
            },
            MessageContent::MessageCustomServiceAction(action) => Self::Custom(action.text),
            _ => return None,
        })
    }
}

/// A photo, video or file with its caption and a blurred preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Media {
    pub caption: FormattedText,
    pub thumbnail: Option<Minithumbnail>,
}

impl Message {
    /// An incoming message with nothing but its content, as fixtures build
    /// them.
    #[must_use]
    pub fn new(id: MessageId, chat: ChatId, sender: Sender, content: Content) -> Self {
        Self {
            id,
            chat,
            sender,
            is_outgoing: false,
            date: 0,
            edit_date: 0,
            content,
            reply_to: None,
            forward: None,
            sending: None,
            views: 0,
            author_signature: String::new(),
            album: None,
            is_channel_post: false,
        }
    }
}

impl From<td_api::types::Message> for Message {
    fn from(message: td_api::types::Message) -> Self {
        Self {
            id: MessageId(message.id),
            chat: ChatId(message.chat_id),
            sender: Sender::from(*message.sender_id),
            is_outgoing: message.is_outgoing,
            date: message.date,
            edit_date: message.edit_date,
            content: Content::from(*message.content),
            reply_to: message.reply_to.and_then(|reply| ReplyTo::from_td(*reply)),
            forward: message.forward_info.map(|forward| Forward {
                origin: Origin::from(*forward.origin),
                date: forward.date,
            }),
            sending: message.sending_state.map(|state| match *state {
                MessageSendingState::Pending(_) => Sending::Pending,
                MessageSendingState::Failed(_) => Sending::Failed,
            }),
            views: message.interaction_info.map_or(0, |info| info.view_count),
            author_signature: message.author_signature,
            album: Some(message.media_album_id).filter(|album| *album != 0),
            is_channel_post: message.is_channel_post,
        }
    }
}

impl ReplyTo {
    fn from_td(reply: MessageReplyTo) -> Option<Self> {
        match reply {
            MessageReplyTo::Message(reply) => Some(Self {
                chat: ChatId(reply.chat_id),
                message: MessageId(reply.message_id),
                quote: reply.quote.map(|quote| FormattedText::from(*quote.text)),
            }),
            MessageReplyTo::Story(_) => None,
        }
    }
}

impl From<MessageOrigin> for Origin {
    fn from(origin: MessageOrigin) -> Self {
        match origin {
            MessageOrigin::User(user) => Self::User(UserId(user.sender_user_id)),
            MessageOrigin::HiddenUser(user) => Self::HiddenUser(user.sender_name),
            MessageOrigin::Chat(chat) => Self::Chat {
                chat: ChatId(chat.sender_chat_id),
                signature: chat.author_signature,
            },
            MessageOrigin::Channel(channel) => Self::Channel {
                chat: ChatId(channel.chat_id),
                message: MessageId(channel.message_id),
                signature: channel.author_signature,
            },
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
            MessageContent::MessageText(text) => Self::Text {
                text: FormattedText::from(*text.text),
                preview: text
                    .link_preview
                    .map(|preview| Box::new(LinkPreview::from(*preview))),
            },
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
            other => Self::Service(Service::from_td(other).unwrap_or(Service::Other)),
        }
    }
}
