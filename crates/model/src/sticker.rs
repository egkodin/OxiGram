//! A sticker as a message shows it (`messageSticker`): its file and size,
//! and a still for the animated kinds.

use td_api::enums::{StickerFormat, ThumbnailFormat};

use crate::file::{Dimensions, File};

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
        Self {
            kind,
            dimensions: Dimensions::new(sticker.width, sticker.height),
            file: File::from(*sticker.sticker),
            still,
        }
    }
}
