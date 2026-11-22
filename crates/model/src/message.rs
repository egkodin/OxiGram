//! Messages as the chat list previews them and the history shows them.

use td_api::enums::{
    CallDiscardReason, DiceStickers, LinkPreviewAlbumMedia, LinkPreviewType, MessageContent,
    MessageOrigin, MessageReplyTo, MessageSchedulingState, MessageSelfDestructType, MessageSender,
    MessageSendingState, MessageTopic,
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

pub(crate) use convert::{interaction, still};

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
    fn from_td(state: MessageSchedulingState) -> Self {
        match state {
            MessageSchedulingState::SendAtDate(state) => Self::At(state.send_date),
            MessageSchedulingState::SendWhenOnline(_) => Self::WhenOnline,
            MessageSchedulingState::SendWhenVideoProcessed(state) => Self::At(state.send_date),
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
        is_group: bool,
        is_missed: bool,
        duration: i32,
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
// shortcut: video and animation previews show their stills; add playback with embedded media.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkPreview {
    pub url: String,
    pub site_name: String,
    pub title: String,
    pub description: FormattedText,
    pub image: Option<Box<Media>>,
    pub show_large_media: bool,
    pub show_media_above_description: bool,
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
            image: preview_image(*preview.r#type, preview.show_large_media).map(Box::new),
            show_large_media: preview.show_large_media,
            show_media_above_description: preview.show_media_above_description,
            is_above_text: preview.show_above_text,
            is_url_in_text: preview.skip_confirmation,
        }
    }
}

fn preview_image(kind: LinkPreviewType, large: bool) -> Option<Media> {
    use td_api::types::{ChatPhoto, Minithumbnail as TdMini, Photo, PhotoSize, Thumbnail};

    fn photo_image(sizes: Vec<PhotoSize>, mini: Option<Box<TdMini>>, target: i32) -> Option<Media> {
        let size = sizes.into_iter().min_by_key(|size| {
            let side = size.width.max(size.height);
            (side < target, side.abs_diff(target))
        });
        let dimensions = size
            .as_ref()
            .and_then(|size| Dimensions::new(size.width, size.height));
        let file = size.map(|size| File::from(*size.photo));
        image(file, dimensions, mini)
    }

    fn still_image(still: Option<Box<Thumbnail>>, mini: Option<Box<TdMini>>) -> Option<Media> {
        let dimensions = still
            .as_ref()
            .and_then(|still| Dimensions::new(still.width, still.height));
        image(convert::still(still), dimensions, mini)
    }

    fn image(
        file: Option<File>,
        dimensions: Option<Dimensions>,
        mini: Option<Box<TdMini>>,
    ) -> Option<Media> {
        let thumbnail = mini.and_then(|mini| Minithumbnail::from_td(*mini));
        (file.is_some() || thumbnail.is_some()).then_some(Media {
            file,
            dimensions,
            thumbnail,
            ..Media::default()
        })
    }

    let target = if large { 800 } else { 144 };
    let web_photo = |photo: Photo| photo_image(photo.sizes, photo.minithumbnail, target);
    let peer_photo = |photo: Box<ChatPhoto>| photo_image(photo.sizes, photo.minithumbnail, target);
    let sticker_image = |sticker: td_api::types::Sticker| {
        let sticker = StickerPicture::from(sticker);
        image(sticker.drawable().cloned(), sticker.dimensions, None)
    };
    match kind {
        LinkPreviewType::Article(article) => article.photo.and_then(|photo| web_photo(*photo)),
        LinkPreviewType::Photo(photo) => web_photo(*photo.photo),
        LinkPreviewType::App(app) => web_photo(*app.photo),
        LinkPreviewType::WebApp(app) => app.photo.and_then(|photo| web_photo(*photo)),
        LinkPreviewType::Chat(chat) => chat.photo.and_then(peer_photo),
        LinkPreviewType::User(user) => user.photo.and_then(peer_photo),
        LinkPreviewType::DirectMessagesChat(chat) => chat.photo.and_then(peer_photo),
        LinkPreviewType::ChannelBoost(chat) => chat.photo.and_then(peer_photo),
        LinkPreviewType::SupergroupBoost(chat) => chat.photo.and_then(peer_photo),
        LinkPreviewType::VideoChat(chat) => chat.photo.and_then(peer_photo),
        LinkPreviewType::Sticker(sticker) => sticker_image(*sticker.sticker),
        LinkPreviewType::StickerSet(set) => set.stickers.into_iter().find_map(sticker_image),
        LinkPreviewType::StoryAlbum(album) => album
            .photo_icon
            .and_then(|photo| web_photo(*photo))
            .or_else(|| {
                album
                    .video_icon
                    .and_then(|video| still_image(video.thumbnail, video.minithumbnail))
            }),
        LinkPreviewType::Audio(audio) => still_image(
            audio.audio.album_cover_thumbnail,
            audio.audio.album_cover_minithumbnail,
        ),
        LinkPreviewType::VideoNote(note) => {
            still_image(note.video_note.thumbnail, note.video_note.minithumbnail)
        }
        LinkPreviewType::Album(album) => album.media.into_iter().find_map(|media| match media {
            LinkPreviewAlbumMedia::Photo(photo) => web_photo(*photo.photo),
            LinkPreviewAlbumMedia::Video(video) => {
                still_image(video.video.thumbnail, video.video.minithumbnail)
            }
        }),
        LinkPreviewType::Animation(animation) => still_image(
            animation.animation.thumbnail,
            animation.animation.minithumbnail,
        ),
        LinkPreviewType::Video(video) => video
            .cover
            .and_then(|cover| web_photo(*cover))
            .or_else(|| still_image(video.video.thumbnail, video.video.minithumbnail)),
        LinkPreviewType::Document(document) => {
            still_image(document.document.thumbnail, document.document.minithumbnail)
        }
        LinkPreviewType::EmbeddedAnimationPlayer(player) => player
            .thumbnail
            .and_then(|photo| web_photo(*photo))
            .or_else(|| {
                player
                    .animation
                    .and_then(|animation| still_image(animation.thumbnail, animation.minithumbnail))
            }),
        LinkPreviewType::EmbeddedVideoPlayer(player) => player
            .thumbnail
            .and_then(|photo| web_photo(*photo))
            .or_else(|| {
                player
                    .video
                    .and_then(|video| still_image(video.thumbnail, video.minithumbnail))
            }),
        LinkPreviewType::EmbeddedAudioPlayer(player) => {
            player.thumbnail.and_then(|photo| web_photo(*photo))
        }
        _ => None,
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    use td_api::types::{LinkPreviewTypeArticle, Photo, PhotoSize};

    #[test]
    fn an_article_keeps_its_photo_and_dimensions() {
        let file =
            |id| {
                Box::new(serde_json::from_value(serde_json::json!({
            "id": id, "size": 0, "expected_size": 0,
            "local": {"path": "", "can_be_downloaded": true, "can_be_deleted": false,
                "is_downloading_active": false, "is_downloading_completed": false,
                "download_offset": 0, "downloaded_prefix_size": 0, "downloaded_size": 0},
            "remote": {"id": "", "unique_id": "", "is_uploading_active": false,
                "is_uploading_completed": false, "uploaded_size": 0}
        })).unwrap())
            };
        let size = |id, side| PhotoSize {
            r#type: "x".into(),
            photo: file(id),
            width: side,
            height: side,
            progressive_sizes: vec![],
        };
        let article = |photo| LinkPreviewType::Article(LinkPreviewTypeArticle { photo });
        assert!(preview_image(article(None), true).is_none());
        let with_sizes = || {
            article(Some(Box::new(Photo {
                has_stickers: false,
                minithumbnail: None,
                sizes: vec![size(1, 40), size(2, 160), size(3, 800), size(4, 2560)],
            })))
        };
        let large = preview_image(with_sizes(), true).unwrap();
        assert_eq!(large.file.unwrap().id, crate::FileId(3));
        assert_eq!(large.dimensions, Dimensions::new(800, 800));
        let small = preview_image(with_sizes(), false).unwrap();
        assert_eq!(small.file.unwrap().id, crate::FileId(2));
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
