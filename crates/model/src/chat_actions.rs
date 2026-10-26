//! What the chat list's context menu does to a chat
//! (`Window::PeerMenu`): pin, mute, read state, archive, clear and delete.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{BlockList, MessageSender, MessageSource};
use td_api::functions::{
    AddChatToList, DeleteChatHistory, GetChat, LeaveChat, ReadAllChatMentions,
    ReadAllChatReactions, SetChatNotificationSettings, SetMessageSenderBlockList,
    ToggleChatIsMarkedAsUnread, ToggleChatIsPinned, ViewMessages,
};
use td_api::types::{BlockListMain, MessageSenderUser, MessageSourceChatList};

use crate::chat::ChatList;
use crate::ids::{ChatId, MessageId, UserId};
use crate::session::{RequestError, Session};

/// TDLib mutes for good past a year (`mute_for` in seconds).
const MUTE_FOREVER: i32 = i32::MAX;

/// How deleting a chat leaves it (`DeleteAndLeaveHandler`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// Private and secret chats: the history goes, for the user only.
    DeleteHistory,
    /// Basic groups: leave, then drop the history.
    LeaveAndDelete,
    /// Supergroups and channels: leaving removes them from the list.
    Leave,
}

/// Whether a chat sits at the top of its list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pinning {
    Pinned,
    Unpinned,
}

/// Whether a chat's notifications are silenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Muting {
    Muted,
    Unmuted,
}

/// The user's own unread mark on a chat (`isMarkedAsUnread`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadMark {
    Marked,
    Cleared,
}

fn done(result: Result<td_api::types::Ok, tg::TdError>) -> Result<(), RequestError> {
    result.map(drop).map_err(RequestError::from)
}

impl Session {
    pub fn set_chat_pinned(
        &self,
        list: ChatList,
        chat: ChatId,
        pinning: Pinning,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ToggleChatIsPinned {
                chat_list: Box::new(list.to_td()),
                chat_id: chat.0,
                is_pinned: pinning == Pinning::Pinned,
            })
            .map(done)
    }

    /// Moves `chat` to the archive or back to the main list.
    pub fn move_chat_to(
        &self,
        chat: ChatId,
        list: ChatList,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&AddChatToList {
                chat_id: chat.0,
                chat_list: Box::new(list.to_td()),
            })
            .map(done)
    }

    /// Mutes `chat` for good, or unmutes it, keeping its other
    /// notification settings as they are.
    pub fn set_chat_muted(
        &self,
        chat: ChatId,
        muting: Muting,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let client = self.client().clone();
        async move {
            let current = client
                .request(&GetChat { chat_id: chat.0 })
                .await
                .map_err(RequestError::from)?;
            let mut settings = *current.notification_settings;
            settings.use_default_mute_for = false;
            settings.mute_for = match muting {
                Muting::Muted => MUTE_FOREVER,
                Muting::Unmuted => 0,
            };
            client
                .request(&SetChatNotificationSettings {
                    chat_id: chat.0,
                    notification_settings: Box::new(settings),
                })
                .await
                .map(drop)
                .map_err(RequestError::from)
        }
    }

    pub fn set_chat_marked_unread(
        &self,
        chat: ChatId,
        mark: UnreadMark,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ToggleChatIsMarkedAsUnread {
                chat_id: chat.0,
                is_marked_as_unread: mark == UnreadMark::Marked,
            })
            .map(done)
    }

    /// Reads everything in `chat` up to `last`, its mentions and reactions
    /// (`MarkAsReadThread`).
    pub fn read_chat(
        &self,
        chat: ChatId,
        last: Option<MessageId>,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let view = last.map(|last| {
            self.client().request(&ViewMessages {
                chat_id: chat.0,
                message_ids: vec![last.0],
                source: Some(Box::new(MessageSource::ChatList(MessageSourceChatList {}))),
                force_read: true,
            })
        });
        let mentions = self
            .client()
            .request(&ReadAllChatMentions { chat_id: chat.0 });
        let reactions = self
            .client()
            .request(&ReadAllChatReactions { chat_id: chat.0 });
        let unmark = self.set_chat_marked_unread(chat, UnreadMark::Cleared);
        async move {
            if let Some(view) = view {
                done(view.await)?;
            }
            done(mentions.await)?;
            done(reactions.await)?;
            unmark.await
        }
    }

    /// Deletes the messages of `chat` for the user, keeping it in the list.
    pub fn clear_chat_history(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DeleteChatHistory {
                chat_id: chat.0,
                remove_from_chat_list: false,
                revoke: false,
            })
            .map(done)
    }

    pub fn remove_chat(
        &self,
        chat: ChatId,
        removal: Removal,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let leave = matches!(removal, Removal::Leave | Removal::LeaveAndDelete)
            .then(|| self.client().request(&LeaveChat { chat_id: chat.0 }));
        let delete =
            matches!(removal, Removal::DeleteHistory | Removal::LeaveAndDelete).then(|| {
                self.client().request(&DeleteChatHistory {
                    chat_id: chat.0,
                    remove_from_chat_list: true,
                    revoke: false,
                })
            });
        async move {
            if let Some(leave) = leave {
                done(leave.await)?;
            }
            if let Some(delete) = delete {
                done(delete.await)?;
            }
            Ok(())
        }
    }

    pub fn block_user(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetMessageSenderBlockList {
                sender_id: Box::new(MessageSender::User(MessageSenderUser { user_id: user.0 })),
                block_list: Some(Box::new(BlockList::Main(BlockListMain {}))),
            })
            .map(done)
    }
}
