//! The main Settings page (`Settings::Main`): the account's cover, the
//! sections, Premium and the help links (`BuildSectionButtons`,
//! `BuildPremiumSection`, `BuildHelpSection`).

use std::collections::HashSet;

use gpui::{
    App, ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div, prelude::*,
};
use i18n::key;
use model::{ChatId, FileId, UserId};
use style::info::{
    INFO_PROFILE_COVER, INFO_PROFILE_LABELED_BUTTON_QR, INFO_PROFILE_LABELED_BUTTON_QR_INSET,
    INFO_PROFILE_MEGAGROUP_COVER,
};
use style::menu_icons::{
    MENU_ICON_CHAT_BUBBLE, MENU_ICON_DISCUSSION, MENU_ICON_EMOJI_OBJECTS, MENU_ICON_FAQ,
    MENU_ICON_GIFT_PREMIUM, MENU_ICON_LOCK, MENU_ICON_MANAGE, MENU_ICON_NOTIFICATIONS,
    MENU_ICON_POWER_USAGE, MENU_ICON_PROFILE, MENU_ICON_SETTINGS, MENU_ICON_SHOP,
    MENU_ICON_SHOW_IN_FOLDER, MENU_ICON_TRANSLATE, MENU_ICON_UNMUTE,
};
use style::settings::{
    SETTINGS_NAME_LEFT, SETTINGS_NAME_TOP, SETTINGS_PHONE_LEFT, SETTINGS_PHONE_TOP,
    SETTINGS_PHOTO_BOTTOM, SETTINGS_PHOTO_LEFT, SETTINGS_PHOTO_TOP, SETTINGS_USERNAME_LEFT,
    SETTINGS_USERNAME_TOP,
};
use style::widgets::{DEFAULT_FLAT_LABEL, DEFAULT_TOAST};
use style::{ColorId, ColorRef, Icon, IconMask, Margins, MonoIcon};
use ui::{IconButton, Userpic, color, font, font_height};

use super::search::{Entry, Target};
use super::{Env, PageId, rows};
use crate::dialogs::photo_userpic;
use crate::lang::{format_phone, tr};
use crate::px_of;

/// `kUserpicPriority`-like: the cover's photo comes before lists.
const USERPIC_PRIORITY: i32 = 8;

/// The Premium rows' star (`AddPremiumStar`).
// shortcut: one flat colour, no gradient and no floating mini stars; draw `Ui::Premium::ColorizedSvg`'s gradient when the Premium page lands.
const PREMIUM_STAR: Icon = Icon::new(&[MonoIcon::new(
    IconMask::File {
        name: "premium/star",
        modifiers: &["20x20"],
    },
    ColorRef::Palette(ColorId::PremiumButtonBg2),
    Margins::new(2, 2, 2, 2),
)]);
const STARS_STAR: Icon = Icon::new(&[MonoIcon::new(
    IconMask::File {
        name: "premium/star",
        modifiers: &["20x20"],
    },
    ColorRef::Palette(ColorId::CreditsBg1),
    Margins::new(2, 2, 2, 2),
)]);

/// A row of the page and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    MyAccount,
    Open(PageId),
    PowerSaving,
    Language,
    Faq,
    Features,
    AskQuestion,
    SendGift,
}

const fn entry(
    id: &'static str,
    title: i18n::Key,
    keywords: &'static [&'static str],
    target: Target,
    icon: Icon,
) -> Entry {
    Entry {
        id,
        title,
        keywords,
        page: PageId::Main,
        target,
        icon: Some(icon),
    }
}

/// The groups of rows, between dividers, in tdesktop's order.
const SECTIONS: &[(Entry, Action)] = &[
    (
        entry(
            "main/account",
            key::lng_settings_my_account,
            &["profile", "edit", "information"],
            Target::Item,
            MENU_ICON_PROFILE,
        ),
        Action::MyAccount,
    ),
    (
        entry(
            "main/notify",
            key::lng_settings_section_notify,
            &["alerts", "sounds", "badge"],
            Target::Page(PageId::Notifications),
            MENU_ICON_NOTIFICATIONS,
        ),
        Action::Open(PageId::Notifications),
    ),
    (
        entry(
            "main/privacy",
            key::lng_settings_section_privacy,
            &["security", "passcode", "password", "2fa"],
            Target::Page(PageId::Privacy),
            MENU_ICON_LOCK,
        ),
        Action::Open(PageId::Privacy),
    ),
    (
        entry(
            "main/chat",
            key::lng_settings_section_chat_settings,
            &["themes", "appearance", "stickers"],
            Target::Page(PageId::Chat),
            MENU_ICON_CHAT_BUBBLE,
        ),
        Action::Open(PageId::Chat),
    ),
    (
        entry(
            "main/folders",
            key::lng_settings_section_filters,
            &["filters", "tabs"],
            Target::Page(PageId::Folders),
            MENU_ICON_SHOW_IN_FOLDER,
        ),
        Action::Open(PageId::Folders),
    ),
    (
        entry(
            "main/advanced",
            key::lng_settings_advanced,
            &["performance", "proxy", "experimental"],
            Target::Page(PageId::Advanced),
            MENU_ICON_MANAGE,
        ),
        Action::Open(PageId::Advanced),
    ),
    (
        entry(
            "main/devices",
            key::lng_settings_section_devices,
            &["sessions", "calls"],
            Target::Page(PageId::Calls),
            MENU_ICON_UNMUTE,
        ),
        Action::Open(PageId::Calls),
    ),
    (
        entry(
            "main/power",
            key::lng_settings_power_menu,
            &["battery", "animations", "power", "saving"],
            Target::Item,
            MENU_ICON_POWER_USAGE,
        ),
        Action::PowerSaving,
    ),
    (
        entry(
            "main/language",
            key::lng_settings_language,
            &["translate", "localization", "language"],
            Target::Item,
            MENU_ICON_TRANSLATE,
        ),
        Action::Language,
    ),
];

const PREMIUM: &[(Entry, Action)] = &[
    (
        entry(
            "main/premium",
            key::lng_premium_summary_title,
            &["subscription"],
            Target::Page(PageId::Premium),
            PREMIUM_STAR,
        ),
        Action::Open(PageId::Premium),
    ),
    (
        entry(
            "main/credits",
            key::lng_settings_credits,
            &["stars", "balance"],
            Target::Page(PageId::Credits),
            STARS_STAR,
        ),
        Action::Open(PageId::Credits),
    ),
    (
        entry(
            "main/business",
            key::lng_business_title,
            &["work", "company"],
            Target::Page(PageId::Business),
            MENU_ICON_SHOP,
        ),
        Action::Open(PageId::Business),
    ),
    (
        entry(
            "main/send-gift",
            key::lng_settings_gift_premium,
            &["present", "send"],
            Target::Item,
            MENU_ICON_GIFT_PREMIUM,
        ),
        Action::SendGift,
    ),
];

const HELP: &[(Entry, Action)] = &[
    (
        entry(
            "main/faq",
            key::lng_settings_faq,
            &[
                "help",
                "support",
                "questions",
                "shortcuts",
                "keyboard",
                "hotkeys",
                "legend",
            ],
            Target::Item,
            MENU_ICON_FAQ,
        ),
        Action::Faq,
    ),
    (
        entry(
            "main/features",
            key::lng_settings_features,
            &["tips", "tutorial"],
            Target::Item,
            MENU_ICON_EMOJI_OBJECTS,
        ),
        Action::Features,
    ),
    (
        entry(
            "main/ask-question",
            key::lng_settings_ask_question,
            &["contact", "feedback"],
            Target::Item,
            MENU_ICON_DISCUSSION,
        ),
        Action::AskQuestion,
    ),
];

/// rgram's own options, where tdesktop forks put theirs.
const RGRAM: &[(Entry, Action)] = &[(
    entry(
        "main/rgram",
        key::rgram_settings_title,
        &["rgram", "send", "enter"],
        Target::Page(PageId::Rgram),
        MENU_ICON_SETTINGS,
    ),
    Action::Open(PageId::Rgram),
)];

/// Every row of the page, for the search.
pub(super) fn entries() -> impl Iterator<Item = &'static Entry> {
    [SECTIONS, RGRAM, PREMIUM, HELP]
        .into_iter()
        .flatten()
        .map(|(entry, _)| entry)
}

pub(super) struct MainPage {
    env: Env,
    requested: HashSet<FileId>,
    _store: Subscription,
}

impl MainPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let store = cx.observe(&env.targets.store, |_, _, cx| cx.notify());
        Self {
            env,
            requested: HashSet::new(),
            _store: store,
        }
    }

    fn me(&self, cx: &App) -> Option<UserId> {
        self.env.targets.store.read(cx).state().me
    }

    fn download(&mut self, file: FileId, cx: &mut Context<Self>) {
        if !self.requested.insert(file) {
            return;
        }
        let task = self
            .env
            .targets
            .store
            .read(cx)
            .session()
            .download_file(file, USERPIC_PRIORITY);
        crate::peer_menu::send(async move { task.await.map(drop) }, "settings photo", cx);
    }

    /// `Settings::Cover`: the userpic, name, phone, username and QR button.
    fn cover(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let me = self.me(cx)?;
        let state = self.env.targets.store.read(cx).state();
        let user = state.chats.user(me)?;
        let name = user.name();
        let (userpic, missing) =
            photo_userpic(&state.chats, user.photo.as_ref(), user.accent_color, &name);
        let has_photo = user.photo.is_some();
        let phone = format_phone(user.phone.expose());
        let username = user.username.clone();
        if let Some(file) = missing {
            self.download(file, cx);
        }
        let st = &INFO_PROFILE_COVER;
        let height =
            px_of(SETTINGS_PHOTO_TOP) + px_of(st.photo.size.height) + px_of(SETTINGS_PHOTO_BOTTOM);
        let right = px_of(st.right_skip);
        let qr_right = px_of(st.right_skip) - px_of(INFO_PROFILE_LABELED_BUTTON_QR_INSET);
        let qr_top = (height - px_of(INFO_PROFILE_LABELED_BUTTON_QR.height)) / 2.0;
        let qr_room = px_of(INFO_PROFILE_LABELED_BUTTON_QR.width) + right;
        let env = self.env.clone();
        let photos = self.env.clone();
        let link = self.env.clone();
        let link_name = name.clone();
        let username_text = if username.is_empty() {
            tr(cx, key::lng_settings_username_add).to_string()
        } else {
            format!("@{username}")
        };
        let status = &INFO_PROFILE_MEGAGROUP_COVER.status;
        Some(
            div()
                .relative()
                .flex_none()
                .h(height)
                .child(
                    div()
                        .id("settings-userpic")
                        .absolute()
                        .left(px_of(SETTINGS_PHOTO_LEFT))
                        .top(px_of(SETTINGS_PHOTO_TOP))
                        .when(has_photo, |this| this.cursor_pointer())
                        .child(Userpic::new(px_of(st.photo.photo_size), userpic))
                        .on_click(move |_, window, cx| {
                            if has_photo {
                                let targets = &photos.targets;
                                crate::info::open_user_photos(
                                    targets.store.clone(),
                                    targets.layers.clone(),
                                    me,
                                    link_name.clone(),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
                .child(
                    rows::text(name, &st.name, cx)
                        .absolute()
                        .left(px_of(SETTINGS_NAME_LEFT))
                        .top(px_of(SETTINGS_NAME_TOP))
                        .right(right + qr_room)
                        .truncate(),
                )
                .child(
                    rows::text(phone, &DEFAULT_FLAT_LABEL, cx)
                        .absolute()
                        .left(px_of(SETTINGS_PHONE_LEFT))
                        .top(px_of(SETTINGS_PHONE_TOP))
                        .right(right),
                )
                .child(
                    div()
                        .id("settings-username")
                        .absolute()
                        .left(px_of(SETTINGS_USERNAME_LEFT))
                        .top(px_of(SETTINGS_USERNAME_TOP))
                        .cursor_pointer()
                        .font(font(status.style.font))
                        .text_size(px_of(status.style.font.size))
                        .line_height(font_height(status.style.font))
                        .text_color(color(cx, status.palette.link_fg))
                        .hover(|this| this.underline())
                        .child(username_text)
                        .on_click(move |_, window, cx| {
                            username_clicked(&link, &username, window, cx);
                        }),
                )
                .child(div().absolute().right(qr_right).top(qr_top).child(
                    IconButton::new("settings-qr", &INFO_PROFILE_LABELED_BUTTON_QR).on_click(
                        move |_, window, cx| {
                            super::qr_box::show(&env, ChatId(me.0), window, cx);
                        },
                    ),
                ))
                .into_any_element(),
        )
    }

    fn group(
        &self,
        group: &'static [(Entry, Action)],
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        group
            .iter()
            .map(|(entry, action)| {
                let icon = entry.icon.unwrap_or(MENU_ICON_SETTINGS);
                let mut button = rows::button(entry.id, tr(cx, entry.title), icon);
                if *action == Action::Language {
                    button = button.label(tr(cx, key::lng_language_name));
                }
                let env = self.env.clone();
                let action = *action;
                button
                    .on_click(move |_, window, cx| run(action, &env, window, cx))
                    .into_any_element()
            })
            .collect()
    }
}

/// The username copies the profile link, or asks for one when there is
/// none (`Cover::initViewers`).
fn username_clicked(env: &Env, username: &str, window: &mut Window, cx: &mut App) {
    match Some(username).filter(|username| !username.is_empty()) {
        Some(username) => {
            cx.write_to_clipboard(ClipboardItem::new_string(format!(
                "https://t.me/{username}"
            )));
            let text = tr(cx, key::lng_username_copied);
            env.targets.layers.update(cx, |layers, cx| {
                layers.show_toast(text, &DEFAULT_TOAST, crate::toast::DEFAULT_DURATION, cx);
            });
        }
        None => env.open(PageId::Information, window, cx),
    }
}

fn run(action: Action, env: &Env, window: &mut Window, cx: &mut App) {
    match action {
        Action::MyAccount => open_my_profile(env, window, cx),
        Action::Open(page) => env.open(page, window, cx),
        Action::Faq => cx.open_url(&tr(cx, key::lng_settings_faq_link)),
        Action::Features => cx.open_url(&tr(cx, key::lng_telegram_features_url)),
        Action::PowerSaving => super::power_box::show(env, window, cx),
        // shortcut: these boxes come with their sections later in M7; until then the rows do nothing.
        Action::Language | Action::AskQuestion | Action::SendGift => {
            tracing::debug!(?action, "settings action not built yet");
        }
    }
}

/// "My Account" and the menu's "Edit info": the own profile over Settings.
pub(super) fn open_my_profile(env: &Env, window: &mut Window, cx: &mut App) {
    let Some(me) = env.targets.store.read(cx).state().me else {
        return;
    };
    // A private chat's id is its user's id.
    crate::info::show_above(env.targets.clone(), ChatId(me.0), window, cx);
}

impl Render for MainPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column: Vec<gpui::AnyElement> = Vec::new();
        column.extend(self.cover(cx));
        column.push(rows::divider(cx));
        column.push(rows::skip());
        column.extend(self.group(SECTIONS, cx));
        column.extend(self.group(RGRAM, cx));
        column.push(rows::skip());
        for group in [PREMIUM, HELP] {
            column.push(rows::divider(cx));
            column.push(rows::skip());
            column.extend(self.group(group, cx));
            column.push(rows::skip());
        }
        div().flex().flex_col().children(column)
    }
}
