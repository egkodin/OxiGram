//! Telegram's notification defaults per kind of chat
//! (`Data::NotifySettings` for the default peers) and the contact-joined
//! event (`account.setContactSignUpNotification`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{NotificationSettingsScope, OptionValue};
use td_api::functions::{
    GetChatNotificationSettingsExceptions, GetOption, GetScopeNotificationSettings, SetOption,
    SetScopeNotificationSettings,
};
use td_api::types as td;

use crate::chats::MuteScope;
use crate::{ChatId, RequestError, Session};

/// TDLib's option for the "contact joined Telegram" event.
const CONTACT_JOINED_OFF: &str = "disable_contact_registered_notifications";

/// A kind of chat's defaults; `stories` stands for the stories of the
/// private chats' scope, which TDLib keeps there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeSettings {
    pub mute_for: i32,
    pub sound_id: i64,
    pub show_preview: bool,
    pub use_default_mute_stories: bool,
    pub mute_stories: bool,
    pub story_sound_id: i64,
    pub show_story_poster: bool,
    pub disable_pinned: bool,
    pub disable_mentions: bool,
}

impl ScopeSettings {
    #[must_use]
    pub fn is_muted(&self) -> bool {
        self.mute_for > 0
    }
}

fn td_scope(scope: MuteScope) -> NotificationSettingsScope {
    match scope {
        MuteScope::Private => {
            NotificationSettingsScope::PrivateChats(td::NotificationSettingsScopePrivateChats {})
        }
        MuteScope::Group => {
            NotificationSettingsScope::GroupChats(td::NotificationSettingsScopeGroupChats {})
        }
        MuteScope::Channel => {
            NotificationSettingsScope::ChannelChats(td::NotificationSettingsScopeChannelChats {})
        }
    }
}

impl From<td::ScopeNotificationSettings> for ScopeSettings {
    fn from(settings: td::ScopeNotificationSettings) -> Self {
        Self {
            mute_for: settings.mute_for,
            sound_id: settings.sound_id,
            show_preview: settings.show_preview,
            use_default_mute_stories: settings.use_default_mute_stories,
            mute_stories: settings.mute_stories,
            story_sound_id: settings.story_sound_id,
            show_story_poster: settings.show_story_poster,
            disable_pinned: settings.disable_pinned_message_notifications,
            disable_mentions: settings.disable_mention_notifications,
        }
    }
}

impl From<ScopeSettings> for td::ScopeNotificationSettings {
    fn from(settings: ScopeSettings) -> Self {
        Self {
            mute_for: settings.mute_for,
            sound_id: settings.sound_id,
            show_preview: settings.show_preview,
            use_default_mute_stories: settings.use_default_mute_stories,
            mute_stories: settings.mute_stories,
            story_sound_id: settings.story_sound_id,
            show_story_poster: settings.show_story_poster,
            disable_pinned_message_notifications: settings.disable_pinned,
            disable_mention_notifications: settings.disable_mentions,
        }
    }
}

impl Session {
    pub fn scope_settings(
        &self,
        scope: MuteScope,
    ) -> impl Future<Output = Result<ScopeSettings, RequestError>> + Send + use<> {
        self.client()
            .request(&GetScopeNotificationSettings {
                scope: Box::new(td_scope(scope)),
            })
            .map(|result| result.map(ScopeSettings::from).map_err(RequestError::from))
    }

    pub fn set_scope_settings(
        &self,
        scope: MuteScope,
        settings: ScopeSettings,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetScopeNotificationSettings {
                scope: Box::new(td_scope(scope)),
                notification_settings: Box::new(settings.into()),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    /// The chats of `scope` whose mute differs from its default.
    pub fn notification_exceptions(
        &self,
        scope: MuteScope,
    ) -> impl Future<Output = Result<Vec<ChatId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetChatNotificationSettingsExceptions {
                scope: Some(Box::new(td_scope(scope))),
                compare_sound: false,
            })
            .map(|result| {
                result
                    .map(|chats| chats.chat_ids.into_iter().map(ChatId).collect())
                    .map_err(RequestError::from)
            })
    }

    /// Whether Telegram tells when a contact joins.
    pub fn notifies_contact_joined(
        &self,
    ) -> impl Future<Output = Result<bool, RequestError>> + Send + use<> {
        self.client()
            .request(&GetOption {
                name: CONTACT_JOINED_OFF.to_owned(),
            })
            .map(|result| {
                result
                    .map(|value| {
                        !matches!(
                            value,
                            OptionValue::Boolean(td::OptionValueBoolean { value: true })
                        )
                    })
                    .map_err(RequestError::from)
            })
    }

    pub fn set_notifies_contact_joined(
        &self,
        notify: bool,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetOption {
                name: CONTACT_JOINED_OFF.to_owned(),
                value: Some(Box::new(OptionValue::Boolean(td::OptionValueBoolean {
                    value: !notify,
                }))),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}
