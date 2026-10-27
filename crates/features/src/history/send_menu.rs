//! The send button's menu (`SendMenu::FillSendMenu`): send without sound,
//! schedule (a reminder in Saved Messages), and send when online.

use std::rc::Rc;

use gpui::{Anchor, App, AppContext as _, Context, Pixels, Point, Window};
use i18n::key;
use model::chat::ChatKind;
use model::message::Schedule;
use model::send::{SendOptions, Sound};
use model::user::{UserKind, UserStatus};
use style::menu_icons::{MENU_ICON_MUTE, MENU_ICON_SCHEDULE, MENU_ICON_WHEN_ONLINE};
use style::widgets::POPUP_MENU_WITH_ICONS;
use ui::{MenuTone, PopupMenu};

use super::HistoryView;
use super::schedule_box;
use crate::lang::tr;

/// Telegram's service notifications account (`isNotificationsUser`).
const NOTIFICATIONS_USER: i64 = 777_000;

/// What the menu offers (`SendMenu::Type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SendMenu {
    Scheduled,
    /// Also "Send when online", to a user whose last seen time shows.
    ScheduledToUser,
    /// Saved Messages: "Set a reminder" and no silent sending.
    Reminder,
}

/// Sends with the options the menu or the schedule box chose.
pub(super) type OnOptions = Rc<dyn Fn(SendOptions, &mut Window, &mut App)>;

impl HistoryView {
    /// The menu this chat's send button has, if any
    /// (`HistoryWidget::sendMenuDetails`).
    // shortcut: no SilentOnly, as paid messages are not read yet; it comes with them.
    pub(super) fn send_menu(&self, cx: &App) -> Option<SendMenu> {
        let state = self.store.read(cx).state();
        let chat = state.chats.chat(self.chat)?;
        let user = match chat.kind {
            // Secret chats cannot schedule.
            ChatKind::Secret { .. } => return None,
            ChatKind::Private { user } => state.chats.user(user),
            ChatKind::Group { .. } | ChatKind::Supergroup { .. } => None,
        };
        if chat.user().is_some() && chat.user() == state.me {
            return Some(SendMenu::Reminder);
        }
        let until_online = user.is_some_and(|user| {
            user.kind != UserKind::Bot
                && user.id.0 != NOTIFICATIONS_USER
                && matches!(
                    user.status,
                    UserStatus::Online { .. } | UserStatus::Offline { .. }
                )
        });
        Some(if until_online {
            SendMenu::ScheduledToUser
        } else {
            SendMenu::Scheduled
        })
    }

    /// Opens the send menu at `position` (a right click on the send
    /// button); `on_send` sends with what it chose.
    pub(super) fn show_send_menu(
        &mut self,
        on_send: OnOptions,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(kind) = self.send_menu(cx) else {
            return;
        };
        let layers = self.layers.clone();
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            if kind != SendMenu::Reminder {
                let on_send = on_send.clone();
                menu.action(
                    tr(cx, key::lng_send_silent_message),
                    MENU_ICON_MUTE,
                    MenuTone::Normal,
                    move |window, cx| {
                        let silent = SendOptions {
                            sound: Sound::Silent,
                            schedule: None,
                        };
                        on_send(silent, window, cx);
                    },
                );
            }
            let schedule = match kind {
                SendMenu::Reminder => key::lng_reminder_message,
                SendMenu::Scheduled | SendMenu::ScheduledToUser => key::lng_schedule_message,
            };
            let scheduled = on_send.clone();
            menu.action(
                tr(cx, schedule),
                MENU_ICON_SCHEDULE,
                MenuTone::Normal,
                move |window, cx| schedule_box::show(&layers, kind, scheduled.clone(), window, cx),
            );
            if kind == SendMenu::ScheduledToUser {
                menu.action(
                    tr(cx, key::lng_scheduled_send_until_online),
                    MENU_ICON_WHEN_ONLINE,
                    MenuTone::Normal,
                    move |window, cx| on_send(WHEN_ONLINE, window, cx),
                );
            }
            menu
        });
        self.present_menu(menu, position, Anchor::BottomRight, window, cx);
    }
}

pub(super) const WHEN_ONLINE: SendOptions = SendOptions {
    sound: Sound::Normal,
    schedule: Some(Schedule::WhenOnline),
};
