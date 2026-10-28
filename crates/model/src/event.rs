use futures::{Stream, StreamExt as _, future};
use td_api::enums::{OptionValue, Update};

use crate::auth::{self, Terms, TermsId};
use crate::chats::Change;
use crate::downloads;
use crate::history;
use crate::ids::UserId;
use crate::state::ConnectionState;

/// A state change, converted from a TDLib update.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    Auth(auth::State),
    /// New terms of service the signed-in user has to accept.
    TermsUpdate {
        id: TermsId,
        terms: Terms,
    },
    Connection(ConnectionState),
    /// The language pack TDLib suggests from the system language
    /// (`suggested_language_pack_id`); empty when there is none.
    SuggestedLanguage(String),
    /// The signed-in user (`my_id`).
    Me(UserId),
    Chats(Change),
    /// A change only open chats' histories show.
    Messages(history::Change),
    Downloads(downloads::Change),
}

/// The events in a client's updates, in TDLib's order. Updates the model does
/// not use yet are dropped here, off the UI thread.
pub fn events(updates: tg::Updates) -> impl Stream<Item = Event> + Send {
    updates.filter_map(|update| future::ready(Event::from_update(update)))
}

impl Event {
    fn from_update(update: Update) -> Option<Self> {
        Some(match update {
            Update::AuthorizationState(update) => {
                Self::Auth(auth::State::from(*update.authorization_state))
            }
            Update::TermsOfService(update) => Self::TermsUpdate {
                id: TermsId(update.terms_of_service_id),
                terms: Terms::from(*update.terms_of_service),
            },
            Update::ConnectionState(update) => {
                Self::Connection(ConnectionState::from(*update.state))
            }
            Update::Option(update) if update.name == "suggested_language_pack_id" => {
                match *update.value {
                    OptionValue::String(value) => Self::SuggestedLanguage(value.value),
                    _ => Self::SuggestedLanguage(String::new()),
                }
            }
            Update::Option(update) if update.name == "my_id" => match *update.value {
                OptionValue::Integer(value) => Self::Me(UserId(value.value)),
                _ => return None,
            },
            other => match Change::from_update(other) {
                Ok(change) => Self::Chats(change),
                Err(other) => match downloads::Change::from_update(other) {
                    Ok(change) => Self::Downloads(change),
                    Err(other) => Self::Messages(history::Change::from_update(other).ok()?),
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(json: &str) -> Option<Event> {
        Event::from_update(serde_json::from_str(json).expect("fixture is a valid update"))
    }

    #[test]
    fn converts_used_updates_and_drops_the_rest() {
        assert_eq!(
            event(
                r#"{"@type":"updateAuthorizationState","authorization_state":{"@type":"authorizationStateReady"}}"#
            ),
            Some(Event::Auth(auth::State::Ready))
        );
        assert_eq!(
            event(
                r#"{"@type":"updateConnectionState","state":{"@type":"connectionStateWaitingForNetwork"}}"#
            ),
            Some(Event::Connection(ConnectionState::WaitingForNetwork))
        );
        assert_eq!(
            event(
                r#"{"@type":"updateOption","name":"version","value":{"@type":"optionValueString","value":"1.8.69"}}"#
            ),
            None
        );
        assert_eq!(
            event(
                r#"{"@type":"updateOption","name":"suggested_language_pack_id","value":{"@type":"optionValueString","value":"ru"}}"#
            ),
            Some(Event::SuggestedLanguage("ru".to_owned()))
        );
    }
}
