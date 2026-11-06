//! What the chat list's context menu does to a chat
//! (`Window::PeerMenu`): pin, mute, read state, archive, clear and delete.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{BlockList, MessageSender, MessageSource};
use td_api::functions::{
    AddChatToList, DeleteChatFolder, DeleteChatHistory, GetChat, JoinChat, LeaveChat,
    OpenMessageContent, ReadAllChatMentions, ReadAllChatReactions, ReorderChatFolders,
    SetChatNotificationSettings, SetMessageSenderBlockList, SetPinnedChats,
    ToggleChatIsMarkedAsUnread, ToggleChatIsPinned, ViewMessages,
};
use td_api::types::{
    BlockListMain, MessageSenderUser, MessageSourceChatHistory, MessageSourceChatList,
};

use crate::chat::ChatList;
use crate::ids::{ChatId, FolderId, MessageId, UserId};
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

pub(crate) fn done(result: Result<td_api::types::Ok, tg::TdError>) -> Result<(), RequestError> {
    result.map(drop).map_err(RequestError::from)
}

impl Session {
    /// Removes a folder; its chats stay (`RemoveChatFilter`).
    // shortcut: never leaves the folder's shared chats; the leave-chats box for shared folders comes with folder settings in M6.
    pub fn delete_folder(
        &self,
        folder: FolderId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DeleteChatFolder {
                chat_folder_id: folder.0,
                leave_chat_ids: Vec::new(),
            })
            .map(done)
    }

    /// Saves the folders' order; "All chats" stays first.
    // shortcut: always puts the main list first; moving it is Premium (`main_chat_list_position`), M13.
    pub fn reorder_folders(
        &self,
        order: &[FolderId],
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ReorderChatFolders {
                chat_folder_ids: order.iter().map(|folder| folder.0).collect(),
                main_chat_list_position: 0,
            })
            .map(done)
    }

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

    /// Sets the order of the pinned chats of `list`, top first.
    pub fn set_pinned_order(
        &self,
        list: ChatList,
        chats: &[ChatId],
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetPinnedChats {
                chat_list: Box::new(list.to_td()),
                chat_ids: chats.iter().map(|chat| chat.0).collect(),
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

    /// Tells TDLib the user played `message`'s voice or round video, which
    /// marks it listened; nothing with `Views::Withhold`.
    pub fn open_message_content(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let request = (self.views() == crate::Views::Report).then(|| {
            self.client().request(&OpenMessageContent {
                chat_id: chat.0,
                message_id: message.0,
            })
        });
        async move {
            match request {
                Some(request) => done(request.await),
                None => Ok(()),
            }
        }
    }

    /// Tells TDLib the user saw `message` in the chat, which reads it and
    /// its mention or reactions; nothing with `Views::Withhold`.
    pub fn view_message(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.view_messages(chat, vec![message])
    }

    /// Tells TDLib the user sees `messages` in the chat's history, which
    /// refreshes their views and reactions and reads them; nothing with
    /// `Views::Withhold`.
    pub fn view_messages(
        &self,
        chat: ChatId,
        messages: Vec<MessageId>,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let request = (self.views() == crate::Views::Report).then(|| {
            self.client().request(&ViewMessages {
                chat_id: chat.0,
                message_ids: messages.into_iter().map(|id| id.0).collect(),
                source: Some(Box::new(MessageSource::ChatHistory(
                    MessageSourceChatHistory {},
                ))),
                force_read: false,
            })
        });
        async move {
            match request {
                Some(request) => done(request.await),
                None => Ok(()),
            }
        }
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

    pub fn unblock_user(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetMessageSenderBlockList {
                sender_id: Box::new(MessageSender::User(MessageSenderUser { user_id: user.0 })),
                block_list: None,
            })
            .map(done)
    }

    /// Joins a public group or channel (`joinChat`); TDLib reports the new
    /// standing in an update.
    pub fn join_chat(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&JoinChat { chat_id: chat.0 })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}
