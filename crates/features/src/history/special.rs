//! What a message shows in place of its text: a contact card, a poll, a
//! checklist, or emoji drawn big.

use gpui::{AnyElement, App, Context};
use model::message::Message;

use style::chat::{LARGE_EMOJI_OUTLINE, LARGE_EMOJI_SIZE};

use super::HistoryView;
use super::large_emoji::{Big, Face};

pub(super) enum Special {
    Contact(super::contact::Card),
    Poll(super::poll::View),
    Checklist(super::checklist::View),
    LargeEmoji(Vec<super::large_emoji::Face>),
}

impl HistoryView {
    pub(super) fn special(&mut self, message: &Message, cx: &mut Context<Self>) -> Option<Special> {
        let is_large_emoji = crate::settings::is_large_emoji(cx);
        match self.contact(message, cx) {
            Some(card) => Some(Special::Contact(card)),
            None => self
                .poll(message, cx)
                .map(Special::Poll)
                .or_else(|| self.checklist(message, cx).map(Special::Checklist))
                .or_else(|| {
                    let big = super::large_emoji::of(message).filter(|_| is_large_emoji)?;
                    Some(Special::LargeEmoji(self.large_faces(big, cx)))
                }),
        }
    }
}

impl HistoryView {
    /// Big emoji as drawn: a custom one's frame or still, else its glyph.
    fn large_faces(&mut self, big: Vec<Big>, cx: &mut Context<Self>) -> Vec<Face> {
        #[expect(clippy::cast_precision_loss, reason = "a size in points")]
        let side = (LARGE_EMOJI_SIZE.0 + LARGE_EMOJI_OUTLINE.0 * 2) as f32;
        big.into_iter()
            .map(|big| {
                let Some(id) = big.custom else {
                    return Face::Glyph(big.emoji);
                };
                if let Some(frame) = self.custom_emoji(id, side, cx) {
                    return Face::Frame(frame);
                }
                let key = crate::store::EmojiKey::Custom(id);
                match self
                    .store
                    .update(cx, |store, cx| store.emoji_picture(&key, cx))
                {
                    Some(path) => Face::Picture(path),
                    None => Face::Glyph(big.emoji),
                }
            })
            .collect()
    }
}

impl Special {
    /// The block in the bubble; emoji under a header stay text.
    pub(super) fn element(
        self,
        id: u64,
        out: bool,
        is_selected: bool,
        cx: &App,
    ) -> Option<AnyElement> {
        match self {
            Self::Contact(card) => Some(super::contact::element(card, id, out, is_selected, cx)),
            Self::Poll(poll) => Some(super::poll::element(poll, id, out, is_selected, cx)),
            Self::Checklist(list) => {
                Some(super::checklist::element(list, id, out, is_selected, cx))
            }
            Self::LargeEmoji(_) => None,
        }
    }
}
