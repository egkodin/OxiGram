//! A sticker or custom emoji set (`stickerSet`), as its box shows it,
//! and adding or removing it (`changeStickerSet`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::StickerType;
use td_api::functions::{ChangeStickerSet, GetStickerSet};

use crate::ids::StickerSetId;
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
    pub stickers: Vec<StickerPicture>,
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
            stickers: set.stickers.into_iter().map(StickerPicture::from).collect(),
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
