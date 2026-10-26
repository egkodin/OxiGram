//! Users, as the chat list and profiles show them.

use td_api::enums::{UserStatus as TdUserStatus, UserType};

use crate::file::Minithumbnail;
use crate::ids::{CustomEmojiId, FileId, UserId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub first_name: String,
    pub last_name: String,
    pub status: UserStatus,
    pub photo: Option<Photo>,
    pub accent_color: i32,
    pub emoji_status: Option<CustomEmojiId>,
    pub badge: Badge,
    pub is_premium: bool,
    pub kind: UserKind,
    pub is_contact: bool,
}

/// A small userpic: the file to download and its blurred preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Photo {
    pub small: FileId,
    pub minithumbnail: Option<Minithumbnail>,
}

/// What a user or chat is marked as next to its name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Badge {
    #[default]
    None,
    Verified,
    Scam,
    Fake,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserKind {
    Regular,
    Bot,
    /// An account that was deleted; shown as "Deleted Account".
    Deleted,
    Unknown,
}

/// When the user was last seen, as far as their privacy settings tell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UserStatus {
    #[default]
    Empty,
    /// Online until this Unix time.
    Online {
        expires: i32,
    },
    Offline {
        was_online: i32,
    },
    Recently,
    LastWeek,
    LastMonth,
}

impl User {
    /// First and last name, as tdesktop's `UserData::name()` joins them.
    #[must_use]
    pub fn name(&self) -> String {
        match (self.first_name.is_empty(), self.last_name.is_empty()) {
            (_, true) => self.first_name.clone(),
            (true, false) => self.last_name.clone(),
            (false, false) => format!("{} {}", self.first_name, self.last_name),
        }
    }

    /// Whether the green dot shows on the userpic at Unix time `now`.
    #[must_use]
    pub fn is_online(&self, now: i32) -> bool {
        self.kind != UserKind::Bot
            && matches!(self.status, UserStatus::Online { expires } if expires > now)
    }
}

impl From<td_api::types::User> for User {
    fn from(user: td_api::types::User) -> Self {
        let badge = user.verification_status.map_or(Badge::None, |status| {
            Badge::of(status.is_verified, status.is_scam, status.is_fake)
        });
        let emoji_status = user.emoji_status.map(|status| match *status.r#type {
            td_api::enums::EmojiStatusType::CustomEmoji(emoji) => {
                CustomEmojiId(emoji.custom_emoji_id)
            }
            td_api::enums::EmojiStatusType::UpgradedGift(gift) => {
                CustomEmojiId(gift.model_custom_emoji_id)
            }
        });
        Self {
            id: UserId(user.id),
            first_name: user.first_name,
            last_name: user.last_name,
            status: UserStatus::from(*user.status),
            photo: user.profile_photo.map(|photo| Photo {
                small: FileId(photo.small.id),
                minithumbnail: photo.minithumbnail.and_then(|m| Minithumbnail::from_td(*m)),
            }),
            accent_color: user.accent_color_id,
            emoji_status,
            badge,
            is_premium: user.is_premium,
            kind: match *user.r#type {
                UserType::Regular(_) => UserKind::Regular,
                UserType::Bot(_) => UserKind::Bot,
                UserType::Deleted(_) => UserKind::Deleted,
                UserType::Unknown(_) => UserKind::Unknown,
            },
            is_contact: user.is_contact,
        }
    }
}

impl Badge {
    pub(crate) fn of(is_verified: bool, is_scam: bool, is_fake: bool) -> Self {
        if is_scam {
            Self::Scam
        } else if is_fake {
            Self::Fake
        } else if is_verified {
            Self::Verified
        } else {
            Self::None
        }
    }
}

impl From<TdUserStatus> for UserStatus {
    fn from(status: TdUserStatus) -> Self {
        match status {
            TdUserStatus::Empty(_) => Self::Empty,
            TdUserStatus::Online(online) => Self::Online {
                expires: online.expires,
            },
            TdUserStatus::Offline(offline) => Self::Offline {
                was_online: offline.was_online,
            },
            TdUserStatus::Recently(_) => Self::Recently,
            TdUserStatus::LastWeek(_) => Self::LastWeek,
            TdUserStatus::LastMonth(_) => Self::LastMonth,
        }
    }
}
