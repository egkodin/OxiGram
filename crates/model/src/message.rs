//! Messages as the chat list previews them and the history shows them.

use td_api::enums::{
    CallDiscardReason, DiceStickers, MessageContent, MessageOrigin, MessageReplyTo,
    MessageSchedulingState, MessageSelfDestructType, MessageSender, MessageSendingState,
    MessageTopic,
};

use crate::file::{Dimensions, File, Minithumbnail};
use crate::ids::{ChatId, MessageId, SavedTopicId, UserId};
use crate::location::Point;
use crate::poll::Poll;
use crate::reaction::Reactions;
use crate::secret::Secret;
use crate::sticker::StickerPicture;
use crate::text::FormattedText;

mod convert;

pub(crate) use convert::interaction;

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
    /// The inline bot the message was sent through.
    pub via_bot: Option<UserId>,
    /// False in chats with protected content: no copying or forwarding.
    pub can_be_saved: bool,
    /// The thread or Saved Messages topic it belongs to.
    pub topic: Option<Topic>,
    /// Comments under a channel post, replies to a group message.
    pub replies: Option<Replies>,
    pub reactions: Option<Reactions>,
    /// Set while it waits in the chat's scheduled messages.
    pub schedule: Option<Schedule>,
    /// Set on view-once and timed media (`ttl_seconds`).
    pub self_destruct: Option<SelfDestruct>,
}

/// When opened media goes (`MessageSelfDestructType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfDestruct {
    /// Once viewed and closed.
    Immediately,
    /// This many seconds after it was opened.
    Timer(i32),
}

impl From<MessageSelfDestructType> for SelfDestruct {
    fn from(kind: MessageSelfDestructType) -> Self {
        match kind {
            MessageSelfDestructType::Immediately(_) => Self::Immediately,
            MessageSelfDestructType::Timer(timer) => Self::Timer(timer.self_destruct_time),
        }
    }
}

/// When a scheduled message goes (`messageSchedulingState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// At this Unix time.
    At(i32),
    /// Once the other user comes online.
    WhenOnline,
}

impl Schedule {
    fn from_td(state: MessageSchedulingState) -> Option<Self> {
        match state {
            MessageSchedulingState::SendAtDate(state) => Some(Self::At(state.send_date)),
            MessageSchedulingState::SendWhenOnline(_) => Some(Self::WhenOnline),
            MessageSchedulingState::SendWhenVideoProcessed(state) => {
                Some(Self::At(state.send_date))
            }
        }
    }
}

/// A part of a chat with a history of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic {
    /// Replies to the message: a channel post's comments in its
    /// discussion group, or a reply thread in a group.
    Thread(MessageId),
    /// The messages saved from one chat.
    Saved(SavedTopicId),
}

/// Replies to a message (`messageReplyInfo`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Replies {
    pub count: i32,
    /// Up to three who replied last.
    pub recent: Vec<Sender>,
    pub last_read: MessageId,
    pub last: MessageId,
}

impl Replies {
    /// Whether replies came after the last one read; never before the
    /// user read any (`HistoryItem::areCommentsUnread`).
    #[must_use]
    pub fn has_unread(&self) -> bool {
        self.last_read.0 > 1 && self.last > self.last_read
    }
}

impl From<td_api::types::MessageReplyInfo> for Replies {
    fn from(info: td_api::types::MessageReplyInfo) -> Self {
        Self {
            count: info.reply_count,
            recent: info
                .recent_replier_ids
                .into_iter()
                .map(Sender::from)
                .collect(),
            last_read: MessageId(info.last_read_inbox_message_id),
            last: MessageId(info.last_message_id),
        }
    }
}

impl Topic {
    fn from_td(topic: MessageTopic) -> Option<Self> {
        match topic {
            MessageTopic::Thread(thread) => Some(Self::Thread(MessageId(thread.message_thread_id))),
            MessageTopic::SavedMessages(saved) => {
                Some(Self::Saved(SavedTopicId(saved.saved_messages_topic_id)))
            }
            // shortcut: forum topics and channel direct messages are not told apart; with forums (M6).
            MessageTopic::Forum(_) | MessageTopic::DirectMessages(_) => None,
        }
    }
}

/// The message a message replies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTo {
    /// 0 when TDLib does not know the chat.
    pub chat: ChatId,
    /// 0 for stories and unknown chats.
    pub message: MessageId,
    /// The quoted part of the replied message, when the sender picked one.
    pub quote: Option<FormattedText>,
    /// What TDLib tells about a reply to another chat or to a story.
    pub external: Option<Box<External>>,
}

/// A replied message the user may not be able to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum External {
    /// A message in another chat: who sent it and its media, if any.
    Message {
        origin: Origin,
        media: Option<Box<Content>>,
    },
    /// A story of this chat's poster.
    Story { poster: ChatId },
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
        music: Box<Music>,
    },
    Voice {
        caption: FormattedText,
        note: Box<VoiceNote>,
    },
    /// A round video message; `is_viewed` once someone played it.
    VideoNote {
        media: Box<Media>,
        is_viewed: bool,
    },
    Sticker {
        emoji: String,
        /// `None` only in fixtures that show the chat list.
        picture: Option<Box<StickerPicture>>,
    },
    /// An emoji sent alone, shown big, or a die thrown; with the sticker
    /// that draws it, when TDLib has one.
    Emoji {
        emoji: String,
        sticker: Option<Box<StickerPicture>>,
    },
    Contact(Box<SharedContact>),
    Location(Point),
    LiveLocation(Point),
    Venue {
        point: Point,
        title: String,
        address: String,
    },
    Poll(Box<Poll>),
    Checklist(Box<crate::checklist::Checklist>),
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

/// A contact card sent in a message (`messageContact`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedContact {
    pub first_name: String,
    pub last_name: String,
    /// Digits only, as TDLib gives them; may be empty.
    pub phone: Secret,
    /// The Telegram account behind the card, when it has one.
    pub user: Option<UserId>,
    pub has_vcard: bool,
}

impl SharedContact {
    /// The first and last name, as tdesktop joins them (`langFullName`).
    #[must_use]
    pub fn name(&self) -> String {
        [self.first_name.as_str(), self.last_name.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
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
    /// The same media with its caption dropped, as reply previews of
    /// messages elsewhere show it (`hideCaption`).
    #[must_use]
    pub fn without_caption(&self) -> Self {
        let bare = |media: &Media| Media {
            caption: FormattedText::default(),
            ..media.clone()
        };
        match self {
            Self::Photo(media) => Self::Photo(bare(media)),
            Self::Video(media) => Self::Video(bare(media)),
            Self::Animation(media) => Self::Animation(bare(media)),
            Self::Document { name, media } => Self::Document {
                name: name.clone(),
                media: bare(media),
            },
            Self::Audio { name, music, .. } => Self::Audio {
                name: name.clone(),
                caption: FormattedText::default(),
                music: music.clone(),
            },
            Self::Voice { note, .. } => Self::Voice {
                caption: FormattedText::default(),
                note: note.clone(),
            },
            Self::VideoNote { media, .. } => Self::VideoNote {
                media: media.clone(),
                is_viewed: true,
            },
            other => other.clone(),
        }
    }

    /// The content once played (`updateMessageContentOpened`).
    #[must_use]
    pub fn opened(&self) -> Self {
        match self {
            Self::Voice { caption, note } => Self::Voice {
                caption: caption.clone(),
                note: Box::new(VoiceNote {
                    is_listened: true,
                    ..VoiceNote::clone(note)
                }),
            },
            other => other.clone(),
        }
    }

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
    /// View-once or timed media gone after it was opened.
    Expired(Expired),
    Other,
}

/// What kind of media expired (`lng_ttl_*_expired`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expired {
    Photo,
    Video,
    VideoNote,
    VoiceNote,
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
            MessageContent::MessageExpiredPhoto(_) => Self::Expired(Expired::Photo),
            MessageContent::MessageExpiredVideo(_) => Self::Expired(Expired::Video),
            MessageContent::MessageExpiredVideoNote(_) => Self::Expired(Expired::VideoNote),
            MessageContent::MessageExpiredVoiceNote(_) => Self::Expired(Expired::VoiceNote),
            _ => return None,
        })
    }
}

/// A music file's tags and the file (`audio`).
// shortcut: no album cover; tdesktop draws it in place of the play circle (`isSongWithCover`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Music {
    /// May be empty.
    pub title: String,
    /// May be empty.
    pub performer: String,
    /// In seconds.
    pub duration: i32,
    pub file: File,
}

impl Music {
    /// "Performer – Title" as tdesktop names a song (`FormatSongName`), the
    /// file name when it has no tags.
    pub fn name(&self, file_name: &str) -> String {
        match self.parts(file_name) {
            (performer, title) if performer.is_empty() => title,
            (performer, title) => format!("{performer} \u{2013} {title}"),
        }
    }

    /// The performer, maybe empty, and the title (`ComputeComposedName`).
    pub fn parts(&self, file_name: &str) -> (String, String) {
        const UNKNOWN: &str = "Unknown Track";
        let title = match (self.title.is_empty(), self.performer.is_empty()) {
            (true, true) if !file_name.is_empty() => file_name,
            (true, _) => UNKNOWN,
            (false, _) => &self.title,
        };
        (self.performer.clone(), title.to_owned())
    }
}

/// A voice message's sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceNote {
    /// In seconds.
    pub duration: i32,
    /// 0-31 per bar; empty when the sender sent none.
    pub waveform: Vec<u8>,
    pub file: File,
    /// Some recipient played it; until then a dot follows the length.
    pub is_listened: bool,
}

/// TDLib's base64 of 5-bit values, least significant bits first
/// (`documentWaveformDecode`).
fn waveform(base64: &str) -> Vec<u8> {
    use base64::Engine as _;
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(base64) else {
        return Vec::new();
    };
    (0..bytes.len() * 8 / 5)
        .map(|index| {
            let (byte, shift) = (index * 5 / 8, index * 5 % 8);
            let low = u16::from(bytes[byte]);
            let high = bytes.get(byte + 1).map_or(0, |&high| u16::from(high));
            let pair = low | high << 8;
            u8::try_from(pair >> shift & 0x1F).unwrap_or(0)
        })
        .collect()
}

/// A photo, video or file with its caption and a blurred preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Media {
    pub caption: FormattedText,
    pub thumbnail: Option<Minithumbnail>,
    pub dimensions: Option<Dimensions>,
    /// A still of a video, GIF or file: TDLib's JPEG thumbnail.
    pub preview: Option<File>,
    /// A video's length in seconds; 0 for other media.
    pub duration: i32,
    /// The photo's largest size, or the video or file itself, as the
    /// message brought it; `updateFile` follows it in `Chats::file`.
    pub file: Option<File>,
    /// Hidden under a spoiler until clicked.
    pub has_spoiler: bool,
    /// Where a video starts playing, in seconds; 0 from the beginning.
    pub start_timestamp: i32,
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
            via_bot: None,
            can_be_saved: true,
            topic: None,
            replies: None,
            reactions: None,
            schedule: None,
            self_destruct: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_is_named_as_tdesktop_names_it() {
        let music = |title: &str, performer: &str| Music {
            title: title.to_owned(),
            performer: performer.to_owned(),
            duration: 0,
            file: File {
                id: crate::FileId(1),
                path: None,
                is_downloading: false,
                downloaded: 0,
                size: 0,
                uploaded: 0,
                is_uploading: false,
                prefix: crate::file::Prefix::default(),
            },
        };
        assert_eq!(music("Song", "Band").name("a.mp3"), "Band \u{2013} Song");
        assert_eq!(music("Song", "").name("a.mp3"), "Song");
        assert_eq!(
            music("", "Band").name("a.mp3"),
            "Band \u{2013} Unknown Track"
        );
        assert_eq!(music("", "").name("a.mp3"), "a.mp3");
        assert_eq!(music("", "").name(""), "Unknown Track");
    }

    #[test]
    fn waveforms_unpack_five_bits_each() {
        // [31, 0, 1] packed as the recorder sends it.
        assert_eq!(waveform("HwQ="), [31, 0, 1]);
        assert!(waveform("not base64!").is_empty());
        assert!(waveform("").is_empty());
    }
}
