//! The main menu (`Window::MainMenu`): the account cover, the accounts,
//! the section buttons, the night mode switch and the version, sliding in
//! from the left.

mod accounts;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, MouseButton,
    ParentElement, Pixels, Point, Render, SharedString, Styled, Subscription, Window, div,
    prelude::*, px,
};
use i18n::key;
use model::State;
use model::chat::ChatList;
use style::menu_icons::{
    MENU_ICON_ARCHIVE_OPEN, MENU_ICON_CHANNEL, MENU_ICON_GROUPS, MENU_ICON_NIGHT_MODE,
    MENU_ICON_PHONE, MENU_ICON_PROFILE, MENU_ICON_SAVED_MESSAGES, MENU_ICON_SETTINGS,
    MENU_ICON_USER_SHOW,
};
use style::window::{MAIN_MENU_BUTTON, MAIN_MENU_TELEGRAM_LABEL};
use style::window_main_menu::{
    MAIN_MENU_COVER_HEIGHT, MAIN_MENU_COVER_NAME_LEFT, MAIN_MENU_COVER_NAME_TOP,
    MAIN_MENU_FOOTER_HEIGHT_MIN, MAIN_MENU_FOOTER_LEFT, MAIN_MENU_SKIP, MAIN_MENU_TELEGRAM_BOTTOM,
    MAIN_MENU_USERPIC_LEFT, MAIN_MENU_USERPIC_TOP, MAIN_MENU_VERSION_BOTTOM, MAIN_MENU_WIDTH,
};
use style::{ColorId, ColorRef};
use ui::{PopupMenu, SettingsButton, Userpic, UserpicSource, color, font, font_height};

use crate::accounts::Accounts;
use crate::lang::{tr, tr_with};
use crate::{APP_NAME, Layers, Store, px_of, settings};

/// The userpic in the cover (`mainMenuUserpic`).
const USERPIC_SIZE: f32 = 48.0;

/// A menu button the main view acts on; the menu closes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Close,
    Archive,
    /// Puts the archive back at the top of the chat list.
    ArchiveToList,
    Contacts,
    Calls,
    MyProfile,
    NewGroup,
    NewChannel,
    SavedMessages,
    Settings,
}

pub(crate) struct MainMenu {
    store: Entity<Store>,
    layers: Entity<Layers>,
    /// None in the snapshots of one account.
    accounts: Option<Entity<Accounts>>,
    /// An account's context menu and where it opened.
    menu: Option<(Point<Pixels>, Entity<PopupMenu>, Subscription)>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for MainMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainMenu").finish_non_exhaustive()
    }
}

impl EventEmitter<MenuAction> for MainMenu {}

impl Focusable for MainMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl MainMenu {
    pub(crate) fn new(
        store: Entity<Store>,
        layers: Entity<Layers>,
        cx: &mut Context<Self>,
    ) -> Self {
        let accounts = crate::accounts::domain(cx);
        let mut subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe_global::<settings::Settings>(|_, cx| cx.notify()),
        ];
        subscriptions.extend(
            accounts
                .as_ref()
                .map(|accounts| cx.observe(accounts, |_, _, cx| cx.notify())),
        );
        Self {
            store,
            layers,
            accounts,
            menu: None,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    fn cover(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = self.accounts_toggle(cx);
        let state = self.store.read(cx).state();
        let (name, userpic) = me(state);
        let name_font = MAIN_MENU_BUTTON.style.font;
        div()
            .relative()
            .flex_none()
            .h(px_of(MAIN_MENU_COVER_HEIGHT))
            .child(
                div()
                    .absolute()
                    .left(px_of(MAIN_MENU_USERPIC_LEFT))
                    .top(px_of(MAIN_MENU_USERPIC_TOP))
                    .child(Userpic::new(px(USERPIC_SIZE), userpic)),
            )
            .child(
                div()
                    .absolute()
                    .left(px_of(MAIN_MENU_COVER_NAME_LEFT))
                    .right(px_of(MAIN_MENU_COVER_NAME_LEFT))
                    .top(px_of(MAIN_MENU_COVER_NAME_TOP))
                    .truncate()
                    .font(font(name_font))
                    .text_size(px_of(name_font.size))
                    .line_height(font_height(name_font))
                    .text_color(color(cx, ColorRef::Palette(ColorId::WindowBoldFg)))
                    .child(name),
            )
            .children(toggle)
    }

    fn button(
        &self,
        id: &'static str,
        text: SharedString,
        icon: style::Icon,
        action: MenuAction,
        cx: &mut Context<Self>,
    ) -> SettingsButton {
        let entity = cx.entity();
        SettingsButton::new(id, &MAIN_MENU_BUTTON, text)
            .icon(icon)
            .on_click(move |_, _, cx| {
                entity.update(cx, |_, cx| cx.emit(action));
            })
    }

    fn footer(cx: &App) -> impl IntoElement {
        let label = MAIN_MENU_TELEGRAM_LABEL;
        let text_color = color(cx, label.text_fg);
        let version = tr_with(
            cx,
            key::lng_settings_current_version,
            &[("version", env!("CARGO_PKG_VERSION"))],
        );
        let line = |text: SharedString, style: style::Font, bottom: style::Px| {
            div()
                .absolute()
                .left(px_of(MAIN_MENU_FOOTER_LEFT))
                .bottom(px_of(bottom))
                .font(font(style))
                .text_size(px_of(style.size))
                .line_height(font_height(style))
                .text_color(text_color)
                .child(text)
        };
        div()
            .relative()
            .flex_none()
            .h(px_of(MAIN_MENU_FOOTER_HEIGHT_MIN))
            .child(line(
                APP_NAME.into(),
                label.style.font,
                MAIN_MENU_TELEGRAM_BOTTOM,
            ))
            .child(line(
                version.into(),
                style::window::MAIN_MENU_VERSION_LABEL.style.font,
                MAIN_MENU_VERSION_BOTTOM,
            ))
    }
}

/// The signed-in user's name and userpic.
fn me(state: &State) -> (SharedString, UserpicSource) {
    let Some(user) = state.me.and_then(|me| state.chats.user(me)) else {
        return (
            SharedString::default(),
            UserpicSource::Empty {
                color_index: 0,
                name: SharedString::default(),
            },
        );
    };
    let name: SharedString = user.name().into();
    let file = user
        .photo
        .as_ref()
        .and_then(|photo| state.chats.file(photo.small.id))
        .and_then(|file| file.path.clone());
    let source = match (
        file,
        user.photo
            .as_ref()
            .and_then(|photo| photo.minithumbnail.as_ref()),
    ) {
        (Some(path), _) => UserpicSource::File(path.into()),
        (None, Some(thumbnail)) => UserpicSource::Jpeg(ui::jpeg_image(thumbnail.jpeg.clone())),
        (None, None) => UserpicSource::Empty {
            color_index: user.accent_color,
            name: name.clone(),
        },
    };
    (name, source)
}

impl Render for MainMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let night = settings::is_night_mode(cx);
        let layers = self.layers.clone();
        let shadow = color(cx, ColorRef::Palette(ColorId::ShadowFg));
        let separator = move || div().my(px_of(MAIN_MENU_SKIP)).h(px(1.0)).bg(shadow);
        let state = self.store.read(cx).state();
        let shows_archive =
            settings::is_archive_in_main_menu(cx) && state.chats.list_len(ChatList::Archive) > 0;
        let entity = cx.entity();
        // shortcut: a right click returns the archive at once; tdesktop opens the archive menu there.
        let archive = shows_archive.then(|| {
            div()
                .child(self.button(
                    "archive",
                    tr(cx, key::lng_archived_name),
                    MENU_ICON_ARCHIVE_OPEN,
                    MenuAction::Archive,
                    cx,
                ))
                .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                    entity.update(cx, |_, cx| cx.emit(MenuAction::ArchiveToList));
                })
                .child(separator())
        });
        div()
            .track_focus(&self.focus)
            .w(px_of(MAIN_MENU_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(color(cx, ColorRef::Palette(ColorId::MainMenuBg)))
            .child(self.cover(cx))
            .children(self.accounts_section(cx))
            .child(
                div()
                    .id("main-menu-items")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(archive)
                    .child(self.button(
                        "my-profile",
                        tr(cx, key::lng_menu_my_profile),
                        MENU_ICON_PROFILE,
                        MenuAction::MyProfile,
                        cx,
                    ))
                    .child(separator())
                    .child(self.button(
                        "new-group",
                        tr(cx, key::lng_create_group_title),
                        MENU_ICON_GROUPS,
                        MenuAction::NewGroup,
                        cx,
                    ))
                    .child(self.button(
                        "new-channel",
                        tr(cx, key::lng_create_channel_title),
                        MENU_ICON_CHANNEL,
                        MenuAction::NewChannel,
                        cx,
                    ))
                    .child(self.button(
                        "contacts",
                        tr(cx, key::lng_menu_contacts),
                        MENU_ICON_USER_SHOW,
                        MenuAction::Contacts,
                        cx,
                    ))
                    .child(self.button(
                        "calls",
                        tr(cx, key::lng_menu_calls),
                        MENU_ICON_PHONE,
                        MenuAction::Calls,
                        cx,
                    ))
                    .child(self.button(
                        "saved-messages",
                        tr(cx, key::lng_saved_messages),
                        MENU_ICON_SAVED_MESSAGES,
                        MenuAction::SavedMessages,
                        cx,
                    ))
                    .child(self.button(
                        "settings",
                        tr(cx, key::lng_menu_settings),
                        MENU_ICON_SETTINGS,
                        MenuAction::Settings,
                        cx,
                    ))
                    .child(
                        SettingsButton::new(
                            "night-mode",
                            &MAIN_MENU_BUTTON,
                            tr(cx, key::lng_menu_night_mode),
                        )
                        .icon(MENU_ICON_NIGHT_MODE)
                        .toggle(night)
                        .on_click(move |_, window, cx| {
                            let toggle: crate::boxes::Callback =
                                std::rc::Rc::new(move |_, cx| settings::set_night_mode(!night, cx));
                            crate::settings_screen::toggle_night_mode(&layers, toggle, window, cx);
                        }),
                    ),
            )
            .child(Self::footer(cx))
            .children(self.menu_overlay())
    }
}
