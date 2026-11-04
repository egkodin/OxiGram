//! "My Account" (`Settings::Information`): the photo with its upload
//! button, the bio, the name, phone and username, the personal channel and
//! the birthday.
// shortcut: no right-click copy menus on the rows, no name colour row (`AddPeerColorButton`), no business chat automation row and no accounts list under the birthday; with row "My account".

use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use chrono::Local;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, StatefulInteractiveElement as _, Styled, Subscription, Task, Window,
    div, prelude::*,
};
use i18n::key;
use model::profile::UserProfile;
use model::{FileId, UserId};
use style::menu_icons::{
    MENU_ICON_CHANNEL, MENU_ICON_GIFT_PREMIUM, MENU_ICON_PHONE, MENU_ICON_PROFILE,
    MENU_ICON_USERNAME,
};
use style::settings::{
    SETTINGS_BIO, SETTINGS_BIO_COUNTDOWN, SETTINGS_BIO_MARGINS, SETTINGS_COVER_NAME,
    SETTINGS_COVER_STATUS, SETTINGS_INFO_PHOTO, SETTINGS_INFO_PHOTO_HEIGHT,
    SETTINGS_INFO_PHOTO_SKIP, SETTINGS_INFO_PHOTO_TOP, SETTINGS_INFO_UPLOAD_LEFT,
};
use style::userpic_button::UPLOAD_USERPIC_BUTTON;
use style::{ColorId, ColorRef};
use ui::{InputEvent, InputField, Markup, Userpic};

use super::{Env, rows};
use crate::dialogs::photo_userpic;
use crate::info::{OwnDetails, OwnField, Saved};
use crate::lang::{format_phone, strings, tr, tr_with};
use crate::px_of;

/// `kSaveBioTimeout`.
const SAVE_BIO_DELAY: Duration = Duration::from_millis(1000);
/// `aboutLengthDefault` and `aboutLengthPremium`.
const BIO_LIMIT: usize = 70;
const BIO_LIMIT_PREMIUM: usize = 140;
const USERPIC_PRIORITY: i32 = 8;

pub(super) struct InformationPage {
    env: Env,
    profile: Option<UserProfile>,
    bio: Entity<InputField>,
    /// The bio Telegram has.
    bio_saved: String,
    bio_save: Option<Task<()>>,
    load: Option<Task<()>>,
    requested: HashSet<FileId>,
    _subscriptions: [Subscription; 2],
}

impl InformationPage {
    pub(super) fn new(env: Env, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = tr(cx, key::lng_bio_placeholder);
        let bio = cx.new(|cx| {
            InputField::new(&SETTINGS_BIO, placeholder, window, cx)
                .with_max_length(BIO_LIMIT_PREMIUM * 2)
        });
        let bio_changed = cx.subscribe(&bio, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Changed => this.bio_changed(cx),
            InputEvent::Submitted => this.save_bio(cx),
            _ => {}
        });
        let store = cx.observe(&env.targets.store, |_, _, cx| cx.notify());
        let mut page = Self {
            env,
            profile: None,
            bio,
            bio_saved: String::new(),
            bio_save: None,
            load: None,
            requested: HashSet::new(),
            _subscriptions: [bio_changed, store],
        };
        page.reload(cx);
        page
    }

    fn me(&self, cx: &App) -> Option<UserId> {
        self.env.targets.store.read(cx).state().me
    }

    /// Asks for the own full info: the bio, birthday and channel.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(me) = self.me(cx) else {
            return;
        };
        let task = crate::store::spawn(
            self.env.targets.store.read(cx).session().user_profile(me),
            cx,
        );
        self.load = Some(cx.spawn(async move |this, cx| match task.await {
            Ok(profile) => {
                let _ = this.update(cx, |this, cx| this.loaded(profile, cx));
            }
            Err(error) => tracing::warn!(%error, "own profile failed"),
        }));
    }

    fn loaded(&mut self, profile: UserProfile, cx: &mut Context<Self>) {
        let bio = profile.bio.text.clone();
        // A bio being typed stays as it is.
        if self.bio.read(cx).text() == self.bio_saved {
            self.bio
                .update(cx, |field, cx| field.set_text(&bio, None, cx));
        }
        self.bio_saved = bio;
        self.profile = Some(profile);
        cx.notify();
    }

    fn bio_limit(&self, cx: &App) -> usize {
        let state = self.env.targets.store.read(cx).state();
        let is_premium = state
            .me
            .and_then(|me| state.chats.user(me))
            .is_some_and(|user| user.is_premium);
        if is_premium {
            BIO_LIMIT_PREMIUM
        } else {
            BIO_LIMIT
        }
    }

    /// Saves a changed bio a second after the last key (`SetupBio`).
    fn bio_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.bio.read(cx).text().to_owned();
        if text.contains('\n') {
            let single = text.replace('\n', " ");
            self.bio
                .update(cx, |field, cx| field.set_text(&single, None, cx));
        }
        self.bio_save = (self.bio.read(cx).text() != self.bio_saved).then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SAVE_BIO_DELAY).await;
                let _ = this.update(cx, |this, cx| this.save_bio(cx));
            })
        });
        cx.notify();
    }

    fn save_bio(&mut self, cx: &mut Context<Self>) {
        self.bio_save = None;
        let text = self.bio.read(cx).text().trim().to_owned();
        if text == self.bio_saved || text.chars().count() > self.bio_limit(cx) {
            return;
        }
        self.bio_saved.clone_from(&text);
        let task = self.env.targets.store.read(cx).session().set_bio(text);
        crate::peer_menu::send(task, "save bio", cx);
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

    fn reloader(&self, cx: &Context<Self>) -> Saved {
        let page = cx.weak_entity();
        Rc::new(move |cx| {
            let _ = page.update(cx, |page, cx| {
                page.requested.clear();
                page.reload(cx);
            });
        })
    }

    fn edit(&self, field: OwnField, window: &mut Window, cx: &mut Context<Self>) {
        let details = OwnDetails {
            bio: self.bio_saved.clone(),
            birthday: self.profile.as_ref().and_then(|profile| profile.birthday),
        };
        let on_saved = self.reloader(cx);
        let targets = &self.env.targets;
        crate::info::edit_own(
            field,
            &targets.store,
            &targets.layers,
            details,
            on_saved,
            window,
            cx,
        );
    }

    /// `SetupPhoto`: the photo, its upload button, the name and "online".
    fn photo(&mut self, me: UserId, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.env.targets.store.read(cx).state();
        let user = state.chats.user(me)?;
        let name = user.name();
        let (userpic, missing) =
            photo_userpic(&state.chats, user.photo.as_ref(), user.accent_color, &name);
        let has_photo = user.photo.is_some();
        let status = crate::status::status(strings(cx), user, &Local::now()).text;
        if let Some(file) = missing {
            self.download(file, cx);
        }
        let st = &SETTINGS_INFO_PHOTO;
        let size = px_of(st.size.width);
        let upload = &UPLOAD_USERPIC_BUTTON;
        let upload_size = px_of(upload.size.width);
        let photos = self.env.targets.clone();
        let photo_name = name.clone();
        let store = self.env.targets.store.clone();
        let on_photo = self.reloader(cx);
        let photo = div()
            .id("account-photo")
            .relative()
            .size(size)
            .when(has_photo, |this| this.cursor_pointer())
            .child(Userpic::new(px_of(st.photo_size), userpic))
            .on_click(move |_, window, cx| {
                if has_photo {
                    crate::info::open_user_photos(
                        photos.store.clone(),
                        photos.layers.clone(),
                        me,
                        photo_name.clone(),
                        window,
                        cx,
                    );
                }
            })
            .child(
                div()
                    .id("account-upload")
                    .absolute()
                    .left(size - upload_size + px_of(SETTINGS_INFO_UPLOAD_LEFT))
                    .top(size - upload_size)
                    .size(upload_size)
                    .rounded_full()
                    .cursor_pointer()
                    .bg(ui::color(cx, upload.change_button.text_bg))
                    .hover(|this| this.bg(ui::color(cx, upload.change_button.text_bg_over)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(ui::icon(upload.change_icon))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        crate::info::pick_own_photo(&store, on_photo.clone(), cx);
                    }),
            );
        Some(
            div()
                .flex_none()
                .h(px_of(SETTINGS_INFO_PHOTO_HEIGHT))
                .pt(px_of(SETTINGS_INFO_PHOTO_TOP))
                .flex()
                .flex_col()
                .items_center()
                .child(photo)
                .child(
                    rows::text(name, &SETTINGS_COVER_NAME, cx)
                        .mt(px_of(SETTINGS_INFO_PHOTO_SKIP))
                        .truncate(),
                )
                .child(rows::text(status, &SETTINGS_COVER_STATUS, cx))
                .into_any_element(),
        )
    }

    /// `SetupBio`: the field with the characters left on its right.
    fn bio_field(&self, cx: &App) -> AnyElement {
        let limit = self.bio_limit(cx);
        let typed = self.bio.read(cx).text().chars().count();
        let left = i64::try_from(limit).unwrap_or(i64::MAX) - i64::try_from(typed).unwrap_or(0);
        let countdown = rows::text(left.to_string(), &SETTINGS_BIO_COUNTDOWN, cx)
            .absolute()
            .right(px_of(SETTINGS_BIO_MARGINS.right))
            .top(px_of(SETTINGS_BIO_MARGINS.top) + px_of(SETTINGS_BIO.text_margins.top))
            .when(left < 0, |this| {
                this.text_color(ui::color(cx, ColorRef::Palette(ColorId::BoxTextFgError)))
            });
        div()
            .relative()
            .flex_none()
            .pl(px_of(SETTINGS_BIO_MARGINS.left))
            .pr(px_of(SETTINGS_BIO_MARGINS.right))
            .pt(px_of(SETTINGS_BIO_MARGINS.top))
            .pb(px_of(SETTINGS_BIO_MARGINS.bottom))
            .child(self.bio.clone())
            .child(countdown)
            .into_any_element()
    }

    /// `AddRow`: a section row with its value on the right.
    fn row(
        &self,
        id: &'static str,
        label: impl Into<gpui::SharedString>,
        value: String,
        icon: style::Icon,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = cx.weak_entity();
        rows::button(id, label, icon)
            .label(value)
            .on_click(move |_, window, cx| {
                let _ = page.update(cx, |this, cx| on_click(this, window, cx));
            })
            .into_any_element()
    }

    fn details(&self, me: UserId, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.env.targets.store.read(cx).state();
        let Some(user) = state.chats.user(me) else {
            return Vec::new();
        };
        let name = user.name();
        let phone = format_phone(user.phone.expose());
        let username = user.username.clone();
        let channel = self
            .profile
            .as_ref()
            .and_then(|profile| profile.personal_chat)
            .and_then(|chat| state.chats.chat(chat))
            .map(|chat| chat.title.clone());
        let birthday = self
            .profile
            .as_ref()
            .and_then(|profile| profile.birthday)
            .map(|birthday| crate::info::birthday_text(birthday, cx));
        let or_add = |value: Option<String>, add| value.unwrap_or_else(|| tr(cx, add).to_string());
        let (username_label, username_value) = if username.is_empty() {
            (
                "t.me/username".into(),
                tr(cx, key::lng_settings_username_add).to_string(),
            )
        } else {
            (
                tr(cx, key::lng_settings_username_label),
                format!("@{username}"),
            )
        };
        let channel = or_add(channel, key::lng_settings_channel_add);
        let birthday = or_add(birthday, key::lng_settings_birthday_add);
        let birthday_about = tr_with(
            cx,
            key::lng_settings_birthday_about,
            &[("link", &tr(cx, key::lng_settings_birthday_about_link))],
        );
        vec![
            rows::divider_text(tr(cx, key::lng_settings_about_bio), cx),
            rows::skip(),
            self.row(
                "account-name",
                tr(cx, key::lng_settings_name_label),
                name,
                MENU_ICON_PROFILE,
                |this, window, cx| this.edit(OwnField::Name, window, cx),
                cx,
            ),
            self.row(
                "account-phone",
                tr(cx, key::lng_settings_phone_label),
                phone,
                MENU_ICON_PHONE,
                |this, window, cx| {
                    let text = Markup::plain(tr(cx, key::lng_change_phone_error).to_string());
                    crate::boxes::inform(&this.env.targets.layers, text, None, window, cx);
                },
                cx,
            ),
            self.row(
                "account-username",
                username_label,
                username_value,
                MENU_ICON_USERNAME,
                |this, window, cx| this.edit(OwnField::Username, window, cx),
                cx,
            ),
            rows::skip(),
            rows::divider_text(tr(cx, key::lng_settings_username_about), cx),
            rows::skip(),
            self.row(
                "account-channel",
                tr(cx, key::lng_settings_channel_label),
                channel,
                MENU_ICON_CHANNEL,
                |this, window, cx| {
                    let current = this
                        .profile
                        .as_ref()
                        .and_then(|profile| profile.personal_chat);
                    let on_saved = this.reloader(cx);
                    super::personal_channel::show(&this.env, current, on_saved, window, cx);
                },
                cx,
            ),
            rows::skip(),
            rows::divider(cx),
            rows::skip(),
            self.row(
                "account-birthday",
                tr(cx, key::lng_settings_birthday_label),
                birthday,
                MENU_ICON_GIFT_PREMIUM,
                |this, window, cx| this.edit(OwnField::Birthday, window, cx),
                cx,
            ),
            rows::skip(),
            rows::divider_text(birthday_about.replace("**", ""), cx),
        ]
    }
}

impl Render for InformationPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(me) = self.me(cx) else {
            return div();
        };
        let photo = self.photo(me, cx);
        let bio = self.bio_field(cx);
        let details = self.details(me, cx);
        div()
            .flex()
            .flex_col()
            .children(photo)
            .child(bio)
            .children(details)
    }
}
