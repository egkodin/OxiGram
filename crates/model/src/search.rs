//! Chat list search requests (`Dialogs::Widget` search): known chats,
//! public chats and messages in all chats.

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{
    GetContacts, SearchChats, SearchChatsOnServer, SearchMessages, SearchPublicChats,
};

use crate::chat::ChatList;
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

/// Which kind of chat found messages come from (`ChatTypeFilter`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChatTypeFilter {
    #[default]
    All,
    Private,
    Groups,
    Channels,
}

/// Whether archived chats are searched too (`fromArchive`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArchiveScope {
    #[default]
    Included,
    Excluded,
}

/// Narrows a search for messages in all chats.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MessageFilter {
    pub chat_type: ChatTypeFilter,
    pub archive: ArchiveScope,
}

impl MessageFilter {
    fn chat_type_to_td(self) -> Option<Box<td_api::enums::SearchMessagesChatTypeFilter>> {
        use td_api::enums::SearchMessagesChatTypeFilter as Td;
        use td_api::types::{
            SearchMessagesChatTypeFilterChannel, SearchMessagesChatTypeFilterGroup,
            SearchMessagesChatTypeFilterPrivate,
        };
        let filter = match self.chat_type {
            ChatTypeFilter::All => return None,
            ChatTypeFilter::Private => Td::Private(SearchMessagesChatTypeFilterPrivate {}),
            ChatTypeFilter::Groups => Td::Group(SearchMessagesChatTypeFilterGroup {}),
            ChatTypeFilter::Channels => Td::Channel(SearchMessagesChatTypeFilterChannel {}),
        };
        Some(Box::new(filter))
    }

    /// No chat list searches all of them, the archive included.
    fn chat_list_to_td(self) -> Option<Box<td_api::enums::ChatList>> {
        match self.archive {
            ArchiveScope::Included => None,
            ArchiveScope::Excluded => Some(Box::new(ChatList::Main.to_td())),
        }
    }
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

    /// Messages in all chats that pass `filter`, newest first; `offset` is
    /// empty for the first page.
    pub fn search_messages(
        &self,
        query: &str,
        filter: MessageFilter,
        offset: &str,
        limit: i32,
    ) -> impl Future<Output = Result<FoundMessages, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchMessages {
                chat_list: filter.chat_list_to_td(),
                query: query.to_owned(),
                offset: offset.to_owned(),
                limit,
                filter: None,
                chat_type_filter: filter.chat_type_to_td(),
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

#[cfg(test)]
mod tests {
    use td_api::enums::{ChatList as TdChatList, SearchMessagesChatTypeFilter};

    use super::*;

    #[test]
    fn default_filter_searches_every_chat() {
        let filter = MessageFilter::default();
        assert_eq!(filter.chat_list_to_td(), None);
        assert_eq!(filter.chat_type_to_td(), None);
    }

    #[test]
    fn narrowed_filter_names_the_main_list_and_chat_type() {
        let filter = MessageFilter {
            chat_type: ChatTypeFilter::Groups,
            archive: ArchiveScope::Excluded,
        };
        assert!(matches!(
            filter.chat_list_to_td().as_deref(),
            Some(TdChatList::Main(_))
        ));
        assert!(matches!(
            filter.chat_type_to_td().as_deref(),
            Some(SearchMessagesChatTypeFilter::Group(_))
        ));
    }
}
