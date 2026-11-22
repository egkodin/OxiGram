//! What the user may do at the bottom of a chat: write, or one of the bars
//! that take the field's place (`HistoryWidget::updateControlsVisibility`).

use td_api::enums::ChatMemberStatus;

use crate::chat::ChatKind;
use crate::chats::Chats;
use crate::ids::ChatId;
use crate::user::UserKind;

/// The user's standing in a group or channel (`chatMemberStatus`), the part
/// that decides writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberStatus {
    Creator,
    Administrator {
        can_post_messages: bool,
    },
    Member,
    Restricted {
        is_member: bool,
        /// Unix time the restriction ends; 0 for never.
        until: i32,
        can_send_basic_messages: bool,
    },
    Left,
    Banned,
}

impl MemberStatus {
    /// Whether the user is in the chat (`ChannelData::amIn`).
    #[must_use]
    pub fn is_member(self) -> bool {
        match self {
            Self::Creator | Self::Administrator { .. } | Self::Member => true,
            Self::Restricted { is_member, .. } => is_member,
            Self::Left | Self::Banned => false,
        }
    }
}

impl From<ChatMemberStatus> for MemberStatus {
    fn from(status: ChatMemberStatus) -> Self {
        match status {
            ChatMemberStatus::Creator(_) => Self::Creator,
            ChatMemberStatus::Administrator(admin) => Self::Administrator {
                can_post_messages: admin.rights.can_post_messages,
            },
            ChatMemberStatus::Member(_) => Self::Member,
            ChatMemberStatus::Restricted(restricted) => Self::Restricted {
                is_member: restricted.is_member,
                until: restricted.restricted_until_date,
                can_send_basic_messages: restricted.permissions.can_send_basic_messages,
            },
            ChatMemberStatus::Left(_) => Self::Left,
            ChatMemberStatus::Banned(_) => Self::Banned,
        }
    }
}

/// What every member of a group may send (`chatPermissions`), the part
/// oxigram uses so far.
// shortcut: text only; the media permissions come with attachments (M4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub can_send_basic_messages: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            can_send_basic_messages: true,
        }
    }
}

impl From<td_api::types::ChatPermissions> for Permissions {
    fn from(permissions: td_api::types::ChatPermissions) -> Self {
        Self {
            can_send_basic_messages: permissions.can_send_basic_messages,
        }
    }
}

/// What goes under the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writing {
    Field,
    /// A blocked user: "Unblock".
    Unblock,
    /// A blocked bot: "Restart bot".
    RestartBot,
    /// Not in the group or channel: "Join channel" or "Join group".
    Join {
        is_channel: bool,
    },
    /// A channel the user reads but cannot post in: "Mute" or "Unmute".
    Mute {
        is_muted: bool,
    },
    /// A bot not talked to yet: "Start".
    StartBot,
    /// A group that forbids the user to write, with the reason.
    Restricted(Restriction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restriction {
    /// The admins restricted this user until a Unix time.
    Until(i32),
    /// The admins restricted this user.
    ByAdmins,
    /// No member may write.
    ForAll,
}

impl Chats {
    /// What the user can do at the bottom of `chat`, in tdesktop's order:
    /// blocked, then joining, then muting, then starting a bot, then the
    /// field (or why it is disabled).
    #[must_use]
    pub fn writing(&self, chat: ChatId) -> Writing {
        let Some(chat) = self.chat(chat) else {
            return Writing::Field;
        };
        match chat.kind {
            ChatKind::Private { user } | ChatKind::Secret { user } => {
                let is_bot = self
                    .user(user)
                    .is_some_and(|user| user.kind == UserKind::Bot);
                match (chat.is_blocked, is_bot) {
                    (true, true) => Writing::RestartBot,
                    (true, false) => Writing::Unblock,
                    (false, true) if chat.last_message.is_none() => Writing::StartBot,
                    (false, _) => Writing::Field,
                }
            }
            ChatKind::Supergroup {
                supergroup,
                is_channel,
            } => {
                let Some(status) = self.supergroup_status(supergroup) else {
                    return Writing::Field;
                };
                if !status.is_member() {
                    Writing::Join { is_channel }
                } else if is_channel && !can_post(status) {
                    Writing::Mute {
                        is_muted: self.is_muted(chat),
                    }
                } else {
                    group_writing(status, chat.permissions)
                }
            }
            ChatKind::Group { group } => {
                self.group_status(group).map_or(Writing::Field, |status| {
                    group_writing(status, chat.permissions)
                })
            }
        }
    }
}

fn can_post(status: MemberStatus) -> bool {
    matches!(
        status,
        MemberStatus::Creator
            | MemberStatus::Administrator {
                can_post_messages: true
            }
    )
}

fn group_writing(status: MemberStatus, permissions: Permissions) -> Writing {
    match status {
        MemberStatus::Creator | MemberStatus::Administrator { .. } => Writing::Field,
        MemberStatus::Restricted {
            can_send_basic_messages: false,
            until,
            ..
        } => Writing::Restricted(if until > 0 {
            Restriction::Until(until)
        } else {
            Restriction::ByAdmins
        }),
        _ if !permissions.can_send_basic_messages => Writing::Restricted(Restriction::ForAll),
        _ => Writing::Field,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admins_write_whatever_members_may() {
        let closed = Permissions {
            can_send_basic_messages: false,
        };
        let admin = MemberStatus::Administrator {
            can_post_messages: false,
        };
        assert_eq!(group_writing(admin, closed), Writing::Field);
        assert_eq!(
            group_writing(MemberStatus::Member, closed),
            Writing::Restricted(Restriction::ForAll)
        );
        assert_eq!(
            group_writing(MemberStatus::Member, Permissions::default()),
            Writing::Field
        );
    }

    #[test]
    fn a_restricted_member_hears_until_when() {
        let restricted = |until| MemberStatus::Restricted {
            is_member: true,
            until,
            can_send_basic_messages: false,
        };
        assert_eq!(
            group_writing(restricted(100), Permissions::default()),
            Writing::Restricted(Restriction::Until(100))
        );
        assert_eq!(
            group_writing(restricted(0), Permissions::default()),
            Writing::Restricted(Restriction::ByAdmins)
        );
    }

    #[test]
    fn only_posting_admins_post_in_channels() {
        assert!(can_post(MemberStatus::Creator));
        assert!(can_post(MemberStatus::Administrator {
            can_post_messages: true
        }));
        assert!(!can_post(MemberStatus::Member));
        assert!(!MemberStatus::Left.is_member());
        assert!(
            MemberStatus::Restricted {
                is_member: true,
                until: 0,
                can_send_basic_messages: false
            }
            .is_member()
        );
    }
}
