//! Privacy rules (`Api::UserPrivacy`) and what Settings' Privacy and
//! Security page shows about the account: the cloud password, the
//! auto-delete and self-destruct periods, and the blocked users, sessions,
//! websites and passkeys counts.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{
    BlockList, MessageSender, OptionValue, UserPrivacySetting, UserPrivacySettingRule as TdRule,
};
use td_api::functions::{
    DeleteSavedCredentials, DeleteSavedOrderInfo, SetAccountTtl, SetDefaultMessageAutoDeleteTime,
    SetMessageSenderBlockList,
};
use td_api::functions::{
    GetAccountTtl, GetActiveSessions, GetArchiveChatListSettings, GetBlockedMessageSenders,
    GetConnectedWebsites, GetDefaultMessageAutoDeleteTime, GetLoginPasskeys,
    GetNewChatPrivacySettings, GetOption, GetPasswordState, GetUserPrivacySettingRules,
    SetArchiveChatListSettings, SetNewChatPrivacySettings, SetOption, SetUserPrivacySettingRules,
};
use td_api::types as td;

use crate::{ChatId, RequestError, Session, UserId};

/// A privacy setting, in the order of tdesktop's Privacy group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrivacyKey {
    PhoneNumber,
    /// Who can find the account by its number (the phone number box).
    AddedByPhone,
    LastSeen,
    ProfilePhoto,
    Forwards,
    Calls,
    Voices,
    Birthday,
    Gifts,
    About,
    SavedMusic,
    Invites,
}

impl PrivacyKey {
    fn td(self) -> UserPrivacySetting {
        match self {
            Self::PhoneNumber => {
                UserPrivacySetting::ShowPhoneNumber(td::UserPrivacySettingShowPhoneNumber {})
            }
            Self::AddedByPhone => UserPrivacySetting::AllowFindingByPhoneNumber(
                td::UserPrivacySettingAllowFindingByPhoneNumber {},
            ),
            Self::LastSeen => UserPrivacySetting::ShowStatus(td::UserPrivacySettingShowStatus {}),
            Self::ProfilePhoto => {
                UserPrivacySetting::ShowProfilePhoto(td::UserPrivacySettingShowProfilePhoto {})
            }
            Self::Forwards => UserPrivacySetting::ShowLinkInForwardedMessages(
                td::UserPrivacySettingShowLinkInForwardedMessages {},
            ),
            Self::Calls => UserPrivacySetting::AllowCalls(td::UserPrivacySettingAllowCalls {}),
            Self::Voices => UserPrivacySetting::AllowPrivateVoiceAndVideoNoteMessages(
                td::UserPrivacySettingAllowPrivateVoiceAndVideoNoteMessages {},
            ),
            Self::Birthday => {
                UserPrivacySetting::ShowBirthdate(td::UserPrivacySettingShowBirthdate {})
            }
            Self::Gifts => {
                UserPrivacySetting::AutosaveGifts(td::UserPrivacySettingAutosaveGifts {})
            }
            Self::About => UserPrivacySetting::ShowBio(td::UserPrivacySettingShowBio {}),
            Self::SavedMusic => {
                UserPrivacySetting::ShowProfileAudio(td::UserPrivacySettingShowProfileAudio {})
            }
            Self::Invites => {
                UserPrivacySetting::AllowChatInvites(td::UserPrivacySettingAllowChatInvites {})
            }
        }
    }
}

/// Who a setting allows (`Api::UserPrivacy::Option`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyOption {
    Everyone,
    Contacts,
    Nobody,
}

/// The "always" or "never" list of a rule.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exceptions {
    pub users: Vec<UserId>,
    /// Members of these groups.
    pub chats: Vec<ChatId>,
    pub premiums: bool,
    /// Bots and mini apps.
    pub bots: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRule {
    pub option: PrivacyOption,
    pub always: Exceptions,
    pub never: Exceptions,
}

impl PrivacyRule {
    /// `RulesFromTL`: the option rule, the lists around it; no option rule
    /// means nobody.
    fn from_td(rules: Vec<TdRule>) -> Self {
        let mut rule = Self {
            option: PrivacyOption::Nobody,
            always: Exceptions::default(),
            never: Exceptions::default(),
        };
        for item in rules {
            match item {
                TdRule::AllowAll(_) => rule.option = PrivacyOption::Everyone,
                TdRule::AllowContacts(_) => rule.option = PrivacyOption::Contacts,
                TdRule::RestrictAll(_) => rule.option = PrivacyOption::Nobody,
                TdRule::AllowBots(_) => rule.always.bots = true,
                TdRule::AllowPremiumUsers(_) => rule.always.premiums = true,
                TdRule::AllowUsers(list) => {
                    rule.always
                        .users
                        .extend(list.user_ids.into_iter().map(UserId));
                }
                TdRule::AllowChatMembers(list) => {
                    rule.always
                        .chats
                        .extend(list.chat_ids.into_iter().map(ChatId));
                }
                TdRule::RestrictBots(_) => rule.never.bots = true,
                TdRule::RestrictUsers(list) => {
                    rule.never
                        .users
                        .extend(list.user_ids.into_iter().map(UserId));
                }
                TdRule::RestrictChatMembers(list) => {
                    rule.never
                        .chats
                        .extend(list.chat_ids.into_iter().map(ChatId));
                }
                TdRule::RestrictContacts(_) => {}
            }
        }
        rule
    }

    /// `RulesToTL`: the lists that apply to the option, then the option.
    fn to_td(&self) -> Vec<TdRule> {
        let mut rules = Vec::new();
        if self.option != PrivacyOption::Everyone {
            let always = &self.always;
            if !always.users.is_empty() {
                rules.push(TdRule::AllowUsers(td::UserPrivacySettingRuleAllowUsers {
                    user_ids: always.users.iter().map(|user| user.0).collect(),
                }));
            }
            if !always.chats.is_empty() {
                rules.push(TdRule::AllowChatMembers(
                    td::UserPrivacySettingRuleAllowChatMembers {
                        chat_ids: always.chats.iter().map(|chat| chat.0).collect(),
                    },
                ));
            }
            if always.premiums {
                rules.push(TdRule::AllowPremiumUsers(
                    td::UserPrivacySettingRuleAllowPremiumUsers {},
                ));
            }
            if always.bots {
                rules.push(TdRule::AllowBots(td::UserPrivacySettingRuleAllowBots {}));
            }
        }
        if self.option != PrivacyOption::Nobody {
            let never = &self.never;
            if !never.users.is_empty() {
                rules.push(TdRule::RestrictUsers(
                    td::UserPrivacySettingRuleRestrictUsers {
                        user_ids: never.users.iter().map(|user| user.0).collect(),
                    },
                ));
            }
            if !never.chats.is_empty() {
                rules.push(TdRule::RestrictChatMembers(
                    td::UserPrivacySettingRuleRestrictChatMembers {
                        chat_ids: never.chats.iter().map(|chat| chat.0).collect(),
                    },
                ));
            }
            if never.bots {
                rules.push(TdRule::RestrictBots(
                    td::UserPrivacySettingRuleRestrictBots {},
                ));
            }
        }
        rules.push(match self.option {
            PrivacyOption::Everyone => TdRule::AllowAll(td::UserPrivacySettingRuleAllowAll {}),
            PrivacyOption::Contacts => {
                TdRule::AllowContacts(td::UserPrivacySettingRuleAllowContacts {})
            }
            PrivacyOption::Nobody => TdRule::RestrictAll(td::UserPrivacySettingRuleRestrictAll {}),
        });
        rules
    }
}

/// Someone on the blocked list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Blocked {
    User(UserId),
    Chat(ChatId),
}

impl Blocked {
    fn td(self) -> MessageSender {
        match self {
            Self::User(user) => MessageSender::User(td::MessageSenderUser { user_id: user.0 }),
            Self::Chat(chat) => MessageSender::Chat(td::MessageSenderChat { chat_id: chat.0 }),
        }
    }
}

impl From<MessageSender> for Blocked {
    fn from(sender: MessageSender) -> Self {
        match sender {
            MessageSender::User(user) => Self::User(UserId(user.user_id)),
            MessageSender::Chat(chat) => Self::Chat(ChatId(chat.chat_id)),
        }
    }
}

/// A slice of the blocked list and how long the whole list is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockedSlice {
    pub total: i32,
    pub senders: Vec<Blocked>,
}

/// Who may start a chat (`Api::GlobalPrivacy` for new chats).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewChatPrivacy {
    pub allow_unknown: bool,
    pub paid_stars: i64,
}

/// What the page says about the cloud password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordSummary {
    pub has_password: bool,
    /// The login email with stars, empty when there is none.
    pub login_email_pattern: String,
}

fn request_error(error: tg::TdError) -> RequestError {
    RequestError::from(error)
}

impl Session {
    pub fn privacy_rule(
        &self,
        key: PrivacyKey,
    ) -> impl Future<Output = Result<PrivacyRule, RequestError>> + Send + use<> {
        self.client()
            .request(&GetUserPrivacySettingRules {
                setting: Box::new(key.td()),
            })
            .map(|result| {
                result
                    .map(|rules| PrivacyRule::from_td(rules.rules))
                    .map_err(request_error)
            })
    }

    pub fn set_privacy_rule(
        &self,
        key: PrivacyKey,
        rule: &PrivacyRule,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetUserPrivacySettingRules {
                setting: Box::new(key.td()),
                rules: Box::new(td::UserPrivacySettingRules {
                    rules: rule.to_td(),
                }),
            })
            .map(|result| result.map(drop).map_err(request_error))
    }

    pub fn new_chat_privacy(
        &self,
    ) -> impl Future<Output = Result<NewChatPrivacy, RequestError>> + Send + use<> {
        self.client()
            .request(&GetNewChatPrivacySettings {})
            .map(|result| {
                result
                    .map(|settings| NewChatPrivacy {
                        allow_unknown: settings.allow_new_chats_from_unknown_users,
                        paid_stars: settings.incoming_paid_message_star_count,
                    })
                    .map_err(request_error)
            })
    }

    pub fn set_new_chat_privacy(
        &self,
        privacy: NewChatPrivacy,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetNewChatPrivacySettings {
                settings: Box::new(td::NewChatPrivacySettings {
                    allow_new_chats_from_unknown_users: privacy.allow_unknown,
                    incoming_paid_message_star_count: privacy.paid_stars,
                }),
            })
            .map(|result| result.map(drop).map_err(request_error))
    }

    /// Forgets the shipping address payments filled in.
    pub fn delete_saved_order_info(
        &self,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DeleteSavedOrderInfo {})
            .map(|result| result.map(drop).map_err(request_error))
    }

    /// Forgets the saved payment card.
    pub fn delete_saved_credentials(
        &self,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&DeleteSavedCredentials {})
            .map(|result| result.map(drop).map_err(request_error))
    }

    pub fn password_summary(
        &self,
    ) -> impl Future<Output = Result<PasswordSummary, RequestError>> + Send + use<> {
        self.client().request(&GetPasswordState {}).map(|result| {
            result
                .map(|state| PasswordSummary {
                    has_password: state.has_password,
                    login_email_pattern: state.login_email_address_pattern,
                })
                .map_err(request_error)
        })
    }

    /// Days of inactivity before the account is deleted.
    pub fn account_ttl_days(
        &self,
    ) -> impl Future<Output = Result<i32, RequestError>> + Send + use<> {
        self.client()
            .request(&GetAccountTtl {})
            .map(|result| result.map(|ttl| ttl.days).map_err(request_error))
    }

    pub fn set_account_ttl_days(
        &self,
        days: i32,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetAccountTtl {
                ttl: Box::new(td::AccountTtl { days }),
            })
            .map(|result| result.map(drop).map_err(request_error))
    }

    /// Seconds new chats' messages live, 0 for off.
    pub fn default_auto_delete(
        &self,
    ) -> impl Future<Output = Result<i32, RequestError>> + Send + use<> {
        self.client()
            .request(&GetDefaultMessageAutoDeleteTime {})
            .map(|result| result.map(|time| time.time).map_err(request_error))
    }

    /// Seconds new chats' messages live; 0 keeps them.
    pub fn set_default_auto_delete(
        &self,
        seconds: i32,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetDefaultMessageAutoDeleteTime {
                message_auto_delete_time: Box::new(td::MessageAutoDeleteTime { time: seconds }),
            })
            .map(|result| result.map(drop).map_err(request_error))
    }

    pub fn blocked_count(&self) -> impl Future<Output = Result<i32, RequestError>> + Send + use<> {
        self.client()
            .request(&GetBlockedMessageSenders {
                block_list: Box::new(BlockList::Main(td::BlockListMain {})),
                offset: 0,
                limit: 1,
            })
            .map(|result| {
                result
                    .map(|senders| senders.total_count)
                    .map_err(request_error)
            })
    }

    pub fn blocked(
        &self,
        offset: i32,
        limit: i32,
    ) -> impl Future<Output = Result<BlockedSlice, RequestError>> + Send + use<> {
        self.client()
            .request(&GetBlockedMessageSenders {
                block_list: Box::new(BlockList::Main(td::BlockListMain {})),
                offset,
                limit,
            })
            .map(|result| {
                result
                    .map(|senders| BlockedSlice {
                        total: senders.total_count,
                        senders: senders.senders.into_iter().map(Blocked::from).collect(),
                    })
                    .map_err(request_error)
            })
    }

    pub fn unblock(
        &self,
        sender: Blocked,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetMessageSenderBlockList {
                sender_id: Box::new(sender.td()),
                block_list: None,
            })
            .map(|result| result.map(drop).map_err(request_error))
    }

    pub fn sessions_count(
        &self,
    ) -> impl Future<Output = Result<usize, RequestError>> + Send + use<> {
        self.client().request(&GetActiveSessions {}).map(|result| {
            result
                .map(|sessions| sessions.sessions.len())
                .map_err(request_error)
        })
    }

    pub fn websites_count(
        &self,
    ) -> impl Future<Output = Result<usize, RequestError>> + Send + use<> {
        self.client()
            .request(&GetConnectedWebsites {})
            .map(|result| {
                result
                    .map(|sites| sites.websites.len())
                    .map_err(request_error)
            })
    }

    /// The login passkeys' names.
    pub fn passkey_names(
        &self,
    ) -> impl Future<Output = Result<Vec<String>, RequestError>> + Send + use<> {
        self.client().request(&GetLoginPasskeys {}).map(|result| {
            result
                .map(|keys| keys.passkeys.into_iter().map(|key| key.name).collect())
                .map_err(request_error)
        })
    }

    /// Whether new chats from strangers go to the archive muted.
    pub fn archives_unknown(
        &self,
    ) -> impl Future<Output = Result<bool, RequestError>> + Send + use<> {
        self.client()
            .request(&GetArchiveChatListSettings {})
            .map(|result| {
                result
                    .map(|settings| settings.archive_and_mute_new_chats_from_unknown_users)
                    .map_err(request_error)
            })
    }

    /// Archives and mutes new chats from strangers, or stops; the other
    /// archive settings stay as they are.
    pub fn set_archives_unknown(
        &self,
        archive: bool,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let client = self.client().clone();
        async move {
            let mut settings = client
                .request(&GetArchiveChatListSettings {})
                .await
                .map_err(request_error)?;
            settings.archive_and_mute_new_chats_from_unknown_users = archive;
            client
                .request(&SetArchiveChatListSettings {
                    settings: Box::new(settings),
                })
                .await
                .map(drop)
                .map_err(request_error)
        }
    }

    /// A boolean TDLib option; unset reads as false.
    pub fn bool_option(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<bool, RequestError>> + Send + use<> {
        self.client()
            .request(&GetOption {
                name: name.to_owned(),
            })
            .map(|result| {
                result
                    .map(|value| {
                        matches!(
                            value,
                            OptionValue::Boolean(td::OptionValueBoolean { value: true })
                        )
                    })
                    .map_err(request_error)
            })
    }

    pub fn set_bool_option(
        &self,
        name: &str,
        value: bool,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetOption {
                name: name.to_owned(),
                value: Some(Box::new(OptionValue::Boolean(td::OptionValueBoolean {
                    value,
                }))),
            })
            .map(|result| result.map(drop).map_err(request_error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_round_trip_and_drop_lists_the_option_overrides() {
        let rule = PrivacyRule {
            option: PrivacyOption::Contacts,
            always: Exceptions {
                users: vec![UserId(5)],
                premiums: true,
                ..Exceptions::default()
            },
            never: Exceptions {
                chats: vec![ChatId(-7)],
                ..Exceptions::default()
            },
        };
        assert_eq!(PrivacyRule::from_td(rule.to_td()), rule);

        let everyone = PrivacyRule {
            option: PrivacyOption::Everyone,
            ..rule.clone()
        };
        let read = PrivacyRule::from_td(everyone.to_td());
        assert_eq!(read.always, Exceptions::default());
        assert_eq!(read.never, rule.never);

        assert_eq!(
            PrivacyRule::from_td(Vec::new()).option,
            PrivacyOption::Nobody
        );
    }
}
