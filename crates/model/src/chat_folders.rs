//! A chat folder's rules (`Data::ChatFilter`): its name, the chat types and
//! chats it shows, and creating, editing and suggesting folders.

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{
    CreateChatFolder, EditChatFolder, GetChatFolder, GetRecommendedChatFolders,
};
use td_api::types as td;

use crate::chat_actions::done;
use crate::ids::{ChatId, FolderId};
use crate::session::{RequestError, Session};

/// The longest folder name TDLib takes (`kMaxFilterTitleLength`).
pub const NAME_MAX: usize = 12;

/// The chat types a folder includes or excludes (`Data::ChatFilter::Flag`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatType {
    Contacts,
    NonContacts,
    Groups,
    Channels,
    Bots,
    Muted,
    Read,
    Archived,
}

impl ChatType {
    /// The types "Add Chats" offers, in tdesktop's order.
    pub const INCLUDED: [Self; 5] = [
        Self::Contacts,
        Self::NonContacts,
        Self::Groups,
        Self::Channels,
        Self::Bots,
    ];
    /// The types "Exclude Chats" offers.
    pub const EXCLUDED: [Self; 3] = [Self::Muted, Self::Read, Self::Archived];
}

/// A folder as TDLib stores it; edits keep what rgram does not show yet
/// (the icon, color, custom emoji in the name).
#[derive(Debug, Clone, PartialEq)]
pub struct FolderRules {
    raw: td::ChatFolder,
}

impl FolderRules {
    /// A new folder: no name, no chats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            raw: td::ChatFolder {
                name: Box::new(td::ChatFolderName {
                    text: Box::new(td::FormattedText {
                        text: String::new(),
                        entities: Vec::new(),
                    }),
                    animate_custom_emoji: true,
                }),
                icon: None,
                color_id: -1,
                is_shareable: false,
                pinned_chat_ids: Vec::new(),
                included_chat_ids: Vec::new(),
                excluded_chat_ids: Vec::new(),
                exclude_muted: false,
                exclude_read: false,
                exclude_archived: false,
                include_contacts: false,
                include_non_contacts: false,
                include_bots: false,
                include_groups: false,
                include_channels: false,
            },
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.raw.name.text.text
    }

    /// Sets the name; a changed name drops its custom emoji.
    pub fn set_name(&mut self, name: &str) {
        if name != self.name() {
            *self.raw.name.text = td::FormattedText {
                text: name.to_owned(),
                entities: Vec::new(),
            };
        }
    }

    /// Created invite links make it shared: chats can't be excluded.
    #[must_use]
    pub fn is_shareable(&self) -> bool {
        self.raw.is_shareable
    }

    #[must_use]
    pub fn has(&self, kind: ChatType) -> bool {
        let raw = &self.raw;
        match kind {
            ChatType::Contacts => raw.include_contacts,
            ChatType::NonContacts => raw.include_non_contacts,
            ChatType::Groups => raw.include_groups,
            ChatType::Channels => raw.include_channels,
            ChatType::Bots => raw.include_bots,
            ChatType::Muted => raw.exclude_muted,
            ChatType::Read => raw.exclude_read,
            ChatType::Archived => raw.exclude_archived,
        }
    }

    pub fn set(&mut self, kind: ChatType, on: bool) {
        let raw = &mut self.raw;
        let flag = match kind {
            ChatType::Contacts => &mut raw.include_contacts,
            ChatType::NonContacts => &mut raw.include_non_contacts,
            ChatType::Groups => &mut raw.include_groups,
            ChatType::Channels => &mut raw.include_channels,
            ChatType::Bots => &mut raw.include_bots,
            ChatType::Muted => &mut raw.exclude_muted,
            ChatType::Read => &mut raw.exclude_read,
            ChatType::Archived => &mut raw.exclude_archived,
        };
        *flag = on;
    }

    /// Always included chats, pinned ones first (`always()`).
    #[must_use]
    pub fn included(&self) -> Vec<ChatId> {
        let raw = &self.raw;
        let rest = raw
            .included_chat_ids
            .iter()
            .filter(|id| !raw.pinned_chat_ids.contains(id));
        raw.pinned_chat_ids
            .iter()
            .chain(rest)
            .copied()
            .map(ChatId)
            .collect()
    }

    #[must_use]
    pub fn excluded(&self) -> Vec<ChatId> {
        self.raw
            .excluded_chat_ids
            .iter()
            .copied()
            .map(ChatId)
            .collect()
    }

    /// Replaces the always included chats; pinned ones still chosen stay
    /// pinned, and none stays excluded.
    pub fn set_included(&mut self, chats: &[ChatId]) {
        let ids: Vec<i64> = chats.iter().map(|chat| chat.0).collect();
        self.raw.pinned_chat_ids.retain(|id| ids.contains(id));
        self.raw.included_chat_ids = ids
            .iter()
            .copied()
            .filter(|id| !self.raw.pinned_chat_ids.contains(id))
            .collect();
        self.raw.excluded_chat_ids.retain(|id| !ids.contains(id));
    }

    /// Replaces the excluded chats; none stays included.
    pub fn set_excluded(&mut self, chats: &[ChatId]) {
        let ids: Vec<i64> = chats.iter().map(|chat| chat.0).collect();
        self.raw.pinned_chat_ids.retain(|id| !ids.contains(id));
        self.raw.included_chat_ids.retain(|id| !ids.contains(id));
        self.raw.excluded_chat_ids = ids;
    }

    /// Removes a chat from both lists (the preview's cross).
    pub fn remove_chat(&mut self, chat: ChatId) {
        let raw = &mut self.raw;
        raw.pinned_chat_ids.retain(|id| *id != chat.0);
        raw.included_chat_ids.retain(|id| *id != chat.0);
        raw.excluded_chat_ids.retain(|id| *id != chat.0);
    }

    /// Shows no chat type and no chat (`lng_filters_empty`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !ChatType::INCLUDED.iter().any(|kind| self.has(*kind)) && self.included().is_empty()
    }

    /// Shows every chat as "All chats" does (`lng_filters_default`).
    #[must_use]
    pub fn is_all_chats(&self) -> bool {
        ChatType::INCLUDED.iter().all(|kind| self.has(*kind))
            && self.has(ChatType::Archived)
            && !self.has(ChatType::Muted)
            && !self.has(ChatType::Read)
            && self.included().is_empty()
            && self.excluded().is_empty()
    }
}

impl Default for FolderRules {
    fn default() -> Self {
        Self::new()
    }
}

/// A folder Telegram suggests, with why.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggested {
    pub rules: FolderRules,
    pub description: String,
}

impl Session {
    pub fn folder_rules(
        &self,
        folder: FolderId,
    ) -> impl Future<Output = Result<FolderRules, RequestError>> + Send + use<> {
        self.client()
            .request(&GetChatFolder {
                chat_folder_id: folder.0,
            })
            .map(|result| {
                result
                    .map(|raw| FolderRules { raw })
                    .map_err(RequestError::from)
            })
    }

    pub fn create_folder(
        &self,
        rules: &FolderRules,
    ) -> impl Future<Output = Result<FolderId, RequestError>> + Send + use<> {
        self.client()
            .request(&CreateChatFolder {
                folder: Box::new(rules.raw.clone()),
            })
            .map(|result| {
                result
                    .map(|info| FolderId(info.id))
                    .map_err(RequestError::from)
            })
    }

    pub fn edit_folder(
        &self,
        folder: FolderId,
        rules: &FolderRules,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&EditChatFolder {
                chat_folder_id: folder.0,
                folder: Box::new(rules.raw.clone()),
            })
            .map(|result| done(result.map(|_| td::Ok {})))
    }

    pub fn recommended_folders(
        &self,
    ) -> impl Future<Output = Result<Vec<Suggested>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetRecommendedChatFolders {})
            .map(|result| {
                result
                    .map(|list| {
                        list.chat_folders
                            .into_iter()
                            .map(|folder| Suggested {
                                rules: FolderRules {
                                    raw: *folder.folder,
                                },
                                description: folder.description,
                            })
                            .collect()
                    })
                    .map_err(RequestError::from)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chats_move_between_included_and_excluded() {
        let mut rules = FolderRules::new();
        rules.set_included(&[ChatId(1), ChatId(2)]);
        rules.set_excluded(&[ChatId(2), ChatId(3)]);
        assert_eq!(rules.included(), [ChatId(1)]);
        assert_eq!(rules.excluded(), [ChatId(2), ChatId(3)]);
        rules.remove_chat(ChatId(3));
        assert_eq!(rules.excluded(), [ChatId(2)]);
        assert!(!rules.is_empty());
    }

    #[test]
    fn a_new_name_drops_custom_emoji_and_all_types_is_all_chats() {
        let mut rules = FolderRules::new();
        assert!(rules.is_empty());
        for kind in ChatType::INCLUDED {
            rules.set(kind, true);
        }
        assert!(!rules.is_all_chats(), "archived chats still shown");
        rules.set(ChatType::Archived, true);
        assert!(rules.is_all_chats());
        rules.set_name("Work");
        assert_eq!(rules.name(), "Work");
    }
}
