//! The contacts box from the main menu (`PrepareContactsBox`): contacts
//! sorted by when they were last online, filtered by name.

use std::collections::HashSet;
use std::rc::Rc;

use chrono::Local;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, WeakEntity, Window, div, prelude::*, px,
    uniform_list,
};
use i18n::key;
use model::user::User;
use model::{FileId, UserId};
use style::basic::{NO_CONTACTS_COLOR, NO_CONTACTS_FONT, NO_CONTACTS_HEIGHT};
use style::boxes::{CONTACTS_STATUS_FONT, CONTACTS_WITH_STORIES};
use style::layers::{BOX_MAX_LIST_HEIGHT, DEFAULT_BOX};
use style::widgets::DEFAULT_MULTI_SELECT;
use ui::{BoxFrame, InputEvent, InputField, RoundButton, Userpic, color, font, font_height};

use crate::Store;
use crate::dialogs::photo_userpic;
use crate::intro::country_box::search_words;
use crate::lang::{strings, tr};
use crate::layers::{Dismiss, Layers};
use crate::px_of;
use crate::status::{sort_value, status};

/// Room the box leaves around itself in a short window.
const WINDOW_MARGIN: f32 = 160.0;
/// Userpics wait behind what the user opened.
const USERPIC_PRIORITY: i32 = 8;

pub(crate) type OnChosen = Rc<dyn Fn(UserId, &mut Window, &mut App)>;

pub(crate) struct ContactsBox {
    store: Entity<Store>,
    layers: WeakEntity<Layers>,
    search: Entity<InputField>,
    query: Vec<String>,
    hovered: Option<usize>,
    requested_files: HashSet<FileId>,
    on_chosen: OnChosen,
    _subscriptions: [Subscription; 2],
}

impl std::fmt::Debug for ContactsBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContactsBox").finish_non_exhaustive()
    }
}

impl Focusable for ContactsBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

/// Opens the box and asks TDLib for the contacts; `on_chosen` runs with the
/// picked contact after the box closes.
pub(crate) fn show(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    on_chosen: OnChosen,
    window: &mut Window,
    cx: &mut App,
) {
    let task = crate::store::spawn(store.read(cx).session().contacts(), cx);
    cx.spawn(async move |_| {
        if let Err(error) = task.await {
            tracing::warn!(%error, "loading contacts failed");
        }
    })
    .detach();
    let weak = layers.downgrade();
    let view = cx.new(|cx| {
        let search = cx.new(|cx| {
            InputField::new(
                &DEFAULT_MULTI_SELECT.field,
                tr(cx, key::lng_participant_filter),
                window,
                cx,
            )
        });
        let search_changed = cx.subscribe(&search, |this: &mut ContactsBox, search, event, cx| {
            if *event == InputEvent::Changed {
                this.query = search_words(search.read(cx).text());
                cx.notify();
            }
        });
        // Contacts arrive and change status as user updates.
        let store_changed = cx.observe(store, |_, _, cx| cx.notify());
        ContactsBox {
            store: store.clone(),
            layers: weak,
            search,
            query: Vec::new(),
            hovered: None,
            requested_files: HashSet::new(),
            on_chosen,
            _subscriptions: [search_changed, store_changed],
        }
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx)
    });
}

/// Contacts whose name has a word starting with each query word, the most
/// recently online first (`SortMode::Online`), then by name.
// shortcut: no alphabet sort toggle, section headers or story rings; they come with the profile work in M6.
fn shown<'a>(
    users: impl Iterator<Item = &'a User>,
    me: Option<UserId>,
    query: &[String],
    now: i64,
) -> Vec<&'a User> {
    let mut list: Vec<&User> = users
        .filter(|user| user.is_contact && Some(user.id) != me)
        .filter(|user| {
            let words = search_words(&user.name());
            query
                .iter()
                .all(|word| words.iter().any(|name| name.starts_with(word.as_str())))
        })
        .collect();
    list.sort_by_cached_key(|user| {
        (
            std::cmp::Reverse(sort_value(user, now)),
            user.name().to_lowercase(),
        )
    });
    list
}

impl ContactsBox {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    fn choose(&mut self, user: UserId, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        (self.on_chosen)(user, window, cx);
    }

    fn download(&mut self, file: FileId, cx: &mut Context<Self>) {
        if !self.requested_files.insert(file) {
            return;
        }
        let task = crate::store::spawn(
            self.store
                .read(cx)
                .session()
                .download_file(file, USERPIC_PRIORITY),
            cx,
        );
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "userpic download failed");
            }
        })
        .detach();
    }

    fn row(&mut self, user: &User, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let st = &CONTACTS_WITH_STORIES.item;
        let chats = &self.store.read(cx).state().chats;
        let name = user.name();
        let (source, download) =
            photo_userpic(chats, user.photo.as_ref(), user.accent_color, &name);
        let line = status(strings(cx), user, &Local::now());
        if let Some(file) = download {
            self.download(file, cx);
        }
        let is_hovered = self.hovered == Some(index);
        let status_fg = if line.is_online {
            st.status_fg_active
        } else if is_hovered {
            st.status_fg_over
        } else {
            st.status_fg
        };
        let name_font = st.name_style.font;
        let status_font = CONTACTS_STATUS_FONT;
        let user_id = user.id;
        div()
            .id(("contact", index))
            .relative()
            .h(px_of(st.height))
            .w_full()
            .bg(color(
                cx,
                if is_hovered {
                    st.button.text_bg_over
                } else {
                    CONTACTS_WITH_STORIES.bg
                },
            ))
            .child(
                div()
                    .absolute()
                    .left(px_of(st.photo_position.x))
                    .top(px_of(st.photo_position.y))
                    .child(Userpic::new(px_of(st.photo_size), source)),
            )
            .child(
                div()
                    .absolute()
                    .left(px_of(st.name_position.x))
                    .top(px_of(st.name_position.y))
                    .right(px(0.0))
                    .pr(px_of(st.photo_position.x))
                    .font(font(name_font))
                    .text_size(px_of(name_font.size))
                    .line_height(font_height(name_font))
                    .text_color(color(cx, st.name_fg))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(SharedString::from(name)),
            )
            .child(
                div()
                    .absolute()
                    .left(px_of(st.status_position.x))
                    .top(px_of(st.status_position.y))
                    .font(font(status_font))
                    .text_size(px_of(status_font.size))
                    .line_height(font_height(status_font))
                    .text_color(color(cx, status_fg))
                    .whitespace_nowrap()
                    .child(SharedString::from(line.text)),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                let next = if *hovered {
                    Some(index)
                } else {
                    this.hovered.filter(|was| *was != index)
                };
                if this.hovered != next {
                    this.hovered = next;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.choose(user_id, window, cx)))
            .into_any_element()
    }
}

impl Render for ContactsBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &DEFAULT_MULTI_SELECT;
        let search_height =
            px_of(st.field.height_min) + px_of(st.padding.top) + px_of(st.padding.bottom);
        let window_room = window.viewport_size().height - px(WINDOW_MARGIN);
        let list_height = (px_of(BOX_MAX_LIST_HEIGHT) - search_height)
            .min(window_room)
            .max(px(100.0));
        let search = div()
            .bg(color(cx, st.bg))
            .pl(px_of(st.padding.left))
            .pr(px_of(st.padding.right))
            .pt(px_of(st.padding.top))
            .pb(px_of(st.padding.bottom))
            .flex()
            .items_center()
            .gap(px_of(st.field_icon_skip))
            .child(ui::icon(st.field_icon))
            .child(div().flex_1().child(self.search.clone()));

        let state = self.store.read(cx).state();
        let now = Local::now().timestamp();
        let ids: Rc<Vec<UserId>> = Rc::new(
            shown(state.chats.users(), state.me, &self.query, now)
                .into_iter()
                .map(|user| user.id)
                .collect(),
        );
        let list = if ids.is_empty() {
            let text = if self.query.is_empty() {
                key::lng_contacts_not_found
            } else {
                key::lng_blocked_list_not_found
            };
            div()
                .h(px_of(NO_CONTACTS_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .font(font(NO_CONTACTS_FONT))
                .text_size(px_of(NO_CONTACTS_FONT.size))
                .text_color(color(cx, NO_CONTACTS_COLOR))
                .child(tr(cx, text))
                .into_any_element()
        } else {
            uniform_list(
                "contacts",
                ids.len(),
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    range
                        .filter_map(|index| {
                            let user = this.store.read(cx).state().chats.user(ids[index])?.clone();
                            Some(this.row(&user, index, cx))
                        })
                        .collect::<Vec<_>>()
                }),
            )
            .h(list_height)
            .into_any_element()
        };
        let entity = cx.entity();
        BoxFrame::new("contacts-box", &DEFAULT_BOX)
            .title(tr(cx, key::lng_contacts_header))
            .row(search)
            .row(list)
            // shortcut: no "Add contact" button until the add-contact box in M6.
            .button(
                RoundButton::new(
                    "contacts-close",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_close),
                )
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.close(window, cx))
                }),
            )
    }
}

#[cfg(test)]
mod tests {
    use model::user::{Badge, UserKind, UserStatus};

    use super::*;

    fn user(id: i64, name: &str, status: UserStatus, is_contact: bool) -> User {
        User {
            id: UserId(id),
            first_name: name.to_owned(),
            last_name: String::new(),
            status,
            photo: None,
            accent_color: 0,
            emoji_status: None,
            badge: Badge::None,
            is_premium: false,
            phone: model::Secret::default(),
            kind: UserKind::Regular,
            is_contact,
            username: String::new(),
        }
    }

    #[test]
    fn lists_contacts_online_first_filtered_by_name_prefix() {
        let users = [
            user(1, "Me", UserStatus::Recently, true),
            user(2, "Zoe", UserStatus::Online { expires: i32::MAX }, true),
            user(3, "Adam", UserStatus::Recently, true),
            user(
                4,
                "Stranger",
                UserStatus::Online { expires: i32::MAX },
                false,
            ),
            user(5, "Abby", UserStatus::Recently, true),
        ];
        let names = |query: &str| -> Vec<String> {
            shown(users.iter(), Some(UserId(1)), &search_words(query), 1000)
                .into_iter()
                .map(User::name)
                .collect()
        };
        assert_eq!(names(""), ["Zoe", "Abby", "Adam"]);
        assert_eq!(names("ad"), ["Adam"]);
        assert_eq!(names("dam"), Vec::<String>::new());
    }
}
