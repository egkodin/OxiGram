//! What TDLib keeps on disk (`Settings::LocalStorage`): the cache by kind
//! and clearing it. Only local copies go; Telegram keeps the files.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::FileType;
use td_api::functions::{GetStorageStatistics, OptimizeStorage};
use td_api::types as td;

use crate::session::{RequestError, Session};

/// tdesktop's groups of cached files, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    Images,
    Stickers,
    Voice,
    Round,
    Animations,
    /// Videos, music and files.
    Media,
}

impl StorageKind {
    pub const ALL: [Self; 6] = [
        Self::Images,
        Self::Stickers,
        Self::Voice,
        Self::Round,
        Self::Animations,
        Self::Media,
    ];

    /// None for files clearing must never touch: Passport documents and
    /// notification sounds.
    fn of(file_type: &FileType) -> Option<Self> {
        Some(match file_type {
            FileType::Photo(_)
            | FileType::ProfilePhoto(_)
            | FileType::Thumbnail(_)
            | FileType::Wallpaper(_)
            | FileType::PhotoStory(_)
            | FileType::SelfDestructingPhoto(_)
            | FileType::SecretThumbnail(_) => Self::Images,
            FileType::Sticker(_) => Self::Stickers,
            FileType::VoiceNote(_) | FileType::SelfDestructingVoiceNote(_) => Self::Voice,
            FileType::VideoNote(_) | FileType::SelfDestructingVideoNote(_) => Self::Round,
            FileType::Animation(_) => Self::Animations,
            FileType::Secure(_) | FileType::NotificationSound(_) => return None,
            _ => Self::Media,
        })
    }

    /// TDLib's file types in the group, for `optimizeStorage`.
    fn file_types(self) -> Vec<FileType> {
        match self {
            Self::Images => vec![
                FileType::Photo(td::FileTypePhoto {}),
                FileType::ProfilePhoto(td::FileTypeProfilePhoto {}),
                FileType::Thumbnail(td::FileTypeThumbnail {}),
                FileType::Wallpaper(td::FileTypeWallpaper {}),
                FileType::PhotoStory(td::FileTypePhotoStory {}),
                FileType::SelfDestructingPhoto(td::FileTypeSelfDestructingPhoto {}),
                FileType::SecretThumbnail(td::FileTypeSecretThumbnail {}),
            ],
            Self::Stickers => vec![FileType::Sticker(td::FileTypeSticker {})],
            Self::Voice => vec![
                FileType::VoiceNote(td::FileTypeVoiceNote {}),
                FileType::SelfDestructingVoiceNote(td::FileTypeSelfDestructingVoiceNote {}),
            ],
            Self::Round => vec![
                FileType::VideoNote(td::FileTypeVideoNote {}),
                FileType::SelfDestructingVideoNote(td::FileTypeSelfDestructingVideoNote {}),
            ],
            Self::Animations => vec![FileType::Animation(td::FileTypeAnimation {})],
            Self::Media => vec![
                FileType::Video(td::FileTypeVideo {}),
                FileType::VideoStory(td::FileTypeVideoStory {}),
                FileType::Audio(td::FileTypeAudio {}),
                FileType::Document(td::FileTypeDocument {}),
                FileType::Unknown(td::FileTypeUnknown {}),
                FileType::Secret(td::FileTypeSecret {}),
                FileType::SelfDestructingVideo(td::FileTypeSelfDestructingVideo {}),
                FileType::LivePhotoVideo(td::FileTypeLivePhotoVideo {}),
                FileType::SelfDestructingLivePhotoVideo(
                    td::FileTypeSelfDestructingLivePhotoVideo {},
                ),
            ],
        }
    }
}

/// The cache's size and file count for one kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub size: i64,
    pub count: i64,
}

/// The cache by kind, in [`StorageKind::ALL`]'s order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageUsage {
    pub by_kind: [Usage; 6],
}

impl StorageUsage {
    #[must_use]
    pub fn usage(&self, kind: StorageKind) -> Usage {
        let index = StorageKind::ALL
            .iter()
            .position(|known| *known == kind)
            .unwrap_or_default();
        self.by_kind[index]
    }

    #[must_use]
    pub fn total(&self) -> i64 {
        self.by_kind.iter().map(|usage| usage.size).sum()
    }

    fn from_td(stats: td::StorageStatistics) -> Self {
        let mut usage = Self::default();
        let entries = stats.by_chat.into_iter().flat_map(|chat| chat.by_file_type);
        for entry in entries {
            let Some(kind) = StorageKind::of(&entry.file_type) else {
                continue;
            };
            let index = StorageKind::ALL
                .iter()
                .position(|known| *known == kind)
                .unwrap_or_default();
            usage.by_kind[index].size += entry.size;
            usage.by_kind[index].count += i64::from(entry.count);
        }
        usage
    }
}

impl Session {
    pub fn storage_usage(
        &self,
    ) -> impl Future<Output = Result<StorageUsage, RequestError>> + Send + use<> {
        self.client()
            .request(&GetStorageStatistics { chat_limit: 0 })
            .map(|result| {
                result
                    .map(StorageUsage::from_td)
                    .map_err(RequestError::from)
            })
    }

    /// Deletes the local copies of `kinds`, then gives the cache that is
    /// left.
    pub fn clear_storage(
        &self,
        kinds: &[StorageKind],
    ) -> impl Future<Output = Result<StorageUsage, RequestError>> + Send + use<> {
        let file_types = kinds.iter().flat_map(|kind| kind.file_types()).collect();
        let client = self.client().clone();
        let request = client.request(&OptimizeStorage {
            size: 0,
            ttl: 0,
            count: 0,
            immunity_delay: 0,
            file_types,
            chat_ids: Vec::new(),
            exclude_chat_ids: Vec::new(),
            return_deleted_file_statistics: false,
            chat_limit: 0,
        });
        async move {
            request.await.map_err(RequestError::from)?;
            client
                .request(&GetStorageStatistics { chat_limit: 0 })
                .await
                .map(StorageUsage::from_td)
                .map_err(RequestError::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cleared_type_falls_back_into_its_own_kind() {
        for kind in StorageKind::ALL {
            for file_type in kind.file_types() {
                assert_eq!(StorageKind::of(&file_type), Some(kind), "{file_type:?}");
            }
        }
        assert_eq!(
            StorageKind::of(&FileType::Secure(td::FileTypeSecure {})),
            None
        );
    }
}
