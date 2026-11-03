//! Stickers in the history (`HistoryView::Sticker` in `UnwrappedMedia`):
//! the picture alone at tdesktop's size, no bubble, the time on a pill.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Context, ImageSource, RenderImage};
use model::MessageId;
use model::sticker::{StickerKind, StickerPicture};
use style::chat::MAX_STICKER_SIZE;

use super::HistoryView;
use super::documents::current;
use super::photo::{PHOTO_PRIORITY, Photo};

/// `DownscaledSize` into the sticker box: smaller ones keep their size.
fn size(picture: &StickerPicture) -> (i32, i32) {
    let side = MAX_STICKER_SIZE.0;
    let Some(dimensions) = picture.dimensions else {
        return (side, side);
    };
    let narrow = |value: u32| i32::try_from(value).unwrap_or(i32::MAX).max(1);
    let (width, height) = (narrow(dimensions.width), narrow(dimensions.height));
    if width <= side && height <= side {
        return (width, height);
    }
    let (wide, high) = (i64::from(width), i64::from(height));
    let side = i64::from(side);
    let fitted = if wide >= high {
        (side, (high * side / wide).max(1))
    } else {
        ((wide * side / high).max(1), side)
    };
    let narrow = |value: i64| i32::try_from(value).unwrap_or(i32::MAX);
    (narrow(fitted.0), narrow(fitted.1))
}

impl HistoryView {
    /// `message`'s sticker: the file of a still one, else its still; a
    /// video one loops over its still once on disk.
    // shortcut: animated (TGS) stickers show their still; playing them comes with the Lottie player.
    pub(super) fn sticker(
        &mut self,
        message: MessageId,
        picture: &StickerPicture,
        cx: &mut Context<Self>,
    ) -> Photo {
        let shown = match picture.kind {
            StickerKind::Still => Some(&picture.file),
            StickerKind::Animated | StickerKind::Video => None,
        };
        let mut path = None;
        for file in shown.into_iter().chain(picture.still.as_ref()) {
            let file = current(self, file, cx).clone();
            match file.path {
                Some(found) if path.is_none() => path = Some(found),
                Some(_) => {}
                None => self.fetch(file.id, PHOTO_PRIORITY, cx),
            }
        }
        let (width, height) = size(picture);
        let frame = match picture.kind {
            StickerKind::Video => self.video_sticker(message, picture, (width, height), cx),
            StickerKind::Still | StickerKind::Animated => None,
        };
        Photo {
            width,
            height,
            source: path.map(|path| ImageSource::from(PathBuf::from(path))),
            thumbnail: None,
            video: None,
            frame,
            map: None,
            placeholder: None,
            spoiler: None,
            open: None,
            once: None,
        }
    }
}

impl HistoryView {
    /// A WebM sticker's frame, muted and looped as GIFs are; it downloads
    /// the first time it shows.
    fn video_sticker(
        &mut self,
        message: MessageId,
        picture: &StickerPicture,
        (width, height): (i32, i32),
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let file = current(self, &picture.file, cx).clone();
        let Some(path) = file.path else {
            self.fetch(file.id, PHOTO_PRIORITY, cx);
            return None;
        };
        #[expect(clippy::cast_precision_loss, reason = "a size in points")]
        let size = (width as f32, height as f32);
        self.autoplay(message, Path::new(&path), size, cx);
        self.inline_frame(message, cx)?.image
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::FileId;
    use model::file::{Dimensions, File};

    fn sized(width: i32, height: i32) -> (i32, i32) {
        let file = File {
            id: FileId(1),
            path: None,
            is_downloading: false,
            downloaded: 0,
            size: 0,
            uploaded: 0,
            is_uploading: false,
        };
        size(&StickerPicture {
            kind: StickerKind::Still,
            dimensions: Dimensions::new(width, height),
            file,
            still: None,
        })
    }

    #[test]
    fn stickers_fit_the_box_and_small_ones_stay() {
        assert_eq!(sized(512, 512), (224, 224));
        assert_eq!(sized(512, 256), (224, 112));
        assert_eq!(sized(100, 80), (100, 80));
    }
}
