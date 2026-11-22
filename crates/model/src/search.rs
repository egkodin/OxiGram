//! Chat list search requests (`Dialogs::Widget` search): known chats,
//! public chats and messages in all chats.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::TopChatCategory;
use td_api::functions::{
    AddRecentlyFoundChat, ClearRecentlyFoundChats, CreatePrivateChat, GetContacts,
    GetRecommendedChats, GetSupportUser, GetTopChats, RemoveRecentlyFoundChat, RemoveTopChat,
    SearchCallMessages, SearchChats, SearchChatsOnServer, SearchMessages, SearchPublicChat,
    SearchPublicChats, SearchRecentlyFoundChats,
};
use td_api::types::{TopChatCategoryUsers, TopChatCategoryWebAppBots};

use crate::chat::ChatList;
use crate::chat_actions::done;
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

impl From<td_api::types::FoundMessages> for FoundMessages {
    fn from(found: td_api::types::FoundMessages) -> Self {
        Self {
            total_count: found.total_count,
            messages: found.messages.into_iter().map(Message::from).collect(),
            next_offset: (!found.next_offset.is_empty()).then_some(found.next_offset),
        }
    }
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

/// Which frequently used chats to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopChats {
    /// People (`TopPeersContent`).
    Users,
    /// Bots whose Web Apps the user opened (`TopBotApps`).
    WebApps,
}

impl TopChats {
    fn to_td(self) -> TopChatCategory {
        match self {
            Self::Users => TopChatCategory::Users(TopChatCategoryUsers {}),
            Self::WebApps => TopChatCategory::WebAppBots(TopChatCategoryWebAppBots {}),
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

    /// The chats of `category` the user opens most, most first.
    pub fn top_chats(
        &self,
        category: TopChats,
        limit: i32,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetTopChats {
                category: Box::new(category.to_td()),
                limit,
            })
            .map(|result| result.map(chat_ids).map_err(RequestError::from))
    }

    pub fn remove_top_chat(
        &self,
        category: TopChats,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&RemoveTopChat {
                category: Box::new(category.to_td()),
                chat_id: chat.0,
            })
            .map(done)
    }

    /// Channels the server suggests joining (`loadRecommendations`).
    pub fn recommended_channels(
        &self,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetRecommendedChats {})
            .map(|result| result.map(chat_ids).map_err(RequestError::from))
    }

    /// Chats the user picked from search results, the latest first.
    pub fn recently_found_chats(
        &self,
        limit: i32,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchRecentlyFoundChats {
                query: String::new(),
                type_filter: None,
                limit,
            })
            .map(|result| result.map(chat_ids).map_err(RequestError::from))
    }

    pub fn add_recently_found_chat(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&AddRecentlyFoundChat { chat_id: chat.0 })
            .map(done)
    }

    pub fn remove_recently_found_chat(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&RemoveRecentlyFoundChat { chat_id: chat.0 })
            .map(done)
    }

    pub fn clear_recently_found_chats(
        &self,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client().request(&ClearRecentlyFoundChats {}).map(done)
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
            .map(|result| result.map(FoundMessages::from).map_err(RequestError::from))
    }

    /// Returns the next page of the account's call history.
    pub fn search_calls(
        &self,
        offset: &str,
        limit: i32,
    ) -> impl Future<Output = Result<FoundMessages, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchCallMessages {
                offset: offset.to_owned(),
                limit,
                only_missed: false,
            })
            .map(|result| result.map(FoundMessages::from).map_err(RequestError::from))
    }

    /// The public chat with `username`, for an @mention.
    pub fn search_public_chat(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<ChatId, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchPublicChat {
                username: username.to_owned(),
            })
            .map(|result| {
                result
                    .map(|chat| ChatId(chat.id))
                    .map_err(RequestError::from)
            })
    }

    /// The private chat with `user`, for a mention by name.
    /// The chat with Telegram's support (`help.getSupport`); opening it
    /// sends nothing.
    pub fn support_chat(
        &self,
    ) -> impl Future<Output = Result<ChatId, RequestError>> + Send + use<> {
        let client = self.client().clone();
        async move {
            let user = client.request(&GetSupportUser {}).await?;
            let chat = client
                .request(&CreatePrivateChat {
                    user_id: user.id,
                    force: false,
                })
                .await?;
            Ok(ChatId(chat.id))
        }
    }

    pub fn create_private_chat(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<ChatId, RequestError>> + Send + use<> {
        self.client()
            .request(&CreatePrivateChat {
                user_id: user.0,
                force: false,
            })
            .map(|result| {
                result
                    .map(|chat| ChatId(chat.id))
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
