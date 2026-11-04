//! What a message shows in place of its text: a contact card, a poll, a
//! checklist, or emoji drawn big.

use gpui::{AnyElement, App, Context};
use model::message::Message;

use super::HistoryView;

pub(super) enum Special {
    Contact(super::contact::Card),
    Poll(super::poll::View),
    Checklist(super::checklist::View),
    LargeEmoji(Vec<String>),
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
                    super::large_emoji::of(message)
                        .filter(|_| is_large_emoji)
                        .map(Special::LargeEmoji)
                }),
        }
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
