//! New groups and channels (`GroupInfoBox`, `SetupChannelBox`): creating
//! them, their photo, public link or invite link, and adding members.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{ChatType, InputChatPhoto, InputFile};
use td_api::functions::{
    AddChatMembers, CreateNewBasicGroupChat, CreateNewSupergroupChat, DeleteChat,
    GetSupergroupFullInfo, SetChatPhoto, SetSupergroupUsername,
};
use td_api::types as td;

use crate::{ChatId, RequestError, Session, SupergroupId, UserId};

/// A new channel: its chat and the supergroup behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedChannel {
    pub chat: ChatId,
    pub supergroup: SupergroupId,
}

fn ok<T>(result: Result<T, tg::TdError>) -> Result<(), RequestError> {
    result.map(drop).map_err(RequestError::from)
}

impl Session {
    /// A basic group with `members` (`createNewBasicGroupChat`).
    pub fn create_group(
        &self,
        title: String,
        members: &[UserId],
    ) -> impl Future<Output = Result<ChatId, RequestError>> + Send + use<> {
        self.client()
            .request(&CreateNewBasicGroupChat {
                user_ids: members.iter().map(|user| user.0).collect(),
                title,
                message_auto_delete_time: 0,
            })
            .map(|result| {
                result
                    .map(|created| ChatId(created.chat_id))
                    .map_err(RequestError::from)
            })
    }

    pub fn create_channel(
        &self,
        title: String,
        description: String,
    ) -> impl Future<Output = Result<CreatedChannel, RequestError>> + Send + use<> {
        self.client()
            .request(&CreateNewSupergroupChat {
                title,
                is_forum: false,
                is_channel: true,
                description,
                location: None,
                message_auto_delete_time: 0,
                for_import: false,
            })
            .map(|result| {
                let chat = result.map_err(RequestError::from)?;
                let supergroup = match *chat.r#type {
                    ChatType::Supergroup(supergroup) => SupergroupId(supergroup.supergroup_id),
                    _ => SupergroupId(0),
                };
                Ok(CreatedChannel {
                    chat: ChatId(chat.id),
                    supergroup,
                })
            })
    }

    /// The chat's photo from a local image.
    pub fn set_chat_photo(
        &self,
        chat: ChatId,
        path: &str,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetChatPhoto {
                chat_id: chat.0,
                photo: Some(Box::new(InputChatPhoto::Static(td::InputChatPhotoStatic {
                    photo: Box::new(InputFile::Local(td::InputFileLocal {
                        path: path.to_owned(),
                    })),
                }))),
            })
            .map(ok)
    }

    /// Makes the channel public at t.me/`username`, or private with "".
    pub fn set_channel_username(
        &self,
        supergroup: SupergroupId,
        username: String,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetSupergroupUsername {
                supergroup_id: supergroup.0,
                username,
            })
            .map(ok)
    }

    /// The channel's primary invite link.
    pub fn invite_link(
        &self,
        supergroup: SupergroupId,
    ) -> impl Future<Output = Result<String, RequestError>> + Send + use<> {
        self.client()
            .request(&GetSupergroupFullInfo {
                supergroup_id: supergroup.0,
            })
            .map(|result| {
                result
                    .map(|info| {
                        info.invite_link
                            .map(|link| link.invite_link)
                            .unwrap_or_default()
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn add_members(
        &self,
        chat: ChatId,
        members: &[UserId],
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&AddChatMembers {
                chat_id: chat.0,
                user_ids: members.iter().map(|user| user.0).collect(),
            })
            .map(ok)
    }

    /// Deletes an owned group or channel for everyone (`deleteChat`).
    pub fn delete_chat(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DeleteChat { chat_id: chat.0 })
            .map(ok)
    }
}
