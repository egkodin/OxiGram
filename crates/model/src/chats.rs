//! The chat list's part of the state: chats, users, files and the order of
//! every list, kept up to date from TDLib's updates.

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap};

use td_api::enums::{ChatAction as TdChatAction, NotificationSettingsScope, Update};

use crate::chat::{self, Chat, ChatKind, ChatList, Draft, Folder, Position};
use crate::file::File;
use crate::ids::{ChatId, FileId, MessageId, UserId};
use crate::message::{Content, Message, Sender};
use crate::user::{Photo, User, UserStatus};

/// A change to chats, users or files.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    Chat(Box<Chat>),
    Title {
        chat: ChatId,
        title: String,
    },
    Photo {
        chat: ChatId,
        photo: Option<Photo>,
    },
    LastMessage {
        chat: ChatId,
        message: Option<Box<Message>>,
        positions: Vec<Position>,
    },
    Position {
        chat: ChatId,
        position: Position,
    },
    ReadInbox {
        chat: ChatId,
        last_read: MessageId,
        unread_count: i32,
    },
    ReadOutbox {
        chat: ChatId,
        last_read: MessageId,
    },
    Mentions {
        chat: ChatId,
        count: i32,
    },
    Reactions {
        chat: ChatId,
        count: i32,
    },
    MarkedAsUnread {
        chat: ChatId,
        is_marked: bool,
    },
    Mute {
        chat: ChatId,
        mute_for: i32,
        uses_default: bool,
    },
    DefaultMute {
        scope: MuteScope,
        mute_for: i32,
    },
    Draft {
        chat: ChatId,
        draft: Option<Draft>,
        positions: Vec<Position>,
    },
    MessageContent {
        chat: ChatId,
        message: MessageId,
        content: Box<Content>,
    },
    MessageSent {
        message: Box<Message>,
        old: MessageId,
    },
    Action {
        chat: ChatId,
        sender: Sender,
        action: Option<ChatAction>,
    },
    Folders(Vec<Folder>),
    UnreadChats {
        list: ChatList,
        unread: Unread,
    },
    User(Box<User>),
    UserStatus {
        user: UserId,
        status: UserStatus,
    },
    File(File),
}

/// The chat types notification defaults are set for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MuteScope {
    Private,
    Group,
    Channel,
}

/// What someone is doing in a chat right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatAction {
    Typing,
    RecordingVideo,
    UploadingVideo,
    RecordingVoice,
    UploadingVoice,
    UploadingPhoto,
    UploadingDocument,
    ChoosingSticker,
    ChoosingLocation,
    ChoosingContact,
    PlayingGame,
    RecordingVideoNote,
    UploadingVideoNote,
    WatchingAnimations(String),
}

/// Unread chats in one list, for the folder tab counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unread {
    pub chats: i32,
    pub unmuted_chats: i32,
}

/// A list's chats: descending order, then the chat id for ties.
type Ordered = BTreeSet<(Reverse<i64>, Reverse<ChatId>)>;

/// Every known chat and user, and the chat lists in TDLib's order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chats {
    chats: HashMap<ChatId, Chat>,
    users: HashMap<UserId, User>,
    files: HashMap<FileId, File>,
    lists: HashMap<ChatList, Ordered>,
    default_mute: HashMap<MuteScope, i32>,
    actions: HashMap<ChatId, Vec<(Sender, ChatAction)>>,
    folders: Vec<Folder>,
    unread: HashMap<ChatList, Unread>,
}

impl Chats {
    #[must_use]
    pub fn chat(&self, id: ChatId) -> Option<&Chat> {
        self.chats.get(&id)
    }

    #[must_use]
    pub fn user(&self, id: UserId) -> Option<&User> {
        self.users.get(&id)
    }

    #[must_use]
    pub fn file(&self, id: FileId) -> Option<&File> {
        self.files.get(&id)
    }

    /// The chats of `list` from the top.
    pub fn list(&self, list: ChatList) -> impl Iterator<Item = ChatId> + '_ {
        self.lists
            .get(&list)
            .into_iter()
            .flatten()
            .map(|(_, Reverse(id))| *id)
    }

    #[must_use]
    pub fn list_len(&self, list: ChatList) -> usize {
        self.lists.get(&list).map_or(0, BTreeSet::len)
    }

    #[must_use]
    pub fn folders(&self) -> &[Folder] {
        &self.folders
    }

    #[must_use]
    pub fn unread(&self, list: ChatList) -> Unread {
        self.unread.get(&list).copied().unwrap_or_default()
    }

    /// What others are doing in the chat, oldest first.
    #[must_use]
    pub fn actions(&self, chat: ChatId) -> &[(Sender, ChatAction)] {
        self.actions.get(&chat).map_or(&[], Vec::as_slice)
    }

    /// Whether the chat is muted, its own setting or its type's default.
    #[must_use]
    pub fn is_muted(&self, chat: &Chat) -> bool {
        if !chat.uses_default_mute {
            return chat.is_muted();
        }
        let scope = match chat.kind {
            ChatKind::Private { .. } | ChatKind::Secret { .. } => MuteScope::Private,
            ChatKind::Group | ChatKind::Supergroup { is_channel: false } => MuteScope::Group,
            ChatKind::Supergroup { is_channel: true } => MuteScope::Channel,
        };
        self.default_mute
            .get(&scope)
            .is_some_and(|mute_for| *mute_for > 0)
    }

    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Chat(chat) => {
                let mut chat = *chat;
                let id = chat.id;
                let positions = std::mem::take(&mut chat.positions);
                if let Some(old) = self.chats.insert(id, chat) {
                    self.unlist(&old);
                }
                for position in positions {
                    self.place(id, position);
                }
            }
            Change::LastMessage {
                chat,
                message,
                positions,
            } => {
                self.update(chat, |chat| {
                    chat.last_message = message.map(|message| *message)
                });
                for position in positions {
                    self.place(chat, position);
                }
            }
            Change::Draft {
                chat,
                draft,
                positions,
            } => {
                self.update(chat, |chat| chat.draft = draft);
                for position in positions {
                    self.place(chat, position);
                }
            }
            Change::Position { chat, position } => self.place(chat, position),
            Change::Title { chat, title } => self.update(chat, |chat| chat.title = title),
            Change::Photo { chat, photo } => self.update(chat, |chat| chat.photo = photo),
            Change::ReadInbox {
                chat,
                last_read,
                unread_count,
            } => self.update(chat, |chat| {
                chat.last_read_inbox = last_read;
                chat.unread_count = unread_count;
            }),
            Change::ReadOutbox { chat, last_read } => {
                self.update(chat, |chat| chat.last_read_outbox = last_read);
            }
            Change::Mentions { chat, count } => {
                self.update(chat, |chat| chat.unread_mentions = count);
            }
            Change::Reactions { chat, count } => {
                self.update(chat, |chat| chat.unread_reactions = count);
            }
            Change::MarkedAsUnread { chat, is_marked } => {
                self.update(chat, |chat| chat.is_marked_as_unread = is_marked);
            }
            Change::Mute {
                chat,
                mute_for,
                uses_default,
            } => self.update(chat, |chat| {
                chat.mute_for = mute_for;
                chat.uses_default_mute = uses_default;
            }),
            Change::DefaultMute { scope, mute_for } => {
                self.default_mute.insert(scope, mute_for);
            }
            Change::MessageContent {
                chat,
                message,
                content,
            } => self.update(chat, |chat| {
                if let Some(last) = chat.last_message.as_mut().filter(|last| last.id == message) {
                    last.content = *content;
                }
            }),
            Change::MessageSent { message, old } => {
                let message = *message;
                self.update(message.chat, |chat| {
                    if chat
                        .last_message
                        .as_ref()
                        .is_some_and(|last| last.id == old)
                    {
                        chat.last_message = Some(message);
                    }
                });
            }
            Change::Action {
                chat,
                sender,
                action,
            } => self.set_action(chat, sender, action),
            Change::Folders(folders) => self.folders = folders,
            Change::UnreadChats { list, unread } => {
                self.unread.insert(list, unread);
            }
            Change::User(user) => {
                self.users.insert(user.id, *user);
            }
            Change::UserStatus { user, status } => {
                if let Some(user) = self.users.get_mut(&user) {
                    user.status = status;
                }
            }
            Change::File(file) => {
                self.files.insert(file.id, file);
            }
        }
    }

    fn update(&mut self, chat: ChatId, change: impl FnOnce(&mut Chat)) {
        match self.chats.get_mut(&chat) {
            Some(chat) => change(chat),
            // TDLib sends updateNewChat before any other update about a chat.
            None => tracing::warn!(chat = chat.0, "update for an unknown chat"),
        }
    }

    fn unlist(&mut self, chat: &Chat) {
        for position in &chat.positions {
            if let Some(list) = self.lists.get_mut(&position.list) {
                list.remove(&(Reverse(position.order), Reverse(chat.id)));
            }
        }
    }

    /// Moves the chat within `position.list`; an order of 0 takes it out.
    fn place(&mut self, id: ChatId, position: Position) {
        let Some(chat) = self.chats.get_mut(&id) else {
            tracing::warn!(chat = id.0, "position for an unknown chat");
            return;
        };
        let list = self.lists.entry(position.list).or_default();
        if let Some(old) = chat.position(position.list) {
            list.remove(&(Reverse(old.order), Reverse(id)));
        }
        if position.order != 0 {
            list.insert((Reverse(position.order), Reverse(id)));
        }
        chat.set_position(position);
    }

    fn set_action(&mut self, chat: ChatId, sender: Sender, action: Option<ChatAction>) {
        let actions = self.actions.entry(chat).or_default();
        actions.retain(|(who, _)| *who != sender);
        if let Some(action) = action {
            actions.push((sender, action));
        }
        if actions.is_empty() {
            self.actions.remove(&chat);
        }
    }
}

impl Change {
    pub(crate) fn from_update(update: Update) -> Result<Self, Update> {
        Ok(match update {
            Update::NewChat(update) => Self::Chat(Box::new(Chat::from(*update.chat))),
            Update::ChatTitle(update) => Self::Title {
                chat: ChatId(update.chat_id),
                title: update.title,
            },
            Update::ChatPhoto(update) => Self::Photo {
                chat: ChatId(update.chat_id),
                photo: update.photo.map(|photo| chat::photo_of(*photo)),
            },
            Update::ChatLastMessage(update) => Self::LastMessage {
                chat: ChatId(update.chat_id),
                message: update
                    .last_message
                    .map(|message| Box::new(Message::from(*message))),
                positions: update.positions.into_iter().map(Position::from).collect(),
            },
            Update::ChatPosition(update) => Self::Position {
                chat: ChatId(update.chat_id),
                position: Position::from(*update.position),
            },
            Update::ChatReadInbox(update) => Self::ReadInbox {
                chat: ChatId(update.chat_id),
                last_read: MessageId(update.last_read_inbox_message_id),
                unread_count: update.unread_count,
            },
            Update::ChatReadOutbox(update) => Self::ReadOutbox {
                chat: ChatId(update.chat_id),
                last_read: MessageId(update.last_read_outbox_message_id),
            },
            Update::ChatUnreadMentionCount(update) => Self::Mentions {
                chat: ChatId(update.chat_id),
                count: update.unread_mention_count,
            },
            Update::ChatUnreadReactionCount(update) => Self::Reactions {
                chat: ChatId(update.chat_id),
                count: update.unread_reaction_count,
            },
            Update::ChatIsMarkedAsUnread(update) => Self::MarkedAsUnread {
                chat: ChatId(update.chat_id),
                is_marked: update.is_marked_as_unread,
            },
            Update::ChatNotificationSettings(update) => Self::Mute {
                chat: ChatId(update.chat_id),
                mute_for: update.notification_settings.mute_for,
                uses_default: update.notification_settings.use_default_mute_for,
            },
            Update::ScopeNotificationSettings(update) => Self::DefaultMute {
                scope: match *update.scope {
                    NotificationSettingsScope::PrivateChats(_) => MuteScope::Private,
                    NotificationSettingsScope::GroupChats(_) => MuteScope::Group,
                    NotificationSettingsScope::ChannelChats(_) => MuteScope::Channel,
                },
                mute_for: update.notification_settings.mute_for,
            },
            Update::ChatDraftMessage(update) => Self::Draft {
                chat: ChatId(update.chat_id),
                draft: update
                    .draft_message
                    .and_then(|draft| chat::draft_of(*draft)),
                positions: update.positions.into_iter().map(Position::from).collect(),
            },
            Update::MessageContent(update) => Self::MessageContent {
                chat: ChatId(update.chat_id),
                message: MessageId(update.message_id),
                content: Box::new(Content::from(*update.new_content)),
            },
            Update::MessageSendSucceeded(update) => Self::MessageSent {
                message: Box::new(Message::from(*update.message)),
                old: MessageId(update.old_message_id),
            },
            Update::ChatAction(update) if update.topic_id.is_none() => Self::Action {
                chat: ChatId(update.chat_id),
                sender: Sender::from(*update.sender_id),
                action: ChatAction::from_td(*update.action),
            },
            Update::ChatFolders(update) => {
                Self::Folders(update.chat_folders.into_iter().map(Folder::from).collect())
            }
            Update::UnreadChatCount(update) => Self::UnreadChats {
                list: ChatList::from(*update.chat_list),
                unread: Unread {
                    chats: update.unread_count,
                    unmuted_chats: update.unread_unmuted_count,
                },
            },
            Update::User(update) => Self::User(Box::new(User::from(*update.user))),
            Update::UserStatus(update) => Self::UserStatus {
                user: UserId(update.user_id),
                status: UserStatus::from(*update.status),
            },
            Update::File(update) => Self::File(File::from(*update.file)),
            other => return Err(other),
        })
    }
}

impl ChatAction {
    /// `None` for `chatActionCancel`.
    fn from_td(action: TdChatAction) -> Option<Self> {
        Some(match action {
            TdChatAction::Typing(_) => Self::Typing,
            TdChatAction::RecordingVideo(_) => Self::RecordingVideo,
            TdChatAction::UploadingVideo(_) => Self::UploadingVideo,
            TdChatAction::RecordingVoiceNote(_) => Self::RecordingVoice,
            TdChatAction::UploadingVoiceNote(_) => Self::UploadingVoice,
            TdChatAction::UploadingPhoto(_) => Self::UploadingPhoto,
            TdChatAction::UploadingDocument(_) => Self::UploadingDocument,
            TdChatAction::ChoosingSticker(_) => Self::ChoosingSticker,
            TdChatAction::ChoosingLocation(_) => Self::ChoosingLocation,
            TdChatAction::ChoosingContact(_) => Self::ChoosingContact,
            TdChatAction::StartPlayingGame(_) => Self::PlayingGame,
            TdChatAction::RecordingVideoNote(_) => Self::RecordingVideoNote,
            TdChatAction::UploadingVideoNote(_) => Self::UploadingVideoNote,
            TdChatAction::WatchingAnimations(action) => Self::WatchingAnimations(action.emoji),
            TdChatAction::Cancel(_) => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: i64, order: i64) -> Change {
        Change::Chat(Box::new(Chat {
            id: ChatId(id),
            kind: ChatKind::Private { user: UserId(id) },
            title: format!("chat {id}"),
            photo: None,
            accent_color: 0,
            last_message: None,
            positions: vec![Position {
                list: ChatList::Main,
                order,
                is_pinned: false,
            }],
            unread_count: 0,
            last_read_inbox: MessageId(0),
            last_read_outbox: MessageId(0),
            unread_mentions: 0,
            unread_reactions: 0,
            is_marked_as_unread: false,
            mute_for: 0,
            uses_default_mute: true,
            draft: None,
        }))
    }

    fn main_list(chats: &Chats) -> Vec<i64> {
        chats.list(ChatList::Main).map(|id| id.0).collect()
    }

    fn position(chat: i64, order: i64) -> Change {
        Change::Position {
            chat: ChatId(chat),
            position: Position {
                list: ChatList::Main,
                order,
                is_pinned: false,
            },
        }
    }

    #[test]
    fn orders_by_descending_order_and_moves_on_updates() {
        let mut chats = Chats::default();
        chats.apply(chat(1, 10));
        chats.apply(chat(2, 30));
        chats.apply(chat(3, 20));
        assert_eq!(main_list(&chats), [2, 3, 1]);

        chats.apply(position(1, 40));
        assert_eq!(main_list(&chats), [1, 2, 3]);

        // Order 0 takes the chat out of the list.
        chats.apply(position(2, 0));
        assert_eq!(main_list(&chats), [1, 3]);
        assert_eq!(chats.chat(ChatId(2)).map(|c| c.positions.len()), Some(0));
    }

    #[test]
    fn a_new_last_message_reorders_with_its_positions() {
        let mut chats = Chats::default();
        chats.apply(chat(1, 10));
        chats.apply(chat(2, 20));
        chats.apply(Change::LastMessage {
            chat: ChatId(1),
            message: None,
            positions: vec![Position {
                list: ChatList::Main,
                order: 30,
                is_pinned: false,
            }],
        });
        assert_eq!(main_list(&chats), [1, 2]);
        assert_eq!(chats.list_len(ChatList::Main), 2);
    }

    #[test]
    fn default_mute_applies_by_chat_type() {
        let mut chats = Chats::default();
        chats.apply(chat(1, 10));
        let muted = |chats: &Chats| chats.is_muted(chats.chat(ChatId(1)).expect("chat 1 exists"));
        assert!(!muted(&chats));
        chats.apply(Change::DefaultMute {
            scope: MuteScope::Private,
            mute_for: i32::MAX,
        });
        assert!(muted(&chats));
        chats.apply(Change::Mute {
            chat: ChatId(1),
            mute_for: 0,
            uses_default: false,
        });
        assert!(!muted(&chats));
    }

    #[test]
    fn a_cancelled_action_is_removed() {
        let mut chats = Chats::default();
        let sender = Sender::User(UserId(5));
        chats.apply(Change::Action {
            chat: ChatId(1),
            sender,
            action: Some(ChatAction::Typing),
        });
        assert_eq!(chats.actions(ChatId(1)), [(sender, ChatAction::Typing)]);
        chats.apply(Change::Action {
            chat: ChatId(1),
            sender,
            action: None,
        });
        assert!(chats.actions(ChatId(1)).is_empty());
    }
}
