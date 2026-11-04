//! One privacy setting (`EditPrivacyBox` with the setting's controller):
//! who may, a warning, the "always" and "never" exceptions, Save.
// shortcut: exceptions pick contacts only (groups already there stay), no "who can find me by number" part, profile photo's public photo, forwards' sample message, calls' peer-to-peer part, Premium rows or last seen's read-time switch and one-time confirmation; with row "Privacy rules".

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div, px,
};
use i18n::{Key, key};
use model::UserId;
use model::privacy::{Exceptions, PrivacyKey, PrivacyOption, PrivacyRule};
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::settings::{
    SETTINGS_PRIVACY_OPTION, SETTINGS_PRIVACY_SKIP_TOP, SETTINGS_SEND_TYPE_PADDING,
};
use style::widgets::DEFAULT_RADIO;
use ui::{BoxFrame, Checkbox, Checked, RoundButton};

use super::{Env, rows};
use crate::contacts_box::{self, Cancel, Many};
use crate::lang::{tr, tr_plural, tr_with};
use crate::px_of;

pub(super) type OnSaved = Rc<dyn Fn(PrivacyRule, &mut App)>;

/// Which exceptions list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum List {
    Always,
    Never,
}

const OPTIONS: [PrivacyOption; 3] = [
    PrivacyOption::Everyone,
    PrivacyOption::Contacts,
    PrivacyOption::Nobody,
];

/// The part of the setting's string names after `lng_edit_privacy_`.
fn prefix(key: PrivacyKey) -> &'static str {
    match key {
        PrivacyKey::PhoneNumber | PrivacyKey::AddedByPhone => "phone_number",
        PrivacyKey::LastSeen => "lastseen",
        PrivacyKey::ProfilePhoto => "profile_photo",
        PrivacyKey::Forwards => "forwards",
        PrivacyKey::Calls => "calls",
        PrivacyKey::Voices => "voices",
        PrivacyKey::Birthday => "birthday",
        PrivacyKey::Gifts => "gifts",
        PrivacyKey::About => "about",
        PrivacyKey::SavedMusic => "saved_music",
        PrivacyKey::Invites => "groups",
    }
}

/// `lng_edit_privacy_<prefix>_<suffix>`.
fn text_key(key: PrivacyKey, suffix: &str) -> Key {
    let name = format!("lng_edit_privacy_{}_{suffix}", prefix(key));
    Key::from_name(&name).expect("every privacy setting has its strings, see the test")
}

/// `optionsTitleKey`'s neighbour: the gray text under the options.
fn warning(key: PrivacyKey, option: PrivacyOption) -> Option<Key> {
    match key {
        PrivacyKey::PhoneNumber if option == PrivacyOption::Nobody => {
            Some(key::lng_edit_privacy_phone_number_contacts)
        }
        PrivacyKey::PhoneNumber => Some(key::lng_edit_privacy_phone_number_warning),
        PrivacyKey::LastSeen => Some(key::lng_edit_privacy_lastseen_warning),
        PrivacyKey::Forwards => Some(key::lng_edit_privacy_forwards_warning),
        _ => None,
    }
}

/// `exceptionsDescription`.
fn exceptions_about(key: PrivacyKey, option: PrivacyOption) -> Key {
    match (key, option) {
        (PrivacyKey::ProfilePhoto, PrivacyOption::Everyone) => {
            key::lng_edit_privacy_forwards_exceptions_everyone
        }
        (PrivacyKey::ProfilePhoto, PrivacyOption::Contacts) => {
            key::lng_edit_privacy_forwards_exceptions
        }
        (PrivacyKey::ProfilePhoto, PrivacyOption::Nobody) => {
            key::lng_edit_privacy_forwards_exceptions_nobody
        }
        (PrivacyKey::Gifts, _) => key::lng_edit_privacy_lastseen_exceptions,
        _ => text_key(key, "exceptions"),
    }
}

/// `showExceptionLink`: "always" means nothing for everybody, "never" for nobody.
fn is_shown(list: List, option: PrivacyOption) -> bool {
    match list {
        List::Always => option != PrivacyOption::Everyone,
        List::Never => option != PrivacyOption::Nobody,
    }
}

/// `ExceptionUsersCount`: users count one, groups their members.
pub(super) fn exception_users(exceptions: &Exceptions, cx: &App, env: &Env) -> i64 {
    let state = env.targets.store.read(cx).state();
    let members: i64 = exceptions
        .chats
        .iter()
        .filter_map(|chat| state.chats.chat(*chat))
        .filter_map(|chat| state.chats.member_count(chat))
        .map(i64::from)
        .sum();
    i64::try_from(exceptions.users.len()).unwrap_or(i64::MAX) + members
}

pub(super) struct PrivacyBox {
    env: Env,
    key: PrivacyKey,
    rule: PrivacyRule,
    on_saved: OnSaved,
    focus: FocusHandle,
}

impl std::fmt::Debug for PrivacyBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivacyBox")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl Focusable for PrivacyBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(
    env: &Env,
    key: PrivacyKey,
    rule: PrivacyRule,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let view = cx.new(|cx| PrivacyBox {
        env: env.clone(),
        key,
        rule,
        on_saved,
        focus: cx.focus_handle(),
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl PrivacyBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn save(&self, window: &mut Window, cx: &mut Context<Self>) {
        let rule = self.rule.clone();
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .set_privacy_rule(self.key, &rule),
            cx,
        );
        let on_saved = self.on_saved.clone();
        cx.spawn(async move |_, cx| match task.await {
            Ok(()) => cx.update(|cx| on_saved(rule, cx)),
            Err(error) => tracing::warn!(%error, "saving a privacy rule failed"),
        })
        .detach();
        self.close(window, cx);
    }

    /// `editExceptions`: the contacts box with the list's users ticked; the
    /// ones picked leave the other list.
    fn edit(&self, list: List, window: &mut Window, cx: &mut Context<Self>) {
        let (current, title) = match list {
            List::Always => (&self.rule.always, "always_title"),
            List::Never => (&self.rule.never, "never_title"),
        };
        let weak = cx.weak_entity();
        let many = Many {
            title: tr(cx, text_key(self.key, title)),
            done: tr(cx, key::lng_settings_save),
            on_done: Rc::new(move |users: Vec<UserId>, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.set_users(list, users);
                    cx.notify();
                });
            }),
            cancel: Cancel::Close,
            picked: current.users.clone(),
        };
        let targets = &self.env.targets;
        contacts_box::show_many(&targets.store, &targets.layers, many, window, cx);
    }

    fn set_users(&mut self, list: List, users: Vec<UserId>) {
        let (target, other) = match list {
            List::Always => (&mut self.rule.always, &mut self.rule.never),
            List::Never => (&mut self.rule.never, &mut self.rule.always),
        };
        other.users.retain(|user| !users.contains(user));
        target.users = users;
    }

    fn option_row(&self, option: PrivacyOption, cx: &mut Context<Self>) -> gpui::Div {
        let (id, text) = match option {
            PrivacyOption::Everyone => ("privacy-everyone", key::lng_edit_privacy_everyone),
            PrivacyOption::Contacts => ("privacy-contacts", key::lng_edit_privacy_contacts),
            PrivacyOption::Nobody => ("privacy-nobody", key::lng_edit_privacy_nobody),
        };
        let checked = if self.rule.option == option {
            Checked::Yes
        } else {
            Checked::No
        };
        let weak = cx.weak_entity();
        let padding = SETTINGS_SEND_TYPE_PADDING;
        div()
            .pl(px_of(padding.left) - px(1.0))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top) + px_of(SETTINGS_PRIVACY_SKIP_TOP))
            .pb(px_of(padding.bottom))
            .child(
                Checkbox::radio(
                    id,
                    &SETTINGS_PRIVACY_OPTION,
                    &DEFAULT_RADIO,
                    tr(cx, text),
                    checked,
                )
                .on_toggle(move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.rule.option = option;
                        cx.notify();
                    });
                }),
            )
    }

    /// The count at the right of an exceptions row.
    fn list_label(&self, list: List, cx: &App) -> SharedString {
        let exceptions = match list {
            List::Always => &self.rule.always,
            List::Never => &self.rule.never,
        };
        let count = exception_users(exceptions, cx, &self.env);
        let users = if count > 0 {
            tr_plural(cx, key::lng_edit_privacy_exceptions_count, count, &[])
        } else {
            tr(cx, key::lng_edit_privacy_exceptions_add).to_string()
        };
        let with = |alone: Key, and: Key| {
            if count > 0 {
                tr_with(cx, and, &[("users", &users)])
            } else {
                tr(cx, alone).to_string()
            }
        };
        let text = if exceptions.premiums {
            with(
                key::lng_edit_privacy_premium,
                key::lng_edit_privacy_exceptions_premium_and,
            )
        } else if exceptions.bots {
            with(
                key::lng_edit_privacy_miniapps,
                key::lng_edit_privacy_exceptions_miniapps_and,
            )
        } else {
            users
        };
        text.into()
    }

    fn list_row(&self, list: List, cx: &mut Context<Self>) -> Option<ui::SettingsButton> {
        if !is_shown(list, self.rule.option) {
            return None;
        }
        let (id, suffix) = match list {
            List::Always => ("privacy-always", "always_empty"),
            List::Never => ("privacy-never", "never_empty"),
        };
        let weak = cx.weak_entity();
        Some(
            rows::plain(id, tr(cx, text_key(self.key, suffix)))
                .label(self.list_label(list, cx))
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.edit(list, window, cx));
                }),
        )
    }
}

impl Render for PrivacyBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let options: Vec<_> = OPTIONS
            .iter()
            .map(|option| self.option_row(*option, cx))
            .collect();
        let under_options = match warning(self.key, self.rule.option) {
            Some(text) => rows::divider_text(tr(cx, text), cx),
            None => rows::divider(cx),
        };
        let about = exceptions_about(self.key, self.rule.option);
        let content = div()
            .flex()
            .flex_col()
            .child(rows::subtitle(tr(cx, text_key(self.key, "header")), cx))
            .children(options)
            .child(rows::skip_by(style::Px(
                style::basic::DEFAULT_VERTICAL_LIST_SKIP.0 + SETTINGS_PRIVACY_SKIP_TOP.0,
            )))
            .child(under_options)
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_edit_privacy_exceptions), cx))
            .children(self.list_row(List::Always, cx))
            .children(self.list_row(List::Never, cx))
            .child(rows::skip())
            .child(rows::divider_text(tr(cx, about), cx));
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        div().track_focus(&self.focus).child(
            BoxFrame::new("privacy-box", &DEFAULT_BOX)
                .width(px_of(BOX_WIDE_WIDTH))
                .title(tr(cx, text_key(self.key, "title")))
                .row(content)
                .button(
                    RoundButton::new(
                        "privacy-save",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_settings_save),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = save.update(cx, |this, cx| this.save(window, cx));
                    }),
                )
                .button(
                    RoundButton::new(
                        "privacy-cancel",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_cancel),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                    }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [PrivacyKey; 11] = [
        PrivacyKey::PhoneNumber,
        PrivacyKey::LastSeen,
        PrivacyKey::ProfilePhoto,
        PrivacyKey::Forwards,
        PrivacyKey::Calls,
        PrivacyKey::Voices,
        PrivacyKey::Birthday,
        PrivacyKey::Gifts,
        PrivacyKey::About,
        PrivacyKey::SavedMusic,
        PrivacyKey::Invites,
    ];

    #[test]
    fn every_setting_has_its_box_strings() {
        for key in KEYS {
            for suffix in [
                "title",
                "header",
                "always_empty",
                "never_empty",
                "always_title",
                "never_title",
            ] {
                text_key(key, suffix);
            }
            for option in OPTIONS {
                exceptions_about(key, option);
            }
        }
    }

    #[test]
    fn exceptions_show_only_where_they_change_something() {
        assert!(!is_shown(List::Always, PrivacyOption::Everyone));
        assert!(is_shown(List::Never, PrivacyOption::Everyone));
        assert!(is_shown(List::Always, PrivacyOption::Contacts));
        assert!(is_shown(List::Never, PrivacyOption::Contacts));
        assert!(!is_shown(List::Never, PrivacyOption::Nobody));
    }
}
