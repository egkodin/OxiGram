//! Chat list search requests (`Dialogs::Widget` search): known chats,
//! public chats and messages in all chats.

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{
    GetContacts, SearchChats, SearchChatsOnServer, SearchMessages, SearchPublicChats,
};

use crate::ids::{ChatId, UserId};
use crate::message::Message;
use crate::session::{RequestError, Session};

/// One page of messages found in all chats.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoundMessages {
    /// All matches on the server, an approximation.
    pub total_count: i32,
    pub messages: Vec<Message>,
    /// Passed back to get the next page; `None` once nothing is left.
    pub next_offset: Option<String>,
}

fn chat_ids(chats: td_api::types::Chats) -> Vec<ChatId> {
    chats.chat_ids.into_iter().map(ChatId).collect()
}

impl Session {
    /// The user's contacts; TDLib sends each of them as a user update first.
    pub fn contacts(
        &self,
    ) -> impl Future<Output = Result<Vec<UserId>, RequestError>> + Send + use<> {
        self.client().request(&GetContacts {}).map(|result| {
            result
                .map(|users| users.user_ids.into_iter().map(UserId).collect())
                .map_err(RequestError::from)
        })
    }

    /// Known chats whose title or username matches, offline.
    pub fn search_chats(
        &self,
        query: &str,
        limit: i32,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchChats {
                query: query.to_owned(),
                type_filter: None,
                limit,
            })
            .map(|result| result.map(chat_ids).map_err(RequestError::from))
    }

    /// The user's own chats and contacts that match, asked from the server.
    pub fn search_chats_on_server(
        &self,
        query: &str,
        limit: i32,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchChatsOnServer {
                query: query.to_owned(),
                type_filter: None,
                limit,
            })
            .map(|result| result.map(chat_ids).map_err(RequestError::from))
    }

    /// Public chats by username or title (global search).
    pub fn search_public_chats(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchPublicChats {
                query: query.to_owned(),
                type_filter: None,
            })
            .map(|result| result.map(chat_ids).map_err(RequestError::from))
    }

    /// Messages in all chats, newest first; `offset` is empty for the first
    /// page.
    pub fn search_messages(
        &self,
        query: &str,
        offset: &str,
        limit: i32,
    ) -> impl Future<Output = Result<FoundMessages, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchMessages {
                chat_list: None,
                query: query.to_owned(),
                offset: offset.to_owned(),
                limit,
                filter: None,
                chat_type_filter: None,
                min_date: 0,
                max_date: 0,
            })
            .map(|result| {
                result
                    .map(|found| FoundMessages {
                        total_count: found.total_count,
                        messages: found.messages.into_iter().map(Message::from).collect(),
                        next_offset: (!found.next_offset.is_empty()).then_some(found.next_offset),
                    })
                    .map_err(RequestError::from)
            })
    }
}
