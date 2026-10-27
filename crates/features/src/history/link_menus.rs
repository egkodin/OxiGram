//! The menus a phone number, a bank card or a date in a message opens
//! (`PhoneClickHandler`, `BankCardClickHandler`,
//! `FormattedDateClickHandler`).

use chrono::Local;
use gpui::{Anchor, AppContext as _, ClipboardItem, Context, Pixels, Point, Window};
use i18n::key;
use model::RequestError;
use model::UserId;
use model::text::{DateFormat, Precision};
use model::text_links::BankCard;
use style::menu_icons::{MENU_ICON_COPY, MENU_ICON_PAYMENT, MENU_ICON_PROFILE};
use style::widgets::{DEFAULT_TOAST, POPUP_MENU_WITH_ICONS};
use ui::{MenuTone, PopupMenu};

use super::service::user_name;
use super::{HistoryEvent, HistoryView, Link, dates, menu};
use crate::lang::{strings, tr};
use crate::store::spawn;

impl HistoryView {
    /// Looks the number up, then opens its menu at the pointer.
    // shortcut: the menu opens once TDLib answers instead of showing "Loading..." inside it.
    pub(super) fn open_phone_menu(
        &mut self,
        phone: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = spawn(
            self.store.read(cx).session().user_by_phone(phone.trim()),
            cx,
        );
        let position = window.mouse_position();
        cx.spawn_in(window, async move |this, cx| {
            let user = task.await;
            this.update_in(cx, |this, window, cx| {
                this.show_phone_menu(phone.trim(), user, position, window, cx);
            })
        })
        .detach();
    }

    // shortcut: no "Add to contacts" until the add-contact box (M6).
    fn show_phone_menu(
        &mut self,
        phone: &str,
        user: Result<Option<UserId>, RequestError>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = match &user {
            Ok(Some(user)) => Some((
                *user,
                user_name(strings(cx), &self.store.read(cx).state().chats, *user),
            )),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error, "looking up a phone number failed");
                None
            }
        };
        let (copy, not_found) = (
            tr(cx, key::lng_profile_copy_phone),
            tr(cx, key::lng_menu_not_contact),
        );
        let failed = tr(cx, key::lng_attach_failed);
        let (phone, weak) = (phone.to_owned(), cx.weak_entity());
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            menu.action(copy, MENU_ICON_COPY, MenuTone::Normal, move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(phone.clone()));
            });
            menu.separator();
            match (name, user.is_ok()) {
                // shortcut: opens the chat with them; tdesktop opens the profile (M6) and shows their userpic over "View profile".
                (Some((user, name)), _) => {
                    menu.action(name, MENU_ICON_PROFILE, MenuTone::Normal, move |_, cx| {
                        let _ =
                            weak.update(cx, |_, cx| cx.emit(HistoryEvent::Link(Link::User(user))));
                    })
                }
                (None, true) => menu.note(not_found),
                (None, false) => menu.note(failed),
            }
            menu
        });
        self.present_menu(menu, position, Anchor::TopLeft, window, cx);
    }

    /// Asks TDLib for the card's bank, then opens its menu at the pointer.
    pub(super) fn open_bank_card_menu(
        &mut self,
        number: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = spawn(self.store.read(cx).session().bank_card(&number), cx);
        let position = window.mouse_position();
        cx.spawn_in(window, async move |this, cx| {
            let card = task.await;
            this.update_in(cx, |this, window, cx| {
                this.show_bank_card_menu(number, card, position, window, cx);
            })
        })
        .detach();
    }

    fn show_bank_card_menu(
        &mut self,
        number: String,
        card: Result<BankCard, RequestError>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (copy, copied) = (
            tr(cx, key::lng_context_bank_card_copy),
            tr(cx, key::lng_context_bank_card_copied),
        );
        let failed = tr(cx, key::lng_attach_failed);
        let layers = self.layers.clone();
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            menu.action(copy, MENU_ICON_COPY, MenuTone::Normal, move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(number.clone()));
                let copied = copied.clone();
                layers.update(cx, |layers, cx| {
                    layers.show_toast(copied, &DEFAULT_TOAST, menu::TOAST_DURATION, cx);
                });
            });
            menu.separator();
            match card {
                Ok(card) => {
                    for action in card.actions {
                        menu.action(
                            action.text,
                            MENU_ICON_PAYMENT,
                            MenuTone::Normal,
                            move |_, cx| {
                                cx.open_url(&action.url);
                            },
                        );
                    }
                    // shortcut: the bank's name is an inert line; tdesktop copies the number on its click too.
                    if !card.title.is_empty() {
                        menu.note(card.title);
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "looking up a bank card failed");
                    menu.note(failed);
                }
            }
            menu
        });
        self.present_menu(menu, position, Anchor::TopLeft, window, cx);
    }

    /// "Copy Date" at the pointer (`FormattedDateClickHandler`).
    // shortcut: no "Add to Calendar" (an .ics export, M7) or "Set a Reminder" (scheduled sending, M4).
    pub(super) fn show_date_menu(
        &mut self,
        unix_time: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let long = DateFormat::Absolute {
            time: Precision::Long,
            date: Precision::Long,
            has_day_of_week: true,
        };
        let date = dates::format(strings(cx), unix_time, long, &Local::now());
        let (copy, copied) = (
            tr(cx, key::lng_context_copy_date),
            tr(cx, key::lng_date_copied),
        );
        let layers = self.layers.clone();
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            menu.action(copy, MENU_ICON_COPY, MenuTone::Normal, move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(date.clone()));
                let copied = copied.clone();
                layers.update(cx, |layers, cx| {
                    layers.show_toast(copied, &DEFAULT_TOAST, menu::TOAST_DURATION, cx);
                });
            });
            menu
        });
        self.present_menu(menu, window.mouse_position(), Anchor::TopLeft, window, cx);
    }
}
