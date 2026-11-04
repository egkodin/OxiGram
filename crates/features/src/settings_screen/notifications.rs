//! "Notifications and Sounds" (`Settings::Notifications`): the global
//! switches, sender name and preview, the defaults per kind of chat, the
//! events and the badge counter.
// shortcut: no notification preview card, volume slider, accounts switch, calls or system integration groups, and a type row toggles its default instead of opening its page (sound, exceptions, stories, reactions); with M7 notifications.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, Styled, Task, Window, div,
    prelude::*, px,
};
use i18n::key;
use model::chats::MuteScope;
use model::notifications::ScopeSettings;
use style::layers::DEFAULT_BOX_CHECKBOX;
use style::menu_icons::{
    MENU_ICON_CHANNEL, MENU_ICON_DOCK_BOUNCE, MENU_ICON_GROUPS, MENU_ICON_INVITE,
    MENU_ICON_NOTIFICATIONS, MENU_ICON_PIN, MENU_ICON_PROFILE, MENU_ICON_UNMUTE,
};
use style::settings::{
    NOTIFY_PREVIEW_CHECKS_SKIP, SETTINGS_BUTTON_NO_ICON, SETTINGS_NOTIFICATION_TYPE,
    SETTINGS_NOTIFICATION_TYPE_DETAILS,
};
use style::widgets::DEFAULT_CHECK;
use ui::{Checkbox, Checked, SettingsButton, color};

use super::search::{Entry, Target};
use super::{Env, PageId, rows};
use crate::lang::{tr, tr_plural, tr_with};
use crate::px_of;
use crate::settings::{self, Notify, NotifyView};

/// `rightsButtonToggleWidth`.
const TOGGLE_WIDTH: f32 = 70.0;
/// `MuteValue::forever` as TDLib takes it.
const MUTE_FOREVER: i32 = i32::MAX;
const SCOPES: [MuteScope; 3] = [MuteScope::Private, MuteScope::Group, MuteScope::Channel];

const fn item(
    id: &'static str,
    title: i18n::Key,
    keywords: &'static [&'static str],
    icon: Option<style::Icon>,
) -> Entry {
    Entry {
        id,
        title,
        keywords,
        page: PageId::Notifications,
        target: Target::Item,
        icon,
    }
}

/// The page's searchable rows, with tdesktop's ids and keywords.
const ENTRIES: [Entry; 11] = [
    item(
        "notifications/desktop",
        key::lng_settings_desktop_notify,
        &["desktop", "popup", "show"],
        Some(MENU_ICON_NOTIFICATIONS),
    ),
    item(
        "notifications/flash",
        key::lng_settings_alert_mac,
        &["flash", "bounce", "taskbar"],
        Some(MENU_ICON_DOCK_BOUNCE),
    ),
    item(
        "notifications/sound",
        key::lng_settings_sound_allowed,
        &["sound", "audio", "mute"],
        Some(MENU_ICON_UNMUTE),
    ),
    item(
        "notifications/private",
        key::lng_notification_private_chats,
        &["private", "chats", "direct"],
        Some(MENU_ICON_PROFILE),
    ),
    item(
        "notifications/groups",
        key::lng_notification_groups,
        &["groups", "chats"],
        Some(MENU_ICON_GROUPS),
    ),
    item(
        "notifications/channels",
        key::lng_notification_channels,
        &["channels", "broadcast"],
        Some(MENU_ICON_CHANNEL),
    ),
    item(
        "notifications/events/joined",
        key::lng_settings_events_joined,
        &["joined", "contacts", "signup"],
        Some(MENU_ICON_INVITE),
    ),
    item(
        "notifications/events/pinned",
        key::lng_settings_events_pinned,
        &["pinned", "message"],
        Some(MENU_ICON_PIN),
    ),
    item(
        "notifications/badge",
        key::lng_settings_badge_title,
        &["badge", "counter", "unread"],
        None,
    ),
    item(
        "notifications/include-muted-chats",
        key::lng_settings_include_muted,
        &["muted", "badge", "counter"],
        None,
    ),
    item(
        "notifications/count-unread-messages",
        key::lng_settings_count_unread,
        &["unread", "messages", "count"],
        None,
    ),
];

pub(super) fn entries() -> impl Iterator<Item = &'static Entry> {
    ENTRIES.iter()
}

/// What Telegram has for one kind of chat.
#[derive(Debug, Clone, Copy)]
struct Scope {
    settings: ScopeSettings,
    exceptions: usize,
}

pub(super) struct NotificationsPage {
    env: Env,
    scopes: HashMap<MuteScope, Scope>,
    /// None until Telegram answers.
    contact_joined: Option<bool>,
    _loads: Vec<Task<()>>,
}

impl NotificationsPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let mut loads: Vec<_> = SCOPES
            .iter()
            .map(|scope| load_scope(&env, *scope, cx))
            .collect();
        loads.push(load_contact_joined(&env, cx));
        Self {
            env,
            scopes: HashMap::new(),
            contact_joined: None,
            _loads: loads,
        }
    }

    /// The type row's toggle (`AddTypeButton`): mutes or unmutes the default.
    fn toggle_scope(&mut self, scope: MuteScope, cx: &mut Context<Self>) {
        let Some(current) = self.scopes.get(&scope).copied() else {
            return;
        };
        let mute_for = if current.settings.is_muted() {
            0
        } else {
            MUTE_FOREVER
        };
        let settings = ScopeSettings {
            mute_for,
            ..current.settings
        };
        self.scopes.insert(
            scope,
            Scope {
                settings,
                ..current
            },
        );
        let task = self
            .env
            .targets
            .store
            .read(cx)
            .session()
            .set_scope_settings(scope, settings);
        crate::peer_menu::send(task, "scope notification settings", cx);
        cx.notify();
    }

    fn toggle_contact_joined(&mut self, cx: &mut Context<Self>) {
        let Some(current) = self.contact_joined else {
            return;
        };
        self.contact_joined = Some(!current);
        let task = self
            .env
            .targets
            .store
            .read(cx)
            .session()
            .set_notifies_contact_joined(!current);
        crate::peer_menu::send(task, "contact joined notifications", cx);
        cx.notify();
    }

    fn type_row(&self, scope: MuteScope, cx: &mut Context<Self>) -> AnyElement {
        let (id, text, icon) = match scope {
            MuteScope::Private => (
                "notify-private",
                key::lng_notification_private_chats,
                MENU_ICON_PROFILE,
            ),
            MuteScope::Group => (
                "notify-groups",
                key::lng_notification_groups,
                MENU_ICON_GROUPS,
            ),
            MuteScope::Channel => (
                "notify-channels",
                key::lng_notification_channels,
                MENU_ICON_CHANNEL,
            ),
        };
        let found = self.scopes.get(&scope);
        let is_enabled = found.is_some_and(|found| !found.settings.is_muted());
        let details = found.map_or_else(String::new, |found| type_details(found, cx));
        let entity = cx.entity();
        let button = SettingsButton::new(id, &SETTINGS_NOTIFICATION_TYPE, tr(cx, text))
            .icon(icon)
            .toggle(is_enabled)
            .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.toggle_scope(scope, cx)));
        split_row(button, details, cx)
    }

    fn local_toggles(&self, notify: Notify, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut toggles = vec![local_toggle(
            "notify-desktop",
            tr(cx, key::lng_settings_desktop_notify),
            MENU_ICON_NOTIFICATIONS,
            notify.desktop,
            |notify| Notify {
                desktop: !notify.desktop,
                ..notify
            },
            cx,
        )];
        if cfg!(target_os = "macos") {
            toggles.push(local_toggle(
                "notify-bounce",
                tr(cx, key::lng_settings_alert_mac),
                MENU_ICON_DOCK_BOUNCE,
                notify.flash_bounce,
                |notify| Notify {
                    flash_bounce: !notify.flash_bounce,
                    ..notify
                },
                cx,
            ));
        }
        toggles.push(local_toggle(
            "notify-sound",
            tr(cx, key::lng_settings_sound_allowed),
            MENU_ICON_UNMUTE,
            notify.sound,
            |notify| Notify {
                sound: !notify.sound,
                ..notify
            },
            cx,
        ));
        toggles
    }

    fn events(&self, notify: Notify, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let entity = cx.entity();
        let joined = rows::button(
            "notify-joined",
            tr(cx, key::lng_settings_events_joined),
            MENU_ICON_INVITE,
        )
        .toggle(self.contact_joined.unwrap_or(false))
        .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.toggle_contact_joined(cx)));
        let pinned = local_toggle(
            "notify-pinned",
            tr(cx, key::lng_settings_events_pinned),
            MENU_ICON_PIN,
            notify.about_pinned,
            |notify| Notify {
                about_pinned: !notify.about_pinned,
                ..notify
            },
            cx,
        );
        vec![
            rows::subtitle(tr(cx, key::lng_settings_events_title), cx),
            joined.into_any_element(),
            pinned,
        ]
    }

    fn badge(&self, notify: Notify, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = plain_toggle(
            "notify-include-muted",
            tr(cx, key::lng_settings_include_muted),
            notify.include_muted,
            |notify| Notify {
                include_muted: !notify.include_muted,
                ..notify
            },
            cx,
        );
        let count = plain_toggle(
            "notify-count-unread",
            tr(cx, key::lng_settings_count_unread),
            notify.count_messages,
            |notify| Notify {
                count_messages: !notify.count_messages,
                ..notify
            },
            cx,
        );
        vec![
            rows::subtitle(tr(cx, key::lng_settings_badge_title), cx),
            muted,
            count,
        ]
    }
}

fn load_scope(env: &Env, scope: MuteScope, cx: &mut Context<NotificationsPage>) -> Task<()> {
    let session = env.targets.store.read(cx).session().clone();
    let settings = crate::store::spawn(session.scope_settings(scope), cx);
    let exceptions = crate::store::spawn(session.notification_exceptions(scope), cx);
    cx.spawn(async move |this, cx| {
        let settings = match settings.await {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!(%error, ?scope, "notification defaults failed");
                return;
            }
        };
        let exceptions = exceptions.await.map_or_else(
            |error| {
                tracing::warn!(%error, ?scope, "notification exceptions failed");
                0
            },
            |chats| chats.len(),
        );
        let _ = this.update(cx, |this, cx| {
            this.scopes.insert(
                scope,
                Scope {
                    settings,
                    exceptions,
                },
            );
            cx.notify();
        });
    })
}

fn load_contact_joined(env: &Env, cx: &mut Context<NotificationsPage>) -> Task<()> {
    let task = crate::store::spawn(
        env.targets
            .store
            .read(cx)
            .session()
            .notifies_contact_joined(),
        cx,
    );
    cx.spawn(async move |this, cx| match task.await {
        Ok(notify) => {
            let _ = this.update(cx, |this, cx| {
                this.contact_joined = Some(notify);
                cx.notify();
            });
        }
        Err(error) => tracing::warn!(%error, "contact joined option failed"),
    })
}

/// "Click here to change", or on/off with the exceptions count.
fn type_details(scope: &Scope, cx: &App) -> String {
    if scope.exceptions == 0 {
        return tr(cx, key::lng_notification_click_to_change).to_string();
    }
    let count = i64::try_from(scope.exceptions).unwrap_or(i64::MAX);
    let exceptions = tr_plural(cx, key::lng_notification_exceptions, count, &[]);
    let text = if scope.settings.is_muted() {
        key::lng_notification_off
    } else {
        key::lng_notification_on
    };
    tr_with(cx, text, &[("exceptions", &exceptions)])
}

/// `SetupSplitToggle`: the row with its details line and the separator
/// before the toggle.
fn split_row(button: SettingsButton, details: String, cx: &App) -> AnyElement {
    let st = &SETTINGS_NOTIFICATION_TYPE;
    let label = &SETTINGS_NOTIFICATION_TYPE_DETAILS;
    let label_height = ui::font_height(label.style.font);
    let separator_height = px_of(st.height) - px_of(st.toggle.border) * 2.0;
    let row_height = px_of(st.padding.top) + px_of(st.height) + px_of(st.padding.bottom);
    div()
        .relative()
        .child(button)
        .child(
            rows::text(details, label, cx)
                .absolute()
                .left(px_of(st.padding.left))
                .top(px_of(st.padding.top) + px_of(st.height) - label_height),
        )
        .child(
            div()
                .absolute()
                .right(px(TOGGLE_WIDTH))
                .top((row_height - separator_height) / 2.0)
                .w(px(1.0))
                .h(separator_height)
                .bg(color(cx, st.text_bg_over)),
        )
        .into_any_element()
}

/// A toggle row for a setting kept in settings.json.
fn local_toggle(
    id: &'static str,
    text: gpui::SharedString,
    icon: style::Icon,
    is_on: bool,
    flip: fn(Notify) -> Notify,
    cx: &mut Context<NotificationsPage>,
) -> AnyElement {
    let entity = cx.entity();
    rows::button(id, text, icon)
        .toggle(is_on)
        .on_click(move |_, _, cx| {
            settings::set_notify(flip(settings::notify(cx)), cx);
            entity.update(cx, |_, cx| cx.notify());
        })
        .into_any_element()
}

fn plain_toggle(
    id: &'static str,
    text: gpui::SharedString,
    is_on: bool,
    flip: fn(Notify) -> Notify,
    cx: &mut Context<NotificationsPage>,
) -> AnyElement {
    let entity = cx.entity();
    SettingsButton::new(id, &SETTINGS_BUTTON_NO_ICON, text)
        .toggle(is_on)
        .on_click(move |_, _, cx| {
            settings::set_notify(flip(settings::notify(cx)), cx);
            entity.update(cx, |_, cx| cx.notify());
        })
        .into_any_element()
}

/// The sender name and message preview checkboxes (`SetupNotifyViewOptions`):
/// unticking the name unticks the preview, ticking the preview ticks the name.
fn view_options(notify: Notify, cx: &mut Context<NotificationsPage>) -> AnyElement {
    let is_name = notify.view != NotifyView::Nothing;
    let is_preview = notify.view == NotifyView::NameAndPreview;
    let checkbox = |id: &'static str,
                    text,
                    is_checked: bool,
                    next: NotifyView,
                    cx: &mut Context<NotificationsPage>| {
        let entity = cx.entity();
        let checked = if is_checked {
            Checked::Yes
        } else {
            Checked::No
        };
        Checkbox::new(
            id,
            &DEFAULT_BOX_CHECKBOX,
            &DEFAULT_CHECK,
            tr(cx, text),
            checked,
        )
        .on_toggle(move |_, cx| {
            let notify = settings::notify(cx);
            settings::set_notify(
                Notify {
                    view: next,
                    ..notify
                },
                cx,
            );
            entity.update(cx, |_, cx| cx.notify());
        })
    };
    let name_next = if is_name {
        NotifyView::Nothing
    } else {
        NotifyView::NameOnly
    };
    let preview_next = if is_preview {
        NotifyView::NameOnly
    } else {
        NotifyView::NameAndPreview
    };
    div()
        .flex()
        .justify_center()
        .gap(px_of(NOTIFY_PREVIEW_CHECKS_SKIP))
        .py(px_of(NOTIFY_PREVIEW_CHECKS_SKIP))
        .child(checkbox(
            "notify-show-name",
            key::lng_notification_show_name,
            is_name,
            name_next,
            cx,
        ))
        .child(checkbox(
            "notify-show-text",
            key::lng_notification_show_text,
            is_preview,
            preview_next,
            cx,
        ))
        .into_any_element()
}

impl Render for NotificationsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notify = settings::notify(cx);
        let types: Vec<_> = SCOPES
            .iter()
            .map(|scope| self.type_row(*scope, cx))
            .collect();
        div()
            .flex()
            .flex_col()
            .child(rows::subtitle(tr(cx, key::lng_settings_notify_global), cx))
            .children(self.local_toggles(notify, cx))
            .when(notify.desktop, |this| this.child(view_options(notify, cx)))
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_settings_notify_title), cx))
            .children(types)
            .child(rows::skip())
            .child(rows::divider(cx))
            .child(rows::skip())
            .children(self.events(notify, cx))
            .child(rows::skip())
            .child(rows::divider(cx))
            .child(rows::skip())
            .children(self.badge(notify, cx))
            .child(rows::skip())
    }
}
