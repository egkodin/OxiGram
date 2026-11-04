//! What keeps chats in the archive (`Api::GlobalPrivacy`'s archive part).

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{GetArchiveChatListSettings, SetArchiveChatListSettings};
use td_api::types as td;

use crate::{RequestError, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveSettings {
    pub archive_unknown: bool,
    /// Unmuted chats stay archived on new messages.
    pub keep_unmuted: bool,
    /// Unmuted chats in folders stay archived.
    pub keep_from_folders: bool,
}

impl Session {
    pub fn archive_settings(
        &self,
    ) -> impl Future<Output = Result<ArchiveSettings, RequestError>> + Send + use<> {
        self.client()
            .request(&GetArchiveChatListSettings {})
            .map(|result| {
                result
                    .map(|settings| ArchiveSettings {
                        archive_unknown: settings.archive_and_mute_new_chats_from_unknown_users,
                        keep_unmuted: settings.keep_unmuted_chats_archived,
                        keep_from_folders: settings.keep_chats_from_folders_archived,
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn set_archive_settings(
        &self,
        settings: ArchiveSettings,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetArchiveChatListSettings {
                settings: Box::new(td::ArchiveChatListSettings {
                    archive_and_mute_new_chats_from_unknown_users: settings.archive_unknown,
                    keep_unmuted_chats_archived: settings.keep_unmuted,
                    keep_chats_from_folders_archived: settings.keep_from_folders,
                }),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}
