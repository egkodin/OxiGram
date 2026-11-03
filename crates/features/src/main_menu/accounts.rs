//! The main menu's accounts (`MainMenu::ToggleAccountsButton` and
//! `Settings::AccountsList`): the arrow in the cover with the other
//! accounts' unread badge, then each account with its userpic and badge,
//! and "Add Account".

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, DismissEvent, Entity, Focusable as _, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement, PathBuilder,
    Pixels, Point, StatefulInteractiveElement as _, Styled, Window, anchored, canvas, deferred,
    div, point, prelude::FluentBuilder as _, px,
};
use i18n::key;
use style::menu_icons::{MENU_ICON_COPY, MENU_ICON_LEAVE_ATTENTION, MENU_ICON_PROFILE};
use style::settings::SETTINGS_ICON_ADD;
use style::widgets::POPUP_MENU_WITH_ICONS;
use style::window::{
    MAIN_MENU_ACCOUNT_LINE, MAIN_MENU_ACCOUNT_SIZE, MAIN_MENU_ADD_ACCOUNT_BUTTON,
    MAIN_MENU_BADGE_FONT, MAIN_MENU_BADGE_SIZE, MAIN_MENU_TOGGLE_FOUR_STROKES,
};
use style::window_main_menu::{
    MAIN_MENU_COVER_HEIGHT, MAIN_MENU_COVER_NAME_TOP, MAIN_MENU_SKIP, MAIN_MENU_TOGGLE_POSITION,
    MAIN_MENU_TOGGLE_SIZE, MAIN_MENU_WIDTH,
};
use style::{ColorId, ColorRef};
use ui::{Markup, MenuTone, PopupMenu, SettingsButton, Userpic, color, font, icon, icon_size};

use super::{MainMenu, me};
use crate::accounts::{self, Account, AccountId, Accounts};
use crate::boxes::{self, Callback, ConfirmArgs};
use crate::lang::{tr, tr_plural};
use crate::{Store, px_of, settings};

/// Keeps the account menu off the window edges.
const EDGE_MARGIN: f32 = 8.0;
/// The badge's side padding (`UnreadBadgeStyle::padding`).
const BADGE_PADDING: f32 = 5.0;

impl MainMenu {
    /// The arrow over the cover's lower part that opens the account list;
    /// none without a domain (the snapshots of one account).
    pub(super) fn accounts_toggle(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let accounts = self.accounts.as_ref()?;
        let is_shown = settings::is_main_menu_accounts_shown(cx);
        let others = accounts.read(cx).others_unread();
        let fg = color(cx, ColorRef::Palette(ColorId::WindowSubTextFg));
        let height = px_of(MAIN_MENU_COVER_HEIGHT) - px_of(MAIN_MENU_COVER_NAME_TOP);
        let toggle = px_of(MAIN_MENU_TOGGLE_SIZE);
        let center = point(
            px_of(MAIN_MENU_WIDTH) - px_of(MAIN_MENU_TOGGLE_POSITION.x),
            height - px_of(MAIN_MENU_TOGGLE_POSITION.y),
        );
        // `paintUnreadBadge`: left of the arrow while the list is closed.
        let badge = (!is_shown && others.count > 0 && !others.is_all_muted).then(|| {
            div()
                .absolute()
                .right(px_of(MAIN_MENU_TOGGLE_POSITION.x) + toggle * 3.0)
                .top(center.y - px_of(MAIN_MENU_BADGE_SIZE) / 2.0)
                .child(badge(others.count, false, cx))
        });
        let element = div()
            .id("main-menu-accounts-toggle")
            .absolute()
            .left_0()
            .top(px_of(MAIN_MENU_COVER_NAME_TOP))
            .w_full()
            .h(height)
            .cursor_pointer()
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        paint_arrow(bounds.origin + center, is_shown, fg, window);
                    },
                )
                .size_full(),
            )
            .children(badge)
            .on_click(move |_, _, cx| settings::set_main_menu_accounts_shown(!is_shown, cx));
        Some(element.into_any_element())
    }

    /// The accounts and "Add Account" under the cover while the list is
    /// open, then the shadow line above the menu.
    pub(super) fn accounts_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let accounts = self.accounts.clone()?;
        if !settings::is_main_menu_accounts_shown(cx) {
            return None;
        }
        let domain = accounts.read(cx);
        let active = domain.active_id();
        let can_add = domain.can_add();
        let list: Vec<Account> = domain
            .list()
            .iter()
            .filter(|account| accounts::is_signed_in(&account.store, cx))
            .cloned()
            .collect();
        // `AccountsList::rebuild`: no rows for a lone account.
        let rows: Vec<AnyElement> = if list.len() > 1 {
            list.iter()
                .map(|account| self.account_row(account, account.id == active, cx))
                .collect()
        } else {
            Vec::new()
        };
        let shadow = color(cx, ColorRef::Palette(ColorId::ShadowFg));
        let element = div()
            .flex_none()
            .pt(px_of(MAIN_MENU_SKIP))
            .children(rows)
            .children(can_add.then(|| self.add_account_row(cx)))
            .child(div().h(px_of(MAIN_MENU_SKIP)))
            .child(div().h(px(1.0)).bg(shadow));
        Some(element.into_any_element())
    }

    fn account_row(
        &self,
        account: &Account,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (name, userpic) = me(account.store.read(cx).state());
        let id = account.id;
        let entity = cx.entity();
        let button = SettingsButton::new(
            ("main-menu-account", id.0 as usize),
            &MAIN_MENU_ADD_ACCOUNT_BUTTON,
            name,
        )
        .on_click({
            let entity = entity.clone();
            move |_, _, cx| entity.update(cx, |this, cx| this.choose_account(id, cx))
        });
        let (count, is_muted) = accounts::unread(&account.store, cx);
        let badge = (!is_active && count > 0).then(|| {
            div()
                .absolute()
                .right(px_of(MAIN_MENU_ADD_ACCOUNT_BUTTON.padding.right))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .child(badge(count, is_muted, cx))
        });
        div()
            .relative()
            .child(button)
            .child(account_userpic(userpic, is_active, cx))
            .children(badge)
            .on_mouse_down(
                MouseButton::Right,
                move |event: &MouseDownEvent, window, cx| {
                    let position = event.position;
                    entity.update(cx, |this, cx| {
                        this.show_account_menu(id, position, window, cx)
                    });
                },
            )
            .into_any_element()
    }

    fn add_account_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity();
        let size = icon_size(SETTINGS_ICON_ADD);
        let button = SettingsButton::new(
            "main-menu-add-account",
            &MAIN_MENU_ADD_ACCOUNT_BUTTON,
            tr(cx, key::lng_menu_add_account),
        )
        .on_click(move |_, window, cx| {
            entity.update(cx, |this, cx| this.add_account(window, cx));
        });
        // `IconType::Round`: the plus on a windowBgActive circle.
        let height = button_height();
        div()
            .relative()
            .child(button)
            .child(
                div()
                    .absolute()
                    .left(px_of(MAIN_MENU_ADD_ACCOUNT_BUTTON.icon_left))
                    .top((height - size.height) / 2.0)
                    .size(size.width)
                    .rounded_full()
                    .bg(color(cx, ColorRef::Palette(ColorId::WindowBgActive)))
                    .child(icon(SETTINGS_ICON_ADD)),
            )
            .into_any_element()
    }

    /// A click on an account: the shown one closes the menu, another one
    /// shows instead.
    fn choose_account(&mut self, id: AccountId, cx: &mut Context<Self>) {
        let Some(accounts) = &self.accounts else {
            return;
        };
        if accounts.read(cx).active_id() == id {
            cx.emit(super::MenuAction::Close);
            return;
        }
        accounts.update(cx, |accounts, cx| accounts.activate(id, cx));
    }

    fn add_account(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(accounts) = self.accounts.clone() else {
            return;
        };
        let count = accounts.read(cx).list().len();
        if accounts
            .update(cx, |accounts, cx| accounts.add(cx))
            .is_err()
        {
            self.show_limit(count, window, cx);
        }
    }

    /// `AccountsLimitBox`.
    // shortcut: the title, the count and OK; no limit bubble or offer of Premium for one of the accounts, until Premium screens exist.
    fn show_limit(&self, count: usize, window: &mut Window, cx: &mut Context<Self>) {
        let count = i64::try_from(count).unwrap_or(i64::MAX);
        let mut args = ConfirmArgs::new(Markup::rich(&tr_plural(
            cx,
            key::lng_accounts_limit1,
            count,
            &[],
        )));
        args.title = Some(tr(cx, key::lng_accounts_limit_title));
        args.confirm_text = Some(tr(cx, key::lng_box_ok));
        boxes::confirm(&self.layers, args, window, cx);
    }

    /// `MakeAccountButton`'s context menu.
    // shortcut: no "Open in new window": rgram has one window.
    fn show_account_menu(
        &mut self,
        id: AccountId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(accounts) = self.accounts.clone() else {
            return;
        };
        let Some(store) = account_store(&accounts, id, cx) else {
            return;
        };
        let is_active = accounts.read(cx).active_id() == id;
        let layers = self.layers.clone();
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            fill_account_menu(&mut menu, &accounts, &store, id, is_active, &layers, cx);
            menu
        });
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.menu = Some((position, menu, subscription));
        cx.notify();
    }

    pub(super) fn menu_overlay(&self) -> Option<AnyElement> {
        let (position, menu, _) = self.menu.as_ref()?;
        Some(
            deferred(
                anchored()
                    .position(*position)
                    .snap_to_window_with_margin(px(EDGE_MARGIN))
                    .child(div().child(menu.clone())),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

fn account_store(accounts: &Entity<Accounts>, id: AccountId, cx: &App) -> Option<Entity<Store>> {
    accounts
        .read(cx)
        .list()
        .iter()
        .find(|account| account.id == id)
        .map(|account| account.store.clone())
}

fn fill_account_menu(
    menu: &mut PopupMenu,
    accounts: &Entity<Accounts>,
    store: &Entity<Store>,
    id: AccountId,
    is_active: bool,
    layers: &Entity<crate::Layers>,
    cx: &App,
) {
    let phone = {
        let state = store.read(cx).state();
        state
            .me
            .and_then(|me| state.chats.user(me))
            .map(|user| user.phone.expose().to_owned())
            .unwrap_or_default()
    };
    menu.action(
        tr(cx, key::lng_profile_copy_phone),
        MENU_ICON_COPY,
        MenuTone::Normal,
        move |_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(format!("+{phone}"))),
    );
    if !is_active {
        let accounts = accounts.clone();
        menu.action(
            tr(cx, key::lng_menu_activate),
            MENU_ICON_PROFILE,
            MenuTone::Normal,
            move |_, cx| accounts.update(cx, |accounts, cx| accounts.activate(id, cx)),
        );
    }
    crate::dialogs::add_mark_read(menu, store, layers, model::chat::ChatList::Main, cx);
    if is_active {
        return;
    }
    let store = store.clone();
    let layers = layers.clone();
    menu.action(
        tr(cx, key::lng_settings_logout),
        MENU_ICON_LEAVE_ATTENTION,
        MenuTone::Attention,
        move |window, cx| {
            let store = store.clone();
            let log_out: Callback = Rc::new(move |_, cx| {
                let task = store.read(cx).auth(model::auth::Action::LogOut, cx);
                cx.spawn(async move |_| {
                    if let Err(error) = task.await {
                        tracing::warn!(%error, "log out failed");
                    }
                })
                .detach();
            });
            boxes::confirm_attention(
                &layers,
                tr(cx, key::lng_sure_logout).to_string(),
                tr(cx, key::lng_settings_logout),
                log_out,
                window,
                cx,
            );
        },
    );
}

fn button_height() -> Pixels {
    let st = &MAIN_MENU_ADD_ACCOUNT_BUTTON;
    px_of(st.padding.top) + px_of(st.height) + px_of(st.padding.bottom)
}

/// The account's userpic centred where "Add Account" has its plus; the
/// shown account's has a windowBgActive ring.
fn account_userpic(userpic: ui::UserpicSource, is_active: bool, cx: &App) -> AnyElement {
    let line = px_of(MAIN_MENU_ACCOUNT_LINE);
    let skip = line * 2.0 + px(1.0);
    let size = px_of(MAIN_MENU_ACCOUNT_SIZE);
    let full = size + skip * 2.0;
    let left = px_of(MAIN_MENU_ADD_ACCOUNT_BUTTON.icon_left)
        + (icon_size(SETTINGS_ICON_ADD).width - full) / 2.0;
    let ring = color(cx, ColorRef::Palette(ColorId::WindowBgActive));
    div()
        .absolute()
        .left(left)
        .top((button_height() - full) / 2.0)
        .size(full)
        .flex()
        .items_center()
        .justify_center()
        .when(is_active, |this| {
            this.child(
                div()
                    .absolute()
                    .inset(px(1.0))
                    .rounded_full()
                    .border(line)
                    .border_color(ring),
            )
        })
        .child(Userpic::new(size, userpic))
        .into_any_element()
}

/// `Ui::PaintUnreadBadge` in the main menu's size.
fn badge(count: i32, is_muted: bool, cx: &App) -> AnyElement {
    let (bg, fg) = (
        if is_muted {
            ColorId::DialogsUnreadBgMuted
        } else {
            ColorId::DialogsUnreadBg
        },
        ColorId::DialogsUnreadFg,
    );
    let height = px_of(MAIN_MENU_BADGE_SIZE);
    div()
        .h(height)
        .min_w(height)
        .px(px(BADGE_PADDING))
        .rounded(height / 2.0)
        .bg(color(cx, ColorRef::Palette(bg)))
        .flex()
        .items_center()
        .justify_center()
        .font(font(MAIN_MENU_BADGE_FONT))
        .text_size(px_of(MAIN_MENU_BADGE_FONT.size))
        .text_color(color(cx, ColorRef::Palette(fg)))
        .child(crate::history::short_count(count))
        .into_any_element()
}

/// `Ui::ToggleUpDownArrowPath` at rest: a chevron pointing down while the
/// list is closed, up while it is open.
// shortcut: no turning animation between the two (`slideWrapDuration`).
fn paint_arrow(center: Point<Pixels>, is_up: bool, fg: Hsla, window: &mut Window) {
    let size = f32::from(px_of(MAIN_MENU_TOGGLE_SIZE));
    let stroke = (f32::from(px_of(MAIN_MENU_TOGGLE_FOUR_STROKES)) / 4.0) / std::f32::consts::SQRT_2;
    let half = size / 2.0;
    // The up chevron around (0, 0); turned half a circle it points down.
    let points = [
        (-size - stroke, half - stroke),
        (0.0, half - stroke - size - stroke),
        (size + stroke, half - stroke),
        (size - stroke, half + stroke),
        (0.0, half + stroke - size + stroke),
        (-size + stroke, half + stroke),
    ];
    let sign = if is_up { 1.0 } else { -1.0 };
    let corners: Vec<Point<Pixels>> = points
        .iter()
        .map(|(x, y)| center + point(px(x * sign), px(y * sign)))
        .collect();
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&corners, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, fg);
    }
}
