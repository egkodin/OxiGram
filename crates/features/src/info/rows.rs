//! The profile's cover and details (`Info::Profile::Cover`,
//! `DetailsFiller::setupInfo`), notifications and the shared media
//! buttons (`SetupSharedMediaClassic`).

use chrono::{Datelike as _, Local};
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::*,
};
use i18n::{Key, key};
use model::MessageId;
use model::chat_actions::Muting;
use model::file::File;
use model::profile::{Birthday, SharedMedia};
use model::user::Photo;
use style::ColorId;
use style::boxes::{INFO_LABEL, INFO_LABEL_SKIP, INFO_LABELED};
use style::info::{
    INFO_ICON_MEDIA_AUDIO, INFO_ICON_MEDIA_FILE, INFO_ICON_MEDIA_GIF, INFO_ICON_MEDIA_GROUP,
    INFO_ICON_MEDIA_LINK, INFO_ICON_MEDIA_PHOTO, INFO_ICON_MEDIA_VIDEO, INFO_ICON_MEDIA_VOICE,
    INFO_PROFILE_BUTTON, INFO_PROFILE_COVER, INFO_SHARED_MEDIA_BUTTON,
};
use ui::{FlatLabel, Markup, SettingsButton, Userpic, UserpicSource, color};

use super::edit::Field;
use super::{InfoView, Subject, divider, text};
use crate::dialogs::photo_userpic;
use crate::history::{MediaEntry, MediaKind, open_media};
use crate::lang::{strings, tr, tr_plural, tr_with};
use crate::px_of;

/// `infoProfileLabeledPadding`, which lives in info_profile_actions.style.
// shortcut: the style file is not generated yet; regenerate crates/style with it when more of its values are needed.
const LABELED_PADDING: (style::Px, style::Px, style::Px, style::Px) =
    (style::Px(23), style::Px(9), style::Px(20), style::Px(7));

const MONTHS: [Key; 12] = [
    key::lng_month_day1,
    key::lng_month_day2,
    key::lng_month_day3,
    key::lng_month_day4,
    key::lng_month_day5,
    key::lng_month_day6,
    key::lng_month_day7,
    key::lng_month_day8,
    key::lng_month_day9,
    key::lng_month_day10,
    key::lng_month_day11,
    key::lng_month_day12,
];

/// What the cover shows.
struct Look {
    name: String,
    photo: Option<Photo>,
    accent_color: i32,
    status: Option<(String, bool)>,
}

impl InfoView {
    fn look(&self, cx: &App) -> Look {
        let state = self.store().read(cx).state();
        if let Subject::User(id) = self.subject
            && let Some(user) = state.chats.user(id)
        {
            let status = crate::status::status(strings(cx), user, &Local::now());
            return Look {
                name: user.name(),
                photo: user.photo.clone(),
                accent_color: user.accent_color,
                status: Some((status.text, status.is_online)),
            };
        }
        let Some(chat) = self.chat(state) else {
            return Look {
                name: String::new(),
                photo: None,
                accent_color: 0,
                status: None,
            };
        };
        let count = self.group.as_ref().map(|group| group.member_count);
        Look {
            name: chat.title.clone(),
            photo: chat.photo.clone(),
            accent_color: chat.accent_color,
            status: crate::status::members_status(chat, count, cx).map(|line| (line, false)),
        }
    }

    /// The 640 px photo once it is on disk; the small one or the blurred
    /// preview until then (`Info::Profile::Cover` asks for the big one).
    fn big_userpic(&mut self, look: &Look, cx: &mut Context<Self>) -> UserpicSource {
        let chats = &self.store().read(cx).state().chats;
        let big = look.photo.as_ref().map(|photo| {
            (
                photo.big.id,
                chats.file(photo.big.id).and_then(|f| f.path.clone()),
            )
        });
        let (fallback, _) =
            photo_userpic(chats, look.photo.as_ref(), look.accent_color, &look.name);
        match big {
            Some((_, Some(path))) => UserpicSource::File(path.into()),
            Some((id, None)) => {
                self.download(id, cx);
                fallback
            }
            None => fallback,
        }
    }

    pub(super) fn cover(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let st = &INFO_PROFILE_COVER;
        let look = self.look(cx);
        let userpic = self.big_userpic(&look, cx);
        let has_photo = look.photo.is_some();
        let status = look.status.map(|(line, is_online)| {
            let label = text(line, &st.status, cx);
            let fg = if is_online {
                ColorId::WindowActiveTextFg
            } else {
                ColorId::WindowSubTextFg
            };
            label.text_color(color(cx, style::ColorRef::Palette(fg)))
        });
        let entity = cx.entity();
        div()
            .relative()
            .flex_none()
            .h(px_of(st.height))
            .child(
                div()
                    .id("info-userpic")
                    .absolute()
                    .left(px_of(st.photo_left))
                    .top(px_of(st.photo_top))
                    .when(has_photo, |this| this.cursor_pointer())
                    .child(Userpic::new(px_of(st.photo.photo_size), userpic))
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| this.open_photos(window, cx));
                    }),
            )
            .child(
                text(look.name, &st.name, cx)
                    .absolute()
                    .left(px_of(st.name_left))
                    .right(px_of(st.right_skip))
                    .top(px_of(st.name_top))
                    .truncate(),
            )
            .children(status.map(|status| {
                status
                    .absolute()
                    .left(px_of(st.status_left))
                    .right(px_of(st.right_skip))
                    .top(px_of(st.status_top))
                    .truncate()
            }))
            .into_any_element()
    }

    /// The photo viewer on the userpic: a user's photos, newest first; a
    /// group's current photo.
    // shortcut: a group's photo history (`getChatPhotoHistory`) is not paged through; only its current photo.
    fn open_photos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let look = self.look(cx);
        let Some(photo) = look.photo else {
            return;
        };
        let Subject::User(user) = self.subject else {
            let files = vec![photo.big];
            self.show_photos(files, look.name, window, cx);
            return;
        };
        let task = crate::store::spawn(self.store().read(cx).session().profile_photos(user), cx);
        cx.spawn_in(window, async move |this, cx| match task.await {
            Ok(files) => {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.show_photos(files, look.name, window, cx);
                });
            }
            Err(error) => tracing::warn!(%error, "profile photos failed"),
        })
        .detach();
    }

    fn show_photos(&self, files: Vec<File>, name: String, window: &mut Window, cx: &mut App) {
        if files.is_empty() {
            return;
        }
        let entries = files
            .into_iter()
            .zip(1..)
            .map(|(file, index)| MediaEntry {
                message: MessageId(index),
                kind: MediaKind::Photo,
                file: Some(file),
                still: None,
                dimensions: None,
                caption: String::new(),
                sender: name.clone(),
                date: String::new(),
                is_saveable: false,
            })
            .collect();
        open_media(self.store().clone(), self.layers(), entries, 0, window, cx);
    }

    pub(super) fn details(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = if self.is_self(cx) {
            self.own_rows(cx)
        } else {
            self.detail_rows(cx)
        };
        if !rows.is_empty() {
            rows.push(div().h(px_of(LABELED_PADDING.3)).into_any_element());
        }
        if let Some(notifications) = self.notifications(cx) {
            rows.push(notifications);
        }
        rows.push(divider(cx).into_any_element());
        rows
    }

    fn detail_rows(&self, cx: &App) -> Vec<AnyElement> {
        let state = self.store().read(cx).state();
        let mut rows = Vec::new();
        let mut add = |value: String, label: Key, cx: &App| {
            if !value.is_empty() {
                rows.push(labeled(rows.len(), value, tr(cx, label), cx).into_any_element());
            }
        };
        match self.subject {
            Subject::User(id) => {
                let user = state.chats.user(id);
                if let Some(profile) = &self.user {
                    add(profile.bio.text.clone(), key::lng_info_bio_label, cx);
                }
                if let Some(user) = user {
                    add(at(&user.username), key::lng_info_username_label, cx);
                    add(phone(user.phone.expose()), key::lng_info_mobile_label, cx);
                }
                if let Some(birthday) = self.user.as_ref().and_then(|p| p.birthday) {
                    add(
                        birthday_text(birthday, cx),
                        key::lng_info_birthday_label,
                        cx,
                    );
                }
            }
            Subject::Group(_) | Subject::Supergroup { .. } => {
                if let Some(group) = &self.group {
                    add(group.description.clone(), key::lng_info_about_label, cx);
                    add(group.invite_link.clone(), key::lng_info_link_label, cx);
                }
            }
        }
        rows
    }

    /// One's own details, each opening its edit box (`Settings::Information`).
    fn own_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.store().read(cx).state();
        let Some(user) = state.me.and_then(|me| state.chats.user(me)) else {
            return Vec::new();
        };
        let bio = self
            .user
            .as_ref()
            .map(|p| p.bio.text.clone())
            .unwrap_or_default();
        let birthday = self.user.as_ref().and_then(|p| p.birthday);
        let or = |value: String, empty: Key| {
            if value.is_empty() {
                tr(cx, empty).to_string()
            } else {
                value
            }
        };
        let fields = [
            (Field::Name, user.name(), key::lng_settings_name_label),
            (
                Field::Bio,
                or(bio, key::lng_settings_birthday_add),
                key::lng_info_bio_label,
            ),
            (
                Field::Username,
                or(at(&user.username), key::lng_settings_username_add),
                key::lng_info_username_label,
            ),
            (
                Field::Birthday,
                or(
                    birthday.map(|b| birthday_text(b, cx)).unwrap_or_default(),
                    key::lng_settings_birthday_add,
                ),
                key::lng_info_birthday_label,
            ),
        ];
        let phone_row = (!user.phone.is_empty()).then(|| {
            labeled(
                9,
                phone(user.phone.expose()),
                tr(cx, key::lng_info_mobile_label),
                cx,
            )
        });
        let mut rows: Vec<AnyElement> = fields
            .into_iter()
            .enumerate()
            .map(|(index, (field, value, label))| {
                let entity = cx.entity();
                labeled(index, value, tr(cx, label), cx)
                    .cursor_pointer()
                    .hover(|this| {
                        this.bg(color(cx, style::ColorRef::Palette(ColorId::WindowBgOver)))
                    })
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| this.edit(field, window, cx));
                    })
                    .into_any_element()
            })
            .collect();
        rows.extend(phone_row.map(IntoElement::into_any_element));
        let entity = cx.entity();
        rows.push(
            SettingsButton::new(
                "info-set-photo",
                &INFO_PROFILE_BUTTON,
                tr(cx, key::lng_profile_set_photo_button),
            )
            .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.pick_photo(cx)))
            .into_any_element(),
        );
        rows
    }

    /// "Notifications" with its toggle, for chats other than one's own.
    fn notifications(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.is_self(cx) {
            return None;
        }
        let chat = self.chat(self.store().read(cx).state())?;
        let is_muted = chat.is_muted();
        let (session, id) = (self.store().read(cx).session().clone(), chat.id);
        let muting = if is_muted {
            Muting::Unmuted
        } else {
            Muting::Muted
        };
        Some(
            SettingsButton::new(
                "info-notifications",
                &INFO_PROFILE_BUTTON,
                tr(cx, key::lng_profile_enable_notifications),
            )
            .toggle(!is_muted)
            .on_click(move |_, _, cx| {
                // shortcut: mutes for good; the durations come with M7 notification settings.
                crate::peer_menu::send(session.set_chat_muted(id, muting), "mute", cx);
            })
            .into_any_element(),
        )
    }

    pub(super) fn media_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows: Vec<AnyElement> = self
            .counts
            .iter()
            .map(|&(media, count)| {
                let (phrase, icon) = media_phrase(media);
                let entity = cx.entity();
                SettingsButton::new(
                    ("info-media", media as usize),
                    &INFO_SHARED_MEDIA_BUTTON,
                    tr_plural(cx, phrase, i64::from(count), &[]),
                )
                .icon(icon)
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.show_media(media, window, cx));
                })
                .into_any_element()
            })
            .collect();
        let groups = self.user.as_ref().map_or(0, |p| p.common_groups);
        if groups > 0 && !self.is_self(cx) {
            let entity = cx.entity();
            rows.push(
                SettingsButton::new(
                    "info-common-groups",
                    &INFO_SHARED_MEDIA_BUTTON,
                    tr_plural(cx, key::lng_profile_common_groups, i64::from(groups), &[]),
                )
                .icon(INFO_ICON_MEDIA_GROUP)
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.show_common_groups(window, cx));
                })
                .into_any_element(),
            );
        }
        if !rows.is_empty() {
            rows.push(divider(cx).into_any_element());
        }
        rows
    }
}

/// A value over its label (`CreateTextWithLabel`).
fn labeled(
    index: usize,
    value: String,
    label: SharedString,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let (left, top, right, bottom) = LABELED_PADDING;
    div()
        .id(("info-labeled", index))
        .pl(px_of(left))
        .pr(px_of(right))
        .pt(px_of(top))
        .pb(px_of(bottom))
        .child(FlatLabel::new(
            ("info-value", index),
            &INFO_LABELED,
            Markup::plain(value),
        ))
        .child(
            div()
                .pt(px_of(INFO_LABEL_SKIP))
                .child(text(label, &INFO_LABEL, cx)),
        )
}

fn at(username: &str) -> String {
    if username.is_empty() {
        String::new()
    } else {
        format!("@{username}")
    }
}

// shortcut: "+digits" without the country's grouping (`Ui::FormatPhone`); format when the phone module learns the patterns.
fn phone(digits: &str) -> String {
    if digits.is_empty() {
        String::new()
    } else {
        format!("+{digits}")
    }
}

pub(super) fn media_phrase(media: SharedMedia) -> (Key, style::Icon) {
    match media {
        SharedMedia::Photo => (key::lng_profile_photos, INFO_ICON_MEDIA_PHOTO),
        SharedMedia::Video => (key::lng_profile_videos, INFO_ICON_MEDIA_VIDEO),
        SharedMedia::File => (key::lng_profile_files, INFO_ICON_MEDIA_FILE),
        SharedMedia::Music => (key::lng_profile_songs, INFO_ICON_MEDIA_AUDIO),
        SharedMedia::Link => (key::lng_profile_shared_links, INFO_ICON_MEDIA_LINK),
        SharedMedia::Voice => (key::lng_profile_audios, INFO_ICON_MEDIA_VOICE),
        SharedMedia::Gif => (key::lng_profile_gifs, INFO_ICON_MEDIA_GIF),
    }
}

/// "November 3, 1990 (35 years old)" or "November 3" (`Data::Birthday`).
fn birthday_text(birthday: Birthday, cx: &App) -> String {
    let index = usize::try_from(birthday.month - 1).unwrap_or(0).min(11);
    let month = tr(cx, MONTHS[index]);
    let day = birthday.day.to_string();
    let Some(year) = birthday.year else {
        return tr_with(cx, key::lng_month_day, &[("month", &month), ("day", &day)]);
    };
    let date = tr_with(
        cx,
        key::lng_month_day_year,
        &[
            ("month", &month),
            ("day", &day),
            ("year", &year.to_string()),
        ],
    );
    let today = Local::now().date_naive();
    let had_birthday = (today.month(), today.day())
        >= (
            u32::try_from(birthday.month).unwrap_or(0),
            u32::try_from(birthday.day).unwrap_or(0),
        );
    let age = today.year() - year - i32::from(!had_birthday);
    tr_plural(
        cx,
        key::lng_info_birthday_years,
        i64::from(age),
        &[("date", &date)],
    )
}
