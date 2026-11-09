//! A sticker as a message shows it (`messageSticker`): its file and size,
//! and a still for the animated kinds.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{StickerFormat, StickerFullType, ThumbnailFormat};
use td_api::functions::{GetAnimatedEmoji, GetCustomEmojiStickers};

use crate::file::{Dimensions, File};
use crate::ids::{CustomEmojiId, StickerSetId};
use crate::session::RequestError;

/// How the sticker's file draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickerKind {
    /// A WebP picture.
    Still,
    /// A Lottie animation (TGS).
    Animated,
    /// A WebM video.
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickerPicture {
    pub kind: StickerKind,
    pub dimensions: Option<Dimensions>,
    pub file: File,
    /// TDLib's thumbnail when it is a picture gpui draws (WebP, PNG or JPEG).
    pub still: Option<File>,
    /// A Premium sticker's effect around it (`premium_animation`).
    pub premium_effect: Option<File>,
    /// An animated emoji's skin tone, `fitzpatrick_type` 2 to 6.
    pub skin_tone: Option<u8>,
    /// The set it comes from; none for stickers outside any.
    pub set: Option<StickerSetId>,
}

impl From<td_api::types::Sticker> for StickerPicture {
    fn from(sticker: td_api::types::Sticker) -> Self {
        let kind = match *sticker.format {
            StickerFormat::Webp(_) => StickerKind::Still,
            StickerFormat::Tgs(_) => StickerKind::Animated,
            StickerFormat::Webm(_) => StickerKind::Video,
        };
        let still = sticker.thumbnail.and_then(|thumbnail| {
            matches!(
                *thumbnail.format,
                ThumbnailFormat::Webp(_) | ThumbnailFormat::Png(_) | ThumbnailFormat::Jpeg(_)
            )
            .then(|| File::from(*thumbnail.file))
        });
        let premium_effect = match *sticker.full_type {
            StickerFullType::Regular(regular) => {
                regular.premium_animation.map(|file| File::from(*file))
            }
            StickerFullType::Mask(_) | StickerFullType::CustomEmoji(_) => None,
        };
        Self {
            kind,
            set: (sticker.set_id != 0).then_some(StickerSetId(sticker.set_id)),
            dimensions: Dimensions::new(sticker.width, sticker.height),
            file: File::from(*sticker.sticker),
            still,
            premium_effect,
            skin_tone: None,
        }
    }
}

impl crate::session::Session {
    /// Custom emoji by id (`getCustomEmojiStickers`), as pictures; ids TDLib
    /// does not know are left out.
    pub fn custom_emoji(
        &self,
        ids: Vec<CustomEmojiId>,
    ) -> impl Future<Output = Result<Vec<(CustomEmojiId, StickerPicture)>, RequestError>> + Send + use<>
    {
        self.client()
            .request(&GetCustomEmojiStickers {
                custom_emoji_ids: ids.into_iter().map(|id| id.0).collect(),
            })
            .map(|result| {
                result.map_err(RequestError::from).map(|stickers| {
                    stickers
                        .stickers
                        .into_iter()
                        .filter_map(|sticker| match &*sticker.full_type {
                            StickerFullType::CustomEmoji(custom) => {
                                Some((CustomEmojiId(custom.custom_emoji_id), sticker.into()))
                            }
                            StickerFullType::Regular(_) | StickerFullType::Mask(_) => None,
                        })
                        .collect()
                })
            })
    }

    /// The animated version of a lone emoji (`getAnimatedEmoji`); None when
    /// Telegram has none.
    pub fn animated_emoji(
        &self,
        emoji: String,
    ) -> impl Future<Output = Result<Option<StickerPicture>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetAnimatedEmoji { emoji })
            .map(|result| {
                result
                    .map_err(RequestError::from)
                    .map(StickerPicture::from_animated_emoji)
            })
    }
}

impl StickerPicture {
    /// The sticker of an animated emoji, with its skin tone.
    #[must_use]
    pub fn from_animated_emoji(animated: td_api::types::AnimatedEmoji) -> Option<Self> {
        let skin_tone = u8::try_from(animated.fitzpatrick_type)
            .ok()
            .filter(|tone| *tone > 0);
        animated.sticker.map(|sticker| Self {
            skin_tone,
            ..Self::from(*sticker)
        })
    }

    /// The file gpui can draw: the sticker itself when it is a picture, else
    /// its still.
    #[must_use]
    pub fn drawable(&self) -> Option<&File> {
        match self.kind {
            StickerKind::Still => Some(&self.file),
            StickerKind::Animated | StickerKind::Video => self.still.as_ref(),
        }
    }
}
