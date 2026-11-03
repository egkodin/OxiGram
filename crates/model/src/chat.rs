//! Chats and the lists TDLib orders them in.

use td_api::enums::{ChatList as TdChatList, ChatType, DraftMessageContent};

use crate::action_bar::ActionBar;
use crate::file::Minithumbnail;
use crate::ids::{BasicGroupId, ChatId, FolderId, MessageId, SupergroupId, UserId};
use crate::message::Message;
use crate::rights::Permissions;
use crate::text::FormattedText;
use crate::user::Photo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    pub id: ChatId,
    pub kind: ChatKind,
    pub title: String,
    pub photo: Option<Photo>,
    pub accent_color: i32,
    pub last_message: Option<Message>,
    pub positions: Vec<Position>,
    pub unread_count: i32,
    pub last_read_inbox: MessageId,
    pub last_read_outbox: MessageId,
    pub unread_mentions: i32,
    pub unread_reactions: i32,
    pub is_marked_as_unread: bool,
    /// Seconds the chat stays muted for; 0 when not muted.
    pub mute_for: i32,
    /// Whether `mute_for` comes from the defaults for the chat's type.
    pub uses_default_mute: bool,
    pub draft: Option<Draft>,
    pub action_bar: Option<ActionBar>,
    pub permissions: Permissions,
    /// The other user of a private chat is blocked.
    pub is_blocked: bool,
    /// Whether it has scheduled messages, which its field offers to show.
    pub has_scheduled_messages: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatKind {
    Private {
        user: UserId,
    },
    /// A legacy small group.
    Group {
        group: BasicGroupId,
    },
    Supergroup {
        supergroup: SupergroupId,
        is_channel: bool,
    },
    Secret {
        user: UserId,
    },
}

/// One of the lists the chat list screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatList {
    Main,
    Archive,
    Folder(FolderId),
}

/// Where a chat sits in one list. Lists are sorted by descending `order`;
/// pinned chats have the largest orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub list: ChatList,
    pub order: i64,
    pub is_pinned: bool,
}

/// An unsent message the user left in the input field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub date: i32,
    pub text: FormattedText,
}

/// A chat folder's tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub id: FolderId,
    pub name: FormattedText,
    /// Removing it also turns off the invite links the user made.
    pub has_my_invite_links: bool,
    /// TDLib's icon name, such as "Work" or "Custom"; TDLib picks one when
    /// the user did not.
    pub icon: String,
}

impl Chat {
    /// A chat in no list with nothing unread, as fixtures build them.
    #[must_use]
    pub fn new(id: ChatId, kind: ChatKind, title: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            title: title.into(),
            photo: None,
            accent_color: 0,
            last_message: None,
            positions: Vec::new(),
            unread_count: 0,
            last_read_inbox: MessageId(0),
            last_read_outbox: MessageId(0),
            unread_mentions: 0,
            unread_reactions: 0,
            is_marked_as_unread: false,
            mute_for: 0,
            uses_default_mute: true,
            draft: None,
            action_bar: None,
            permissions: Permissions::default(),
            is_blocked: false,
            has_scheduled_messages: false,
        }
    }

    /// The chat's position in `list`, if it is in it.
    #[must_use]
    pub fn position(&self, list: ChatList) -> Option<&Position> {
        self.positions.iter().find(|position| position.list == list)
    }

    /// The other user of a private or secret chat.
    #[must_use]
    pub fn user(&self) -> Option<UserId> {
        match self.kind {
            ChatKind::Private { user } | ChatKind::Secret { user } => Some(user),
            ChatKind::Group { .. } | ChatKind::Supergroup { .. } => None,
        }
    }

    #[must_use]
    pub fn is_muted(&self) -> bool {
        self.mute_for > 0
    }

    /// Replaces the position in `position.list`; an order of 0 removes it.
    pub(crate) fn set_position(&mut self, position: Position) {
        self.positions.retain(|old| old.list != position.list);
        if position.order != 0 {
            self.positions.push(position);
        }
    }
}

impl From<td_api::types::Chat> for Chat {
    fn from(chat: td_api::types::Chat) -> Self {
        let notifications = chat.notification_settings;
        Self {
            id: ChatId(chat.id),
            kind: ChatKind::from(*chat.r#type),
            title: chat.title,
            photo: chat.photo.map(|photo| photo_of(*photo)),
            accent_color: chat.accent_color_id,
            last_message: chat.last_message.map(|message| Message::from(*message)),
            positions: positions(chat.positions),
            unread_count: chat.unread_count,
            last_read_inbox: MessageId(chat.last_read_inbox_message_id),
            last_read_outbox: MessageId(chat.last_read_outbox_message_id),
            unread_mentions: chat.unread_mention_count,
            unread_reactions: chat.unread_reaction_count,
            is_marked_as_unread: chat.is_marked_as_unread,
            mute_for: notifications.mute_for,
            uses_default_mute: notifications.use_default_mute_for,
            draft: chat.draft_message.and_then(|draft| draft_of(*draft)),
            action_bar: chat.action_bar.and_then(|bar| ActionBar::from_td(*bar)),
            permissions: Permissions::from(*chat.permissions),
            is_blocked: chat.block_list.is_some(),
            has_scheduled_messages: chat.has_scheduled_messages,
        }
    }
}

pub(crate) fn photo_of(photo: td_api::types::ChatPhotoInfo) -> Photo {
    Photo {
        small: crate::file::File::from(*photo.small),
        big: crate::file::File::from(*photo.big),
        minithumbnail: photo
            .minithumbnail
            .and_then(|thumbnail| Minithumbnail::from_td(*thumbnail)),
    }
}

/// Only text drafts preview in the list; voice and video note drafts don't.
pub(crate) fn draft_of(draft: td_api::types::DraftMessage) -> Option<Draft> {
    match *draft.content {
        DraftMessageContent::Text(text) => Some(Draft {
            date: draft.date,
            text: FormattedText::from(*text.text),
        }),
        _ => None,
    }
}

pub(crate) fn positions(positions: Vec<td_api::types::ChatPosition>) -> Vec<Position> {
    positions
        .into_iter()
        .map(Position::from)
        .filter(|position| position.order != 0)
        .collect()
}

impl From<ChatType> for ChatKind {
    fn from(kind: ChatType) -> Self {
        match kind {
            ChatType::Private(private) => Self::Private {
                user: UserId(private.user_id),
            },
            ChatType::BasicGroup(group) => Self::Group {
                group: BasicGroupId(group.basic_group_id),
            },
            ChatType::Supergroup(supergroup) => Self::Supergroup {
                supergroup: SupergroupId(supergroup.supergroup_id),
                is_channel: supergroup.is_channel,
            },
            ChatType::Secret(secret) => Self::Secret {
                user: UserId(secret.user_id),
            },
        }
    }
}

impl From<TdChatList> for ChatList {
    fn from(list: TdChatList) -> Self {
        match list {
            TdChatList::Main(_) => Self::Main,
            TdChatList::Archive(_) => Self::Archive,
            TdChatList::Folder(folder) => Self::Folder(FolderId(folder.chat_folder_id)),
        }
    }
}

impl ChatList {
    pub(crate) fn to_td(self) -> TdChatList {
        match self {
            Self::Main => TdChatList::Main(td_api::types::ChatListMain {}),
            Self::Archive => TdChatList::Archive(td_api::types::ChatListArchive {}),
            Self::Folder(FolderId(id)) => {
                TdChatList::Folder(td_api::types::ChatListFolder { chat_folder_id: id })
            }
        }
    }
}

impl From<td_api::types::ChatPosition> for Position {
    fn from(position: td_api::types::ChatPosition) -> Self {
        Self {
            list: ChatList::from(*position.list),
            order: position.order,
            is_pinned: position.is_pinned,
        }
    }
}

impl From<td_api::types::ChatFolderInfo> for Folder {
    fn from(folder: td_api::types::ChatFolderInfo) -> Self {
        Self {
            id: FolderId(folder.id),
            name: FormattedText::from(*folder.name.text),
            has_my_invite_links: folder.has_my_invite_links,
            icon: folder.icon.name,
        }
    }
}
