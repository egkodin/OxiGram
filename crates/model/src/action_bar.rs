//! The bar over a chat that offers to add, block or report the other side
//! (`chatActionBar`, tdesktop's `ContactStatus`), and what its buttons do.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::ChatActionBar;
use td_api::functions::{
    AddContact, GetChat, RemoveChatActionBar, ReportChat, SetChatNotificationSettings,
    SharePhoneNumber,
};
use td_api::types::ImportedContact;

use crate::chat::ChatList;
use crate::chat_actions::done;
use crate::ids::{ChatId, UserId};
use crate::session::{RequestError, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionBar {
    /// A group or channel to report; `can_unarchive` once TDLib archived it.
    ReportSpam {
        can_unarchive: bool,
    },
    /// Someone new in a private chat: add, block or report them.
    ReportAddBlock {
        can_unarchive: bool,
    },
    AddContact,
    SharePhoneNumber,
    /// An admin of a chat the user asked to join.
    JoinRequest {
        title: String,
        is_channel: bool,
        request_date: i32,
    },
}

impl ActionBar {
    /// None for the bars tdesktop's status bar does not show.
    pub(crate) fn from_td(bar: ChatActionBar) -> Option<Self> {
        Some(match bar {
            ChatActionBar::ReportSpam(bar) => Self::ReportSpam {
                can_unarchive: bar.can_unarchive,
            },
            // tdesktop has no bar for a new group's invite offer.
            ChatActionBar::InviteMembers(_) => return None,
            ChatActionBar::ReportAddBlock(bar) => Self::ReportAddBlock {
                can_unarchive: bar.can_unarchive,
            },
            ChatActionBar::AddContact(_) => Self::AddContact,
            ChatActionBar::SharePhoneNumber(_) => Self::SharePhoneNumber,
            ChatActionBar::JoinRequest(bar) => Self::JoinRequest {
                title: bar.title,
                is_channel: bar.is_channel,
                request_date: bar.request_date,
            },
        })
    }
}

impl Session {
    /// Hides the bar without doing anything else.
    pub fn remove_action_bar(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&RemoveChatActionBar { chat_id: chat.0 })
            .map(done)
    }

    /// Shows the user's phone number to a mutual contact.
    pub fn share_phone_number(
        &self,
        user: UserId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SharePhoneNumber { user_id: user.0 })
            .map(done)
    }

    /// Adds the other side of a private chat under the name they go by.
    pub fn add_contact(
        &self,
        user: UserId,
        first_name: String,
        last_name: String,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&AddContact {
                user_id: user.0,
                contact: Box::new(ImportedContact {
                    phone_number: String::new(),
                    first_name,
                    last_name,
                    note: None,
                }),
                share_phone_number: false,
            })
            .map(done)
    }

    /// Reports the chat as spam from its bar: no option, messages or text.
    pub fn report_spam(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&ReportChat {
                chat_id: chat.0,
                option_id: String::new(),
                message_ids: Vec::new(),
                text: String::new(),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    /// Brings an automatically archived chat back with default
    /// notifications, as TDLib asks for the bar's unarchive.
    pub fn unarchive(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let client = self.client().clone();
        let to_main = self.move_chat_to(chat, ChatList::Main);
        async move {
            let current = client
                .request(&GetChat { chat_id: chat.0 })
                .await
                .map_err(RequestError::from)?;
            let mut settings = *current.notification_settings;
            settings.use_default_mute_for = true;
            settings.use_default_sound = true;
            settings.use_default_show_preview = true;
            settings.use_default_mute_stories = true;
            settings.use_default_story_sound = true;
            settings.use_default_show_story_poster = true;
            settings.use_default_disable_pinned_message_notifications = true;
            settings.use_default_disable_mention_notifications = true;
            let reset = client.request(&SetChatNotificationSettings {
                chat_id: chat.0,
                notification_settings: Box::new(settings),
            });
            let (moved, reset) = futures::join!(to_main, reset);
            moved?;
            reset.map(drop).map_err(RequestError::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use td_api::types::{
        ChatActionBarInviteMembers, ChatActionBarJoinRequest, ChatActionBarReportAddBlock,
    };

    use super::*;

    #[test]
    fn bars_tdesktop_shows_keep_their_details() {
        let report = ChatActionBar::ReportAddBlock(ChatActionBarReportAddBlock {
            can_unarchive: true,
            account_info: None,
        });
        assert_eq!(
            ActionBar::from_td(report),
            Some(ActionBar::ReportAddBlock {
                can_unarchive: true
            })
        );
        let request = ChatActionBar::JoinRequest(ChatActionBarJoinRequest {
            title: "Hikers".to_owned(),
            is_channel: false,
            request_date: 5,
        });
        assert!(matches!(
            ActionBar::from_td(request),
            Some(ActionBar::JoinRequest {
                request_date: 5,
                ..
            })
        ));
        let invite = ChatActionBar::InviteMembers(ChatActionBarInviteMembers {});
        assert_eq!(ActionBar::from_td(invite), None);
    }
}
