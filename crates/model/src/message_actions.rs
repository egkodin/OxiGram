//! What a message's context menu does (`HistoryInner::showContextMenu`):
//! what the user may do with a message, its link, and deleting.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::MessageReadDate;
use td_api::functions::{
    DeleteMessages, GetMessageLink, GetMessageProperties, GetMessageReadDate, GetMessageViewers,
    UnpinChatMessage,
};
use td_api::types::MessageProperties;

use crate::chat_actions::done;
use crate::ids::{ChatId, MessageId, UserId};
use crate::session::{RequestError, Session};

/// What the user may do with one message (`messageProperties`), the part
/// rgram offers so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent permissions, as TDLib reports them"
)]
pub struct Abilities {
    /// Protected content cannot be copied.
    pub can_be_copied: bool,
    pub can_be_deleted_only_for_self: bool,
    pub can_be_deleted_for_all_users: bool,
    pub can_get_link: bool,
    pub can_be_pinned: bool,
    /// An outgoing message in a private chat whose read date TDLib keeps.
    pub can_get_read_date: bool,
    /// An outgoing message in a small group whose viewers TDLib keeps.
    pub can_get_viewers: bool,
}

impl Abilities {
    #[must_use]
    pub fn can_be_deleted(&self) -> bool {
        self.can_be_deleted_only_for_self || self.can_be_deleted_for_all_users
    }
}

impl From<MessageProperties> for Abilities {
    fn from(properties: MessageProperties) -> Self {
        Self {
            can_be_copied: properties.can_be_copied,
            can_be_deleted_only_for_self: properties.can_be_deleted_only_for_self,
            can_be_deleted_for_all_users: properties.can_be_deleted_for_all_users,
            can_get_link: properties.can_get_link,
            can_be_pinned: properties.can_be_pinned,
            can_get_read_date: properties.can_get_read_date,
            can_get_viewers: properties.can_get_viewers,
        }
    }
}

/// When the other user read an outgoing message (`messageReadDate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadDate {
    /// Read at this Unix time.
    Read(i32),
    Unread,
    TooOld,
    /// The other user hides when they read.
    Hidden,
    /// The user hides their own read dates, so sees no one else's.
    MyHidden,
}

impl From<MessageReadDate> for ReadDate {
    fn from(date: MessageReadDate) -> Self {
        match date {
            MessageReadDate::Read(read) => Self::Read(read.read_date),
            MessageReadDate::Unread(_) => Self::Unread,
            MessageReadDate::TooOld(_) => Self::TooOld,
            MessageReadDate::UserPrivacyRestricted(_) => Self::Hidden,
            MessageReadDate::MyPrivacyRestricted(_) => Self::MyHidden,
        }
    }
}

/// For whom messages are deleted (`deleteMessages`' `revoke`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deletion {
    ForMe,
    ForEveryone,
}

/// A message's link and whether anyone can open it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageLink {
    pub link: String,
    pub is_public: bool,
}

impl Session {
    pub fn message_abilities(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<Abilities, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessageProperties {
                chat_id: chat.0,
                message_id: message.0,
            })
            .map(|result| result.map(Abilities::from).map_err(RequestError::from))
    }

    pub fn message_link(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<MessageLink, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessageLink {
                chat_id: chat.0,
                message_id: message.0,
                media_timestamp: 0,
                checklist_task_id: 0,
                poll_option_id: String::new(),
                for_album: false,
                in_message_thread: false,
            })
            .map(|result| {
                result
                    .map(|link| MessageLink {
                        link: link.link,
                        is_public: link.is_public,
                    })
                    .map_err(RequestError::from)
            })
    }

    /// Deletes messages; they leave the history through updates.
    pub fn delete_messages(
        &self,
        chat: ChatId,
        messages: &[MessageId],
        deletion: Deletion,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DeleteMessages {
                chat_id: chat.0,
                message_ids: messages.iter().map(|id| id.0).collect(),
                revoke: deletion == Deletion::ForEveryone,
            })
            .map(done)
    }

    pub fn message_read_date(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<ReadDate, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessageReadDate {
                chat_id: chat.0,
                message_id: message.0,
            })
            .map(|result| result.map(ReadDate::from).map_err(RequestError::from))
    }

    /// Who in a small group saw an outgoing message, most recent first.
    pub fn message_viewers(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<Vec<UserId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessageViewers {
                chat_id: chat.0,
                message_id: message.0,
            })
            .map(|result| {
                result
                    .map(|viewers| {
                        viewers
                            .viewers
                            .into_iter()
                            .map(|viewer| UserId(viewer.user_id))
                            .collect()
                    })
                    .map_err(RequestError::from)
            })
    }

    /// Unpins a message; the pinned bar follows the update.
    pub fn unpin_message(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&UnpinChatMessage {
                chat_id: chat.0,
                message_id: message.0,
            })
            .map(done)
    }
}
