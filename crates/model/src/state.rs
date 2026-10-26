use std::collections::HashMap;

use td_api::enums::ConnectionState as TdConnectionState;

use crate::auth::{self, Terms, TermsId};
use crate::chats::Chats;
use crate::event::Event;
use crate::history::{History, Page};
use crate::ids::{ChatId, UserId};
use crate::message::Message;

/// Everything the UI shows, built only from [`Event`]s.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub auth: auth::State,
    /// Updated terms of service waiting for the user.
    pub terms_update: Option<(TermsId, Terms)>,
    pub connection: ConnectionState,
    /// The language pack TDLib suggests for the system language; empty when
    /// there is none.
    pub suggested_language: String,
    pub chats: Chats,
    /// The signed-in user, once TDLib told.
    pub me: Option<UserId>,
    /// The messages of open chats; others get none, so memory stays flat.
    histories: HashMap<ChatId, History>,
}

/// How TDLib's connection to Telegram stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectionState {
    WaitingForNetwork,
    ConnectingToProxy,
    #[default]
    Connecting,
    /// Connected and fetching what changed while offline.
    Updating,
    Ready,
}

impl State {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Auth(state) => {
                if !state.is_signed_in() {
                    self.terms_update = None;
                }
                // A new client sends every chat again.
                if matches!(
                    state,
                    auth::State::LoggingOut | auth::State::Closing | auth::State::Closed
                ) {
                    self.chats = Chats::default();
                    self.histories.clear();
                    self.me = None;
                }
                self.auth = state;
            }
            Event::TermsUpdate { id, terms } => self.terms_update = Some((id, terms)),
            Event::Connection(connection) => self.connection = connection,
            Event::SuggestedLanguage(language) => self.suggested_language = language,
            Event::Me(user) => self.me = Some(user),
            Event::Chats(change) => {
                for history in self.histories.values_mut() {
                    history.observe(&change);
                }
                self.chats.apply(change);
            }
            Event::Messages(change) => {
                if let Some(history) = self.histories.get_mut(&change.chat()) {
                    history.apply(change);
                }
            }
        }
    }

    #[must_use]
    pub fn history(&self, chat: ChatId) -> Option<&History> {
        self.histories.get(&chat)
    }

    /// Starts keeping `chat`'s messages, empty until a page loads.
    pub fn open_history(&mut self, chat: ChatId) {
        self.histories.entry(chat).or_default();
    }

    pub fn close_history(&mut self, chat: ChatId) {
        self.histories.remove(&chat);
    }

    /// Adds a loaded page to an open chat's history.
    pub fn add_history_page(&mut self, chat: ChatId, page: Page, messages: Vec<Message>) {
        if let Some(history) = self.histories.get_mut(&chat) {
            history.add_page(page, messages);
        }
    }

    /// Empties an open chat's history before a jump loads another part.
    pub fn clear_history(&mut self, chat: ChatId) {
        if let Some(history) = self.histories.get_mut(&chat) {
            history.clear_messages();
        }
    }

    /// Keeps a message that `chat`'s history points at.
    pub fn add_reference(&mut self, chat: ChatId, message: Message) {
        if let Some(history) = self.histories.get_mut(&chat) {
            history.add_reference(message);
        }
    }

    /// Forgets the terms update once the user accepted it; TDLib sends no
    /// update for that.
    pub fn accept_terms(&mut self, id: &TermsId) {
        if self
            .terms_update
            .as_ref()
            .is_some_and(|(pending, _)| pending == id)
        {
            self.terms_update = None;
        }
    }
}

impl From<TdConnectionState> for ConnectionState {
    fn from(state: TdConnectionState) -> Self {
        match state {
            TdConnectionState::WaitingForNetwork(_) => Self::WaitingForNetwork,
            TdConnectionState::ConnectingToProxy(_) => Self::ConnectingToProxy,
            TdConnectionState::Connecting(_) => Self::Connecting,
            TdConnectionState::Updating(_) => Self::Updating,
            TdConnectionState::Ready(_) => Self::Ready,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_out_drops_a_pending_terms_update() {
        let mut state = State::default();
        state.apply(Event::Auth(auth::State::Ready));
        let id = TermsId("1".to_owned());
        state.apply(Event::TermsUpdate {
            id: id.clone(),
            terms: Terms::default(),
        });
        assert!(state.terms_update.is_some());
        state.apply(Event::Auth(auth::State::LoggingOut));
        assert_eq!(state.terms_update, None);
        assert_eq!(state.auth, auth::State::LoggingOut);
    }

    #[test]
    fn accepting_clears_only_the_matching_terms() {
        let mut state = State::default();
        state.apply(Event::Auth(auth::State::Ready));
        state.apply(Event::TermsUpdate {
            id: TermsId("2".to_owned()),
            terms: Terms::default(),
        });
        state.accept_terms(&TermsId("1".to_owned()));
        assert!(state.terms_update.is_some());
        state.accept_terms(&TermsId("2".to_owned()));
        assert_eq!(state.terms_update, None);
    }
}
