//! The own account's settings that Telegram keeps: the username's check
//! and the personal channel (`Settings::Information`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::CheckChatUsernameResult;
use td_api::functions::{CheckChatUsername, GetSuitablePersonalChats, SetPersonalChat};

use crate::{ChatId, RequestError, Session, UserId};

/// What Telegram says of a username before it is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameCheck {
    Available,
    Invalid,
    Occupied,
    /// Taken, but on sale at Fragment.
    Purchasable,
    TooManyPublic,
    Unavailable,
}

impl From<CheckChatUsernameResult> for UsernameCheck {
    fn from(result: CheckChatUsernameResult) -> Self {
        match result {
            CheckChatUsernameResult::Ok(_) => Self::Available,
            CheckChatUsernameResult::UsernameInvalid(_) => Self::Invalid,
            CheckChatUsernameResult::UsernameOccupied(_) => Self::Occupied,
            CheckChatUsernameResult::UsernamePurchasable(_) => Self::Purchasable,
            CheckChatUsernameResult::PublicChatsTooMany(_) => Self::TooManyPublic,
            CheckChatUsernameResult::PublicGroupsUnavailable(_) => Self::Unavailable,
        }
    }
}

impl Session {
    /// Whether `me` may take `username` (`account.checkUsername`).
    pub fn check_username(
        &self,
        me: UserId,
        username: &str,
    ) -> impl Future<Output = Result<UsernameCheck, RequestError>> + Send + use<> {
        self.client()
            .request(&CheckChatUsername {
                // The private chat with oneself has one's own id.
                chat_id: me.0,
                username: username.to_owned(),
            })
            .map(|result| result.map(UsernameCheck::from).map_err(RequestError::from))
    }

    /// The public channels one owns that may show on one's profile.
    pub fn suitable_personal_chats(
        &self,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetSuitablePersonalChats {})
            .map(|result| {
                result
                    .map(|chats| chats.chat_ids.into_iter().map(ChatId).collect())
                    .map_err(RequestError::from)
            })
    }

    /// Shows `chat` on one's profile, or none.
    pub fn set_personal_chat(
        &self,
        chat: Option<ChatId>,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetPersonalChat {
                chat_id: chat.map_or(0, |chat| chat.0),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}
