//! Lottie in the history (`HistoryView::Sticker` with its Lottie player):
//! TGS stickers loop, animated emoji play once and again on a click, and a
//! Premium sticker plays its effect beside it once, as Power Usage allows.

use std::path::Path;
use std::sync::Arc;

use gpui::{Context, RenderImage};
use model::file::File;
use model::message::Message;
use model::sticker::StickerPicture;
use model::{CustomEmojiId, MessageId};

use super::HistoryView;
use super::documents::current;
use super::photo::{PHOTO_PRIORITY, Photo};
use crate::lottie::{Key, Play};
use crate::power_saving::{Flag, plays};
use crate::store::EmojiKey;
use media::lottie::SkinTone;

/// `kPremiumMultiplier`: the effect's size against the sticker's.
const PREMIUM_MULTIPLIER: f32 = 1.0 + 0.245 * 2.0;
/// `kPremiumShift`: how far the effect reaches past the sticker's edge.
const PREMIUM_SHIFT: f32 = 21.0 / 240.0;

/// Which side of the history a message sits on.
#[derive(Clone, Copy)]
enum Side {
    Incoming,
    Outgoing,
}

/// An animation drawn over and around a picture, in points from its box.
pub(super) struct Effect {
    pub image: Arc<RenderImage>,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// How stickers in chats move: looped (`loopAnimatedStickers`, on by
/// default), or their first frame with Power Usage's switch off.
pub(super) fn sticker_play(cx: &gpui::App) -> Play {
    if plays(Flag::StickersChat, cx) {
        Play::Loop
    } else {
        Play::Still
    }
}

/// Animated emoji play once (`emojiSticker`), or stay still.
pub(super) fn emoji_play(cx: &gpui::App) -> Play {
    if plays(Flag::EmojiChat, cx) {
        Play::Once
    } else {
        Play::Still
    }
}

fn tone(picture: &StickerPicture) -> SkinTone {
    SkinTone(picture.skin_tone.unwrap_or_default())
}

fn instance(message: MessageId) -> u64 {
    u64::from_ne_bytes(message.0.to_ne_bytes())
}

impl HistoryView {
    /// The player of a TGS file at `size` points, once the file is on disk;
    /// asks for it otherwise.
    pub(super) fn lottie_key(
        &mut self,
        file: &File,
        size: (f32, f32),
        instance: u64,
        cx: &mut Context<Self>,
    ) -> Option<Key> {
        let file = current(self, file, cx).clone();
        let Some(path) = file.path else {
            self.fetch(file.id, PHOTO_PRIORITY, cx);
            return None;
        };
        Some(Key::new(
            Path::new(&path),
            size,
            self.display_scale(),
            instance,
        ))
    }

    /// A TGS sticker's frame at its box, shared by every message showing it.
    pub(super) fn lottie_sticker(
        &mut self,
        picture: &StickerPicture,
        size: (f32, f32),
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let key = self.lottie_key(&picture.file, size, 0, cx)?;
        crate::lottie::frame(&key, sticker_play(cx), cx)
    }

    /// An animated emoji's frame: its own clock per message, played once.
    pub(super) fn lottie_emoji(
        &mut self,
        message: MessageId,
        picture: &StickerPicture,
        size: (f32, f32),
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let key = self.lottie_key(&picture.file, size, instance(message), cx)?;
        let key = key.with_tone(tone(picture));
        crate::lottie::frame(&key, emoji_play(cx), cx)
    }

    /// An animated custom emoji's frame at `side` points, shared by every
    /// place showing it, looped under Power Usage's emoji switch.
    pub(super) fn custom_emoji(
        &mut self,
        id: CustomEmojiId,
        side: f32,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let file = self.store.update(cx, |store, cx| {
            store.emoji_animation(&EmojiKey::Custom(id), cx)
        })?;
        let key = self.lottie_key(&file, (side, side), 0, cx)?;
        let play = if plays(Flag::EmojiChat, cx) {
            Play::Loop
        } else {
            Play::Still
        };
        crate::lottie::frame(&key, play, cx)
    }

    /// Plays an animated emoji again (a click, `emojiInteraction` without
    /// the partner's side).
    pub(super) fn replay_emoji(
        &mut self,
        message: MessageId,
        picture: &StickerPicture,
        size: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        if let Some(key) = self.lottie_key(&picture.file, size, instance(message), cx) {
            crate::lottie::replay(&key.with_tone(tone(picture)), cx);
        }
    }

    /// A sticker with its Premium effect while that plays.
    pub(super) fn premium_sticker(
        &mut self,
        message: &Message,
        picture: &StickerPicture,
        cx: &mut Context<Self>,
    ) -> Photo {
        let sticker = self.sticker(message.id, picture, cx);
        let side = if message.is_outgoing {
            Side::Outgoing
        } else {
            Side::Incoming
        };
        let effect = self.premium_effect(message.id, picture, box_size(&sticker), side, cx);
        Photo { effect, ..sticker }
    }

    /// A Premium sticker's effect, once per message while it shows, reaching
    /// past the side away from the bubble's edge.
    fn premium_effect(
        &mut self,
        message: MessageId,
        picture: &StickerPicture,
        (width, height): (f32, f32),
        side: Side,
        cx: &mut Context<Self>,
    ) -> Option<Effect> {
        let file = picture.premium_effect.as_ref()?;
        if !plays(Flag::StickersChat, cx) {
            return None;
        }
        let size = (width * PREMIUM_MULTIPLIER, height * PREMIUM_MULTIPLIER);
        let key = self.lottie_key(file, size, instance(message), cx)?;
        if crate::lottie::is_done(&key, cx) {
            return None;
        }
        let image = crate::lottie::frame(&key, Play::Once, cx)?;
        let shift = width * PREMIUM_SHIFT;
        let left = match side {
            Side::Outgoing => width + shift - size.0,
            Side::Incoming => -shift,
        };
        Some(Effect {
            image,
            left,
            top: (height - size.1) / 2.0,
            width: size.0,
            height: size.1,
        })
    }
}

/// Sizes in points from the picture's box.
#[expect(clippy::cast_precision_loss, reason = "a size in points")]
pub(super) fn box_size(photo: &Photo) -> (f32, f32) {
    (photo.width as f32, photo.height as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_ids_make_distinct_instances() {
        assert_ne!(instance(MessageId(1 << 20)), instance(MessageId(2 << 20)));
        assert_ne!(instance(MessageId(1)), 0, "0 is the shared sticker clock");
    }
}
