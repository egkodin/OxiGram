//! A sticker or custom emoji set (`stickerSet`), as its box shows it,
//! and adding or removing it (`changeStickerSet`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{StickerFullType, StickerType};
use td_api::functions::{ChangeStickerSet, GetInstalledStickerSets, GetStickerSet};
use td_api::types::StickerTypeCustomEmoji;

use crate::ids::{CustomEmojiId, StickerSetId};
use crate::session::{RequestError, Session};
use crate::sticker::StickerPicture;

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

    /// The custom emoji sets the user added (`getInstalledStickerSets`),
    /// in their order.
    pub fn installed_emoji_sets(
        &self,
    ) -> impl Future<Output = Result<Vec<StickerSetId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetInstalledStickerSets {
                sticker_type: Box::new(StickerType::CustomEmoji(StickerTypeCustomEmoji {})),
            })
            .map(|result| {
                result
                    .map(|sets| sets.sets.iter().map(|set| StickerSetId(set.id)).collect())
                    .map_err(RequestError::from)
            })
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
