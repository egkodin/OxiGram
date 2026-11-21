//! A sticker or custom emoji set (`stickerSet`), as its box shows it,
//! and adding or removing it (`changeStickerSet`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{InputFile, InputMessageContent, StickerFullType, StickerType};
use td_api::functions::{ChangeStickerSet, GetInstalledStickerSets, GetStickerSet, SendMessage};
use td_api::types as td;

use crate::ids::{CustomEmojiId, StickerSetId};
use crate::session::{RequestError, Session};
use crate::sticker::StickerPicture;
use crate::{MessageId, thread::Key};

/// What a set holds (`StickersType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetKind {
    Stickers,
    Masks,
    Emoji,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickerSet {
    pub id: StickerSetId,
    pub title: String,
    /// The short name in its `t.me/addstickers/` link.
    pub name: String,
    pub kind: SetKind,
    pub is_installed: bool,
    pub is_archived: bool,
    pub is_official: bool,
    pub stickers: Vec<SetSticker>,
}

/// A sticker in a set, with the emoji it stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSticker {
    pub picture: StickerPicture,
    /// The glyph a custom emoji falls back to.
    pub emoji: String,
    pub custom: Option<CustomEmojiId>,
}

impl From<td_api::types::Sticker> for SetSticker {
    fn from(sticker: td_api::types::Sticker) -> Self {
        let custom = match &*sticker.full_type {
            StickerFullType::CustomEmoji(custom) => Some(CustomEmojiId(custom.custom_emoji_id)),
            StickerFullType::Regular(_) | StickerFullType::Mask(_) => None,
        };
        Self {
            emoji: sticker.emoji.clone(),
            custom,
            picture: StickerPicture::from(sticker),
        }
    }
}

impl StickerSet {
    /// Added and not archived (`notInstalled` is its opposite).
    #[must_use]
    pub fn is_added(&self) -> bool {
        self.is_installed && !self.is_archived
    }
}

impl From<td_api::types::StickerSet> for StickerSet {
    fn from(set: td_api::types::StickerSet) -> Self {
        let kind = match *set.sticker_type {
            StickerType::Regular(_) => SetKind::Stickers,
            StickerType::Mask(_) => SetKind::Masks,
            StickerType::CustomEmoji(_) => SetKind::Emoji,
        };
        Self {
            id: StickerSetId(set.id),
            title: set.title,
            name: set.name,
            kind,
            is_installed: set.is_installed,
            is_archived: set.is_archived,
            is_official: set.is_official,
            stickers: set.stickers.into_iter().map(SetSticker::from).collect(),
        }
    }
}

impl Session {
    /// The set with its stickers (`getStickerSet`).
    pub fn sticker_set(
        &self,
        set: StickerSetId,
    ) -> impl Future<Output = Result<StickerSet, RequestError>> + Send + use<> {
        self.client()
            .request(&GetStickerSet { set_id: set.0 })
            .map(|result| result.map(StickerSet::from).map_err(RequestError::from))
    }

    /// The sets the user added, in their order.
    pub fn installed_sticker_sets(
        &self,
        kind: SetKind,
    ) -> impl Future<Output = Result<Vec<StickerSetId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetInstalledStickerSets {
                sticker_type: Box::new(match kind {
                    SetKind::Stickers => StickerType::Regular(td::StickerTypeRegular {}),
                    SetKind::Masks => StickerType::Mask(td::StickerTypeMask {}),
                    SetKind::Emoji => StickerType::CustomEmoji(td::StickerTypeCustomEmoji {}),
                }),
            })
            .map(|result| {
                result
                    .map(|sets| sets.sets.iter().map(|set| StickerSetId(set.id)).collect())
                    .map_err(RequestError::from)
            })
    }

    pub fn send_sticker(
        &self,
        key: Key,
        reply: Option<MessageId>,
        sticker: &SetSticker,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&sticker_request(key, reply, sticker))
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    /// Adds the set to the user's, or removes it (`changeStickerSet`).
    pub fn change_sticker_set(
        &self,
        set: StickerSetId,
        change: SetChange,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ChangeStickerSet {
                set_id: set.0,
                is_installed: change == SetChange::Add,
                is_archived: false,
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}

/// What the box's button does with the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetChange {
    Add,
    Remove,
}

fn sticker_request(key: Key, reply: Option<MessageId>, sticker: &SetSticker) -> SendMessage {
    let size = |side| i32::try_from(side).unwrap_or(0);
    let (width, height) = sticker
        .picture
        .dimensions
        .map_or((0, 0), |d| (size(d.width), size(d.height)));
    SendMessage {
        chat_id: key.chat.0,
        topic_id: key.topic.map(|topic| Box::new(topic.to_td())),
        reply_to: crate::send::reply(reply),
        options: None,
        reply_markup: None,
        input_message_content: Box::new(InputMessageContent::InputMessageSticker(
            td::InputMessageSticker {
                sticker: Box::new(td::InputSticker {
                    sticker: Box::new(InputFile::Id(td::InputFileId {
                        id: sticker.picture.file.id.0,
                    })),
                    thumbnail: None,
                    width,
                    height,
                }),
                emoji: sticker.emoji.clone(),
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChatId, FileId,
        file::{Dimensions, File},
        message::Topic,
        sticker::StickerKind,
    };

    #[test]
    fn sends_the_original_sticker_in_the_selected_topic_and_reply() {
        let sticker = SetSticker {
            emoji: "😸".into(),
            custom: None,
            picture: StickerPicture {
                kind: StickerKind::Animated,
                dimensions: Dimensions::new(512, 480),
                file: File {
                    id: FileId(7),
                    path: None,
                    is_downloading: false,
                    downloaded: 0,
                    size: 0,
                    uploaded: 0,
                    is_uploading: false,
                    prefix: Default::default(),
                },
                still: None,
                premium_effect: None,
                skin_tone: None,
                set: None,
            },
        };
        let key = Key {
            chat: ChatId(42),
            topic: Some(Topic::Thread(MessageId(3))),
        };
        let message = MessageId(4);
        let request = sticker_request(key, Some(message), &sticker);
        assert_eq!(request.chat_id, 42);
        assert_eq!(
            request.topic_id,
            key.topic.map(|topic| Box::new(topic.to_td()))
        );
        assert_eq!(request.reply_to, crate::send::reply(Some(message)));
        let InputMessageContent::InputMessageSticker(content) = *request.input_message_content
        else {
            panic!("expected sticker");
        };
        assert_eq!(
            *content.sticker.sticker,
            InputFile::Id(td::InputFileId { id: 7 })
        );
        assert_eq!((content.sticker.width, content.sticker.height), (512, 480));
        assert_eq!(content.emoji, "😸");
    }
}
