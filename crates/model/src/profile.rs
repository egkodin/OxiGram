//! What a profile shows beyond the chat list's data (`Info::Profile`): the
//! full info, the shared media counts and lists, members, groups in common
//! and the photo history; and the own profile's edits.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{BlockList, MessageSender, SearchMessagesFilter, SupergroupMembersFilter};
use td_api::functions::{
    GetBasicGroupFullInfo, GetChatMessageCount, GetGroupsInCommon, GetSupergroupFullInfo,
    GetSupergroupMembers, GetUserFullInfo, GetUserProfilePhotos, RemoveContacts, ReportChat,
    SearchChatMessages, SendMessage, SetBio, SetBirthdate, SetMessageSenderBlockList, SetName,
    SetUsername,
};
use td_api::types as td;

use crate::file::File;
use crate::message::Message;
use crate::rights::MemberStatus;
use crate::text::FormattedText;
use crate::{BasicGroupId, ChatId, RequestError, Session, SupergroupId, UserId};

/// How many shared media, members or groups one request brings.
// shortcut: one page of each list; page with offsets when a profile needs more.
const PAGE: i32 = 100;

/// A user's full info (`userFullInfo`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserProfile {
    pub bio: FormattedText,
    pub birthday: Option<Birthday>,
    pub common_groups: i32,
    pub is_blocked: bool,
}

/// A birthday; the year is hidden by some users.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Birthday {
    pub day: i32,
    pub month: i32,
    pub year: Option<i32>,
}

/// A group's or channel's full info.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupProfile {
    pub description: String,
    /// The primary invite link, for those who may see it.
    pub invite_link: String,
    pub member_count: i32,
    /// Basic groups bring their members; supergroups load them apart.
    pub members: Vec<Member>,
    pub can_get_members: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    pub user: UserId,
    pub status: MemberStatus,
}

/// The shared media lists (`Storage::SharedMediaType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SharedMedia {
    Photo,
    Video,
    File,
    Music,
    Link,
    Voice,
    Gif,
}

impl SharedMedia {
    /// In tdesktop's order (`SetupSharedMediaClassic`).
    pub const ALL: [Self; 7] = [
        Self::Photo,
        Self::Video,
        Self::File,
        Self::Music,
        Self::Link,
        Self::Voice,
        Self::Gif,
    ];

    fn filter(self) -> SearchMessagesFilter {
        match self {
            Self::Photo => SearchMessagesFilter::Photo(td::SearchMessagesFilterPhoto {}),
            Self::Video => SearchMessagesFilter::Video(td::SearchMessagesFilterVideo {}),
            Self::File => SearchMessagesFilter::Document(td::SearchMessagesFilterDocument {}),
            Self::Music => SearchMessagesFilter::Audio(td::SearchMessagesFilterAudio {}),
            Self::Link => SearchMessagesFilter::Url(td::SearchMessagesFilterUrl {}),
            Self::Voice => SearchMessagesFilter::VoiceAndVideoNote(
                td::SearchMessagesFilterVoiceAndVideoNote {},
            ),
            Self::Gif => SearchMessagesFilter::Animation(td::SearchMessagesFilterAnimation {}),
        }
    }
}

/// Whether the user's messages and calls reach the account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocking {
    Blocked,
    Unblocked,
}

/// Where a report stands (`reportChatResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Sent,
    /// The reasons to pick from, as (id, text), under `title`.
    Options {
        title: String,
        options: Vec<(String, String)>,
    },
    /// A comment for the picked reason.
    Text {
        option: String,
        is_optional: bool,
    },
    /// TDLib wants messages picked, which a profile cannot do.
    Messages,
}

fn birthday(date: td::Birthdate) -> Birthday {
    Birthday {
        day: date.day,
        month: date.month,
        year: (date.year > 0).then_some(date.year),
    }
}

fn member(member: td::ChatMember) -> Option<Member> {
    match *member.member_id {
        MessageSender::User(user) => Some(Member {
            user: UserId(user.user_id),
            status: MemberStatus::from(*member.status),
        }),
        MessageSender::Chat(_) => None,
    }
}

fn invite_link(link: Option<Box<td::ChatInviteLink>>) -> String {
    link.map(|link| link.invite_link).unwrap_or_default()
}

fn ok<T>(result: Result<T, tg::TdError>) -> Result<(), RequestError> {
    result.map(drop).map_err(RequestError::from)
}

impl Session {
    pub fn user_profile(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<UserProfile, RequestError>> + Send + use<> {
        self.client()
            .request(&GetUserFullInfo { user_id: user.0 })
            .map(|result| {
                result
                    .map(|info| UserProfile {
                        bio: info
                            .bio
                            .map(|bio| FormattedText::from(*bio))
                            .unwrap_or_default(),
                        birthday: info.birthdate.map(|date| birthday(*date)),
                        common_groups: info.group_in_common_count,
                        is_blocked: matches!(info.block_list.as_deref(), Some(BlockList::Main(_))),
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn group_profile(
        &self,
        group: BasicGroupId,
    ) -> impl Future<Output = Result<GroupProfile, RequestError>> + Send + use<> {
        self.client()
            .request(&GetBasicGroupFullInfo {
                basic_group_id: group.0,
            })
            .map(|result| {
                result
                    .map(|info| {
                        let members: Vec<Member> =
                            info.members.into_iter().filter_map(member).collect();
                        GroupProfile {
                            description: info.description,
                            invite_link: invite_link(info.invite_link),
                            member_count: i32::try_from(members.len()).unwrap_or(i32::MAX),
                            members,
                            can_get_members: true,
                        }
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn supergroup_profile(
        &self,
        supergroup: SupergroupId,
    ) -> impl Future<Output = Result<GroupProfile, RequestError>> + Send + use<> {
        self.client()
            .request(&GetSupergroupFullInfo {
                supergroup_id: supergroup.0,
            })
            .map(|result| {
                result
                    .map(|info| GroupProfile {
                        description: info.description,
                        invite_link: invite_link(info.invite_link),
                        member_count: info.member_count,
                        members: Vec::new(),
                        can_get_members: info.can_get_members,
                    })
                    .map_err(RequestError::from)
            })
    }

    /// The recent members of a supergroup (`ParticipantsBoxController`).
    pub fn supergroup_members(
        &self,
        supergroup: SupergroupId,
    ) -> impl Future<Output = Result<Vec<Member>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetSupergroupMembers {
                supergroup_id: supergroup.0,
                filter: Some(Box::new(SupergroupMembersFilter::Recent(
                    td::SupergroupMembersFilterRecent {},
                ))),
                offset: 0,
                limit: PAGE,
            })
            .map(|result| {
                result
                    .map(|page| page.members.into_iter().filter_map(member).collect())
                    .map_err(RequestError::from)
            })
    }

    /// How many messages of `chat` the list holds, from the server.
    pub fn shared_media_count(
        &self,
        chat: ChatId,
        media: SharedMedia,
    ) -> impl Future<Output = Result<i32, RequestError>> + Send + use<> {
        self.client()
            .request(&GetChatMessageCount {
                chat_id: chat.0,
                topic_id: None,
                filter: Box::new(media.filter()),
                return_local: false,
            })
            .map(|result| result.map(|count| count.count).map_err(RequestError::from))
    }

    /// The newest messages of the list, newest first.
    pub fn shared_media(
        &self,
        chat: ChatId,
        media: SharedMedia,
    ) -> impl Future<Output = Result<Vec<Message>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchChatMessages {
                chat_id: chat.0,
                topic_id: None,
                query: String::new(),
                sender_id: None,
                from_message_id: 0,
                offset: 0,
                limit: PAGE,
                filter: Some(Box::new(media.filter())),
            })
            .map(|result| {
                result
                    .map(|found| found.messages.into_iter().map(Message::from).collect())
                    .map_err(RequestError::from)
            })
    }

    pub fn groups_in_common(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetGroupsInCommon {
                user_id: user.0,
                offset_chat_id: 0,
                limit: PAGE,
            })
            .map(|result| {
                result
                    .map(|chats| chats.chat_ids.into_iter().map(ChatId).collect())
                    .map_err(RequestError::from)
            })
    }

    /// The user's photos, newest first, each at its largest size.
    pub fn profile_photos(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<File>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetUserProfilePhotos {
                user_id: user.0,
                offset: 0,
                limit: PAGE,
            })
            .map(|result| {
                result
                    .map(|photos| {
                        photos
                            .photos
                            .into_iter()
                            .filter_map(|photo| photo.sizes.into_iter().last())
                            .map(|size| File::from(*size.photo))
                            .collect()
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn set_blocked(
        &self,
        user: UserId,
        blocking: Blocking,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let block_list = match blocking {
            Blocking::Blocked => Some(Box::new(BlockList::Main(td::BlockListMain {}))),
            Blocking::Unblocked => None,
        };
        self.client()
            .request(&SetMessageSenderBlockList {
                sender_id: Box::new(MessageSender::User(td::MessageSenderUser {
                    user_id: user.0,
                })),
                block_list,
            })
            .map(ok)
    }

    /// Reports the chat for `option` (empty first, to get the reasons) with
    /// an optional comment.
    pub fn report_chat(
        &self,
        chat: ChatId,
        option: String,
        text: String,
    ) -> impl Future<Output = Result<Report, RequestError>> + Send + use<> {
        use td_api::enums::ReportChatResult;
        self.client()
            .request(&ReportChat {
                chat_id: chat.0,
                option_id: option,
                message_ids: Vec::new(),
                text,
            })
            .map(|result| {
                result
                    .map(|result| match result {
                        ReportChatResult::Ok(_) => Report::Sent,
                        ReportChatResult::OptionRequired(required) => Report::Options {
                            title: required.title,
                            options: required
                                .options
                                .into_iter()
                                .map(|option| (option.id, option.text))
                                .collect(),
                        },
                        ReportChatResult::TextRequired(required) => Report::Text {
                            option: required.option_id,
                            is_optional: required.is_optional,
                        },
                        ReportChatResult::MessagesRequired(_) => Report::Messages,
                    })
                    .map_err(RequestError::from)
            })
    }

    /// Sends `user`'s contact card to `chat` (`SendContact`).
    pub fn send_contact(
        &self,
        chat: ChatId,
        user: &crate::user::User,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let contact = td::Contact {
            phone_number: user.phone.expose().to_owned(),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            vcard: String::new(),
            user_id: user.id.0,
        };
        self.client()
            .request(&SendMessage {
                chat_id: chat.0,
                topic_id: None,
                reply_to: None,
                options: crate::send::SendOptions::default().to_td(),
                reply_markup: None,
                input_message_content: Box::new(
                    td_api::enums::InputMessageContent::InputMessageContact(
                        td::InputMessageContact {
                            contact: Box::new(contact),
                        },
                    ),
                ),
            })
            .map(ok)
    }

    pub fn remove_contact(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&RemoveContacts {
                user_ids: vec![user.0],
            })
            .map(ok)
    }

    pub fn set_name(
        &self,
        first_name: String,
        last_name: String,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetName {
                first_name,
                last_name,
            })
            .map(ok)
    }

    pub fn set_bio(
        &self,
        bio: String,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client().request(&SetBio { bio }).map(ok)
    }

    pub fn set_username(
        &self,
        username: String,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client().request(&SetUsername { username }).map(ok)
    }

    /// None removes the birthday.
    pub fn set_birthday(
        &self,
        birthday: Option<Birthday>,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let birthdate = birthday.map(|birthday| {
            Box::new(td::Birthdate {
                day: birthday.day,
                month: birthday.month,
                year: birthday.year.unwrap_or(0),
            })
        });
        self.client().request(&SetBirthdate { birthdate }).map(ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_birthday_without_a_year_keeps_none() {
        let date = |year| td::Birthdate {
            day: 3,
            month: 11,
            year,
        };
        assert_eq!(birthday(date(0)).year, None);
        assert_eq!(birthday(date(1990)).year, Some(1990));
    }
}
