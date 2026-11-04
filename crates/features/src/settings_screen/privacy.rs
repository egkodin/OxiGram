//! "Privacy and Security" (`Settings::PrivacySecurity`): the security rows
//! with their states, every privacy rule, archiving strangers, suggested
//! contacts and the account's self-destruct period.
// shortcut: the security rows, messages privacy, payment info and self-destruct show their state but open nothing yet, and there are no file confirmations; with rows "Privacy and security" and the pages they open.

use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, Styled, Task, Window, div,
};
use i18n::key;
use model::RequestError;
use model::privacy::{NewChatPrivacy, PasswordSummary, PrivacyKey, PrivacyOption, PrivacyRule};
use style::menu_icons::{
    MENU_ICON_BLOCK, MENU_ICON_DEVICES, MENU_ICON_IP_ADDRESS, MENU_ICON_LOCK,
    MENU_ICON_PERMISSIONS, MENU_ICON_RECOVERY_EMAIL, MENU_ICON_TTL, MENU_ICON2_SV,
};
use style::settings::{SETTINGS_PRIVACY_SECURITY_PADDING, SETTINGS_PRIVACY_SKIP};
use ui::SettingsButton;

use super::{Env, privacy_box, rows};
use crate::lang::{strings, tr, tr_plural};

/// TDLib's options for strangers' chats and suggested contacts.
const CAN_ARCHIVE: &str = "can_archive_and_mute_new_chats_from_unknown_users";
const TOP_CHATS_OFF: &str = "disable_top_chats";

/// The rules in tdesktop's order; messages sit between voices and birthday.
const RULES_BEFORE_MESSAGES: [(PrivacyKey, &str, i18n::Key); 6] = [
    (
        PrivacyKey::PhoneNumber,
        "privacy/phone_number",
        key::lng_settings_phone_number_privacy,
    ),
    (
        PrivacyKey::LastSeen,
        "privacy/last_seen",
        key::lng_settings_last_seen,
    ),
    (
        PrivacyKey::ProfilePhoto,
        "privacy/profile_photo",
        key::lng_settings_profile_photo_privacy,
    ),
    (
        PrivacyKey::Forwards,
        "privacy/forwards",
        key::lng_settings_forwards_privacy,
    ),
    (PrivacyKey::Calls, "privacy/calls", key::lng_settings_calls),
    (
        PrivacyKey::Voices,
        "privacy/voices",
        key::lng_settings_voices_privacy,
    ),
];
const RULES_AFTER_MESSAGES: [(PrivacyKey, &str, i18n::Key); 5] = [
    (
        PrivacyKey::Birthday,
        "privacy/birthday",
        key::lng_settings_birthday_privacy,
    ),
    (
        PrivacyKey::Gifts,
        "privacy/gifts",
        key::lng_settings_gifts_privacy,
    ),
    (
        PrivacyKey::About,
        "privacy/bio",
        key::lng_settings_bio_privacy,
    ),
    (
        PrivacyKey::SavedMusic,
        "privacy/saved_music",
        key::lng_settings_saved_music_privacy,
    ),
    (
        PrivacyKey::Invites,
        "privacy/groups",
        key::lng_settings_groups_invite,
    ),
];

/// What Telegram has told so far; None while asked.
#[derive(Debug, Default)]
struct Loaded {
    password: Option<PasswordSummary>,
    auto_delete: Option<i32>,
    passkeys: Vec<String>,
    blocked: Option<i32>,
    websites: usize,
    sessions: Option<usize>,
    rules: HashMap<PrivacyKey, PrivacyRule>,
    new_chats: Option<NewChatPrivacy>,
    /// None when Telegram offers no archiving of strangers.
    archive_unknown: Option<bool>,
    top_peers: Option<bool>,
    account_ttl: Option<i32>,
}

pub(super) struct PrivacyPage {
    env: Env,
    loaded: Loaded,
    loads: Vec<Task<()>>,
}

impl PrivacyPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            env,
            loaded: Loaded::default(),
            loads: Vec::new(),
        };
        page.load(cx);
        page
    }

    fn fetch<T: Send + 'static>(
        &mut self,
        what: &'static str,
        request: impl Future<Output = Result<T, RequestError>> + Send + 'static,
        apply: impl FnOnce(&mut Loaded, T) + 'static,
        cx: &mut Context<Self>,
    ) {
        let task = crate::store::spawn(request, cx);
        self.loads
            .push(cx.spawn(async move |this, cx| match task.await {
                Ok(value) => {
                    let _ = this.update(cx, |this, cx| {
                        apply(&mut this.loaded, value);
                        cx.notify();
                    });
                }
                Err(error) => tracing::warn!(%error, what, "privacy page request failed"),
            }));
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let session = self.env.targets.store.read(cx).session().clone();
        self.fetch(
            "password",
            session.password_summary(),
            |l, v| l.password = Some(v),
            cx,
        );
        self.fetch(
            "auto-delete",
            session.default_auto_delete(),
            |l, v| l.auto_delete = Some(v),
            cx,
        );
        self.fetch(
            "passkeys",
            session.passkey_names(),
            |l, v| l.passkeys = v,
            cx,
        );
        self.fetch(
            "blocked",
            session.blocked_count(),
            |l, v| l.blocked = Some(v),
            cx,
        );
        self.fetch(
            "websites",
            session.websites_count(),
            |l, v| l.websites = v,
            cx,
        );
        self.fetch(
            "sessions",
            session.sessions_count(),
            |l, v| l.sessions = Some(v),
            cx,
        );
        for (rule, _, _) in RULES_BEFORE_MESSAGES.iter().chain(&RULES_AFTER_MESSAGES) {
            let rule = *rule;
            self.fetch(
                "privacy rule",
                session.privacy_rule(rule),
                move |l, v| {
                    l.rules.insert(rule, v);
                },
                cx,
            );
        }
        self.fetch(
            "new chats",
            session.new_chat_privacy(),
            |l, v| l.new_chats = Some(v),
            cx,
        );
        let (can, archive) = (session.bool_option(CAN_ARCHIVE), session.archives_unknown());
        let archive = async move {
            if can.await? {
                archive.await.map(Some)
            } else {
                Ok(None)
            }
        };
        self.fetch("archive", archive, |l, v| l.archive_unknown = v, cx);
        self.fetch(
            "top chats",
            session.bool_option(TOP_CHATS_OFF),
            |l, v| l.top_peers = Some(!v),
            cx,
        );
        self.fetch(
            "account ttl",
            session.account_ttl_days(),
            |l, v| l.account_ttl = Some(v),
            cx,
        );
    }

    fn security(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let loaded = &self.loaded;
        let loading = tr(cx, key::lng_profile_loading);
        let (on, off) = (
            tr(cx, key::lng_settings_cloud_password_on),
            tr(cx, key::lng_settings_cloud_password_off),
        );
        let password = match &loaded.password {
            None => loading.clone(),
            Some(state) if state.has_password => on,
            Some(_) => off.clone(),
        };
        let auto_delete = match loaded.auto_delete {
            None => loading,
            Some(0) => tr(cx, key::lng_settings_ttl_after_off),
            Some(seconds) => crate::history::format_ttl(strings(cx), seconds).into(),
        };
        let mut items = vec![
            rows::skip_by(SETTINGS_PRIVACY_SKIP),
            rows::subtitle(tr(cx, key::lng_settings_security), cx),
            row(
                "security/cloud_password",
                key::lng_settings_cloud_password_start_title,
                MENU_ICON2_SV,
                password,
                cx,
            ),
            row(
                "security/ttl",
                key::lng_settings_ttl_title,
                MENU_ICON_TTL,
                auto_delete,
                cx,
            ),
            // shortcut: no local passcode yet, so it reads off; with row "Passcode lock".
            row(
                "security/passcode",
                key::lng_settings_passcode_title,
                MENU_ICON_LOCK,
                off.clone(),
                cx,
            ),
        ];
        if !loaded.passkeys.is_empty() {
            let label = match loaded.passkeys.as_slice() {
                [only] => only.clone(),
                many => many.len().to_string(),
            };
            items.push(row(
                "security/passkeys",
                key::lng_settings_passkeys_title,
                MENU_ICON_PERMISSIONS,
                label.into(),
                cx,
            ));
        }
        if let Some(state) = loaded
            .password
            .as_ref()
            .filter(|state| !state.login_email_pattern.is_empty())
        {
            let label = if state.login_email_pattern.contains(' ') {
                off
            } else {
                state.login_email_pattern.clone().into()
            };
            items.push(row(
                "security/login_email",
                key::lng_settings_cloud_login_email_section_title,
                MENU_ICON_RECOVERY_EMAIL,
                label,
                cx,
            ));
        }
        let blocked = match loaded.blocked {
            Some(0) | None => tr(cx, key::lng_settings_no_blocked_users),
            Some(count) => count.to_string().into(),
        };
        items.push(row(
            "security/blocked",
            key::lng_settings_blocked_users,
            MENU_ICON_BLOCK,
            blocked,
            cx,
        ));
        if loaded.websites > 0 {
            let count = loaded.websites.to_string().into();
            items.push(row(
                "security/websites",
                key::lng_settings_logged_in,
                MENU_ICON_IP_ADDRESS,
                count,
                cx,
            ));
        }
        let sessions = loaded
            .sessions
            .filter(|count| *count > 0)
            .map(|count| count.to_string())
            .unwrap_or_default();
        items.push(row(
            "security/sessions",
            key::lng_settings_show_sessions,
            MENU_ICON_DEVICES,
            sessions.into(),
            cx,
        ));
        items.push(rows::skip());
        items.push(rows::divider_text(
            tr(cx, key::lng_settings_sessions_about),
            cx,
        ));
        items
    }

    fn rule_row(
        &self,
        rule: (PrivacyKey, &'static str, i18n::Key),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (key, id, title) = rule;
        let current = self.loaded.rules.get(&key).cloned();
        let label = current
            .as_ref()
            .map(|rule| rule_label(rule, cx, &self.env))
            .unwrap_or_default();
        let (env, weak) = (self.env.clone(), cx.weak_entity());
        rows::plain(id, tr(cx, title))
            .label(label)
            .on_click(move |_, window, cx| {
                let Some(rule) = current.clone() else {
                    return;
                };
                let weak = weak.clone();
                let on_saved: privacy_box::OnSaved = Rc::new(move |saved, cx| {
                    let _ = weak.update(cx, |this: &mut PrivacyPage, cx| {
                        this.loaded.rules.insert(key, saved);
                        cx.notify();
                    });
                });
                privacy_box::show(&env, key, rule, on_saved, window, cx);
            })
            .into_any_element()
    }

    fn privacy(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items = vec![
            rows::skip_by(SETTINGS_PRIVACY_SKIP),
            rows::subtitle(tr(cx, key::lng_settings_privacy_title), cx),
        ];
        for rule in RULES_BEFORE_MESSAGES {
            items.push(self.rule_row(rule, cx));
        }
        let messages = match self.loaded.new_chats {
            None => String::new(),
            Some(chats) if chats.paid_stars > 0 => tr(cx, key::lng_edit_privacy_paid).to_string(),
            Some(chats) if !chats.allow_unknown => {
                tr(cx, key::lng_edit_privacy_contacts_and_premium).to_string()
            }
            Some(_) => tr(cx, key::lng_edit_privacy_everyone).to_string(),
        };
        items.push(
            rows::plain(
                "privacy/messages",
                tr(cx, key::lng_settings_messages_privacy),
            )
            .label(messages)
            .into_any_element(),
        );
        for rule in RULES_AFTER_MESSAGES {
            items.push(self.rule_row(rule, cx));
        }
        items.push(rows::skip_by(SETTINGS_PRIVACY_SECURITY_PADDING));
        items.push(rows::divider(cx));
        items
    }

    fn set_archive_unknown(&mut self, archive: bool, cx: &mut Context<Self>) {
        self.loaded.archive_unknown = Some(archive);
        let task = self
            .env
            .targets
            .store
            .read(cx)
            .session()
            .set_archives_unknown(archive);
        crate::peer_menu::send(task, "archive new chats", cx);
        cx.notify();
    }

    fn set_top_peers(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.loaded.top_peers = Some(enabled);
        let task = self
            .env
            .targets
            .store
            .read(cx)
            .session()
            .set_bool_option(TOP_CHATS_OFF, !enabled);
        crate::peer_menu::send(task, "suggested contacts", cx);
        cx.notify();
    }

    fn rest(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items = Vec::new();
        if let Some(archive) = self.loaded.archive_unknown {
            let weak = cx.weak_entity();
            items.extend([
                rows::skip(),
                rows::subtitle(tr(cx, key::lng_settings_new_unknown), cx),
                rows::plain(
                    "privacy/archive_and_mute",
                    tr(cx, key::lng_settings_auto_archive),
                )
                .toggle(archive)
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.set_archive_unknown(!archive, cx));
                })
                .into_any_element(),
                rows::skip(),
                rows::divider_text(tr(cx, key::lng_settings_auto_archive_about), cx),
            ]);
        }
        items.extend([
            rows::skip(),
            rows::subtitle(tr(cx, key::lng_settings_security_bots), cx),
            rows::plain(
                "privacy/bots_payment",
                tr(cx, key::lng_settings_clear_payment_info),
            )
            .into_any_element(),
            rows::skip(),
            rows::divider(cx),
        ]);
        let top_peers = self.loaded.top_peers.unwrap_or(false);
        let weak = cx.weak_entity();
        items.extend([
            rows::skip(),
            rows::subtitle(tr(cx, key::lng_settings_top_peers_title), cx),
            rows::plain(
                "privacy/top_peers_toggle",
                tr(cx, key::lng_settings_top_peers_suggest),
            )
            .toggle(top_peers)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.set_top_peers(!top_peers, cx));
            })
            .into_any_element(),
            rows::skip(),
            rows::divider_text(tr(cx, key::lng_settings_top_peers_about), cx),
        ]);
        let days = self
            .loaded
            .account_ttl
            .map(|days| days_label(days, cx))
            .unwrap_or_default();
        items.extend([
            rows::skip(),
            rows::subtitle(tr(cx, key::lng_settings_destroy_title), cx),
            rows::plain(
                "privacy/self_destruct_button",
                tr(cx, key::lng_settings_destroy_if),
            )
            .label(days)
            .into_any_element(),
            rows::skip(),
        ]);
        items
    }
}

fn row(
    id: &'static str,
    title: i18n::Key,
    icon: style::Icon,
    label: gpui::SharedString,
    cx: &App,
) -> AnyElement {
    let button: SettingsButton = rows::button(id, tr(cx, title), icon).label(label);
    button.into_any_element()
}

/// `PrivacyString`: who, then the never and always counts.
fn rule_label(rule: &PrivacyRule, cx: &App, env: &Env) -> String {
    let base = match rule.option {
        PrivacyOption::Everyone if rule.never.bots => key::lng_edit_privacy_no_miniapps,
        PrivacyOption::Everyone => key::lng_edit_privacy_everyone,
        PrivacyOption::Contacts if rule.always.premiums => {
            key::lng_edit_privacy_contacts_and_premium
        }
        PrivacyOption::Contacts if rule.always.bots => key::lng_edit_privacy_contacts_and_miniapps,
        PrivacyOption::Contacts => key::lng_edit_privacy_contacts,
        PrivacyOption::Nobody if rule.always.premiums => key::lng_edit_privacy_premium,
        PrivacyOption::Nobody if rule.always.bots => key::lng_edit_privacy_miniapps,
        PrivacyOption::Nobody => key::lng_edit_privacy_nobody,
    };
    let never = privacy_box::exception_users(&rule.never, cx, env);
    let always = privacy_box::exception_users(&rule.always, cx, env);
    with_counts(tr(cx, base).to_string(), never, always)
}

fn with_counts(base: String, never: i64, always: i64) -> String {
    let added: Vec<_> = [(never, '-'), (always, '+')]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, sign)| format!("{sign}{count}"))
        .collect();
    if added.is_empty() {
        base
    } else {
        format!("{base} ({})", added.join(", "))
    }
}

/// `SelfDestructionBox::DaysLabel`.
fn days_label(days: i32, cx: &App) -> String {
    match days {
        0 => String::new(),
        days if days > 25 => tr_plural(cx, key::lng_months, i64::from((days / 30).max(1)), &[]),
        days => tr_plural(cx, key::lng_weeks, i64::from((days / 7).max(1)), &[]),
    }
}

impl Render for PrivacyPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .children(self.security(cx))
            .children(self.privacy(cx))
            .children(self.rest(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_labels_add_never_then_always_counts() {
        assert_eq!(with_counts("Nobody".into(), 0, 0), "Nobody");
        assert_eq!(with_counts("My contacts".into(), 2, 0), "My contacts (-2)");
        assert_eq!(with_counts("Everybody".into(), 1, 3), "Everybody (-1, +3)");
    }
}
