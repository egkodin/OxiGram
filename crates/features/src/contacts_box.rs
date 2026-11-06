//! The contacts box from the main menu (`PrepareContactsBox`): contacts
//! sorted by when they were last online, filtered by name.

use std::rc::Rc;

use chrono::Local;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Styled,
    Subscription, WeakEntity, Window, prelude::*, uniform_list,
};
use i18n::key;
use model::user::User;
use model::{FileId, UserId};
use style::layers::DEFAULT_BOX;
use style::widgets::DEFAULT_MULTI_SELECT;
use ui::{BoxFrame, InputEvent, InputField, RoundButton};

use crate::Store;
use crate::dialogs::photo_userpic;
use crate::intro::country_box::search_words;
use crate::lang::{strings, tr};
use crate::layers::{Dismiss, Layers};
use crate::peer_list;
use crate::status::{sort_value, status};

/// Userpics wait behind what the user opened.
const USERPIC_PRIORITY: i32 = 8;

pub(crate) type OnChosen = Rc<dyn Fn(UserId, &mut Window, &mut App)>;
pub(crate) type OnDone = Rc<dyn Fn(Vec<UserId>, &mut Window, &mut App)>;

/// Picking several contacts, as new groups and channels add members
/// (`AddParticipantsBoxController`).
// shortcut: picked rows keep the hover background, with no round checks or chips over the list.
pub(crate) struct Many {
    pub title: gpui::SharedString,
    pub done: gpui::SharedString,
    pub on_done: OnDone,
    pub cancel: Cancel,
    /// Ticked when the box opens.
    pub picked: Vec<UserId>,
}

/// What the box's other button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cancel {
    Close,
    /// "Skip" (a channel just created): done with nobody picked.
    Skip,
}

pub(crate) struct ContactsBox {
    store: Entity<Store>,
    layers: WeakEntity<Layers>,
    search: Entity<InputField>,
    query: Vec<String>,
    hovered: Option<usize>,
    on_chosen: OnChosen,
    many: Option<(Many, Vec<UserId>)>,
    /// A title of its own, with "Cancel", over the layer below.
    title: Option<gpui::SharedString>,
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
    open(store, layers, on_chosen, None, None, window, cx);
}

/// The box under `title`, kept over the layer below it, as "Gift Premium
/// or Stars" picks its recipient (`ChooseStarGiftRecipient`).
pub(crate) fn show_above(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    title: gpui::SharedString,
    on_chosen: OnChosen,
    window: &mut Window,
    cx: &mut App,
) {
    open(store, layers, on_chosen, None, Some(title), window, cx);
}

/// The box with checks; `many.on_done` gets the picked contacts.
pub(crate) fn show_many(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    many: Many,
    window: &mut Window,
    cx: &mut App,
) {
    open(
        store,
        layers,
        Rc::new(|_, _, _| {}),
        Some(many),
        None,
        window,
        cx,
    );
}

fn open(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    on_chosen: OnChosen,
    many: Option<Many>,
    title: Option<gpui::SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    let is_above = title.is_some();
    let task = crate::store::spawn(store.read(cx).session().contacts(), cx);
    cx.spawn(async move |_| match task.await {
        Ok(contacts) => tracing::debug!(count = contacts.len(), "contacts loaded"),
        Err(error) => tracing::warn!(%error, "loading contacts failed"),
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
            on_chosen,
            many: many.map(|many| {
                let picked = many.picked.clone();
                (many, picked)
            }),
            title,
            _subscriptions: [search_changed, store_changed],
        }
    });
    layers.update(cx, |layers, cx| {
        if is_above {
            layers.show_above(view, window, cx);
        } else {
            layers.show(view, Dismiss::Allowed, None, window, cx);
        }
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
        if let Some((_, picked)) = &mut self.many {
            if let Some(at) = picked.iter().position(|id| *id == user) {
                picked.remove(at);
            } else {
                picked.push(user);
            }
            cx.notify();
            return;
        }
        self.close(window, cx);
        (self.on_chosen)(user, window, cx);
    }

    /// Closes the box and hands over the picked contacts.
    fn done(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((many, picked)) = &self.many else {
            return;
        };
        let (on_done, picked) = (many.on_done.clone(), picked.clone());
        self.close(window, cx);
        on_done(picked, window, cx);
    }

    /// Closes the box as if nobody was picked.
    fn skip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((many, _)) = &self.many else {
            return;
        };
        let on_done = many.on_done.clone();
        self.close(window, cx);
        on_done(Vec::new(), window, cx);
    }

    fn download(&mut self, file: FileId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.load_userpic(file, USERPIC_PRIORITY, cx)
        });
    }

    fn row(&mut self, user: &User, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let chats = &self.store.read(cx).state().chats;
        let name = user.name();
        let (userpic, download) =
            photo_userpic(chats, user.photo.as_ref(), user.accent_color, &name);
        let line = status(strings(cx), user, &Local::now());
        if let Some(file) = download {
            self.download(file, cx);
        }
        let look = peer_list::Look {
            name: name.into(),
            userpic,
            status: line.text.into(),
            is_active: line.is_online,
        };
        let user_id = user.id;
        let is_picked = self
            .many
            .as_ref()
            .is_some_and(|(_, picked)| picked.contains(&user_id));
        peer_list::row(
            ("contact", index),
            look,
            is_picked || self.hovered == Some(index),
            cx,
        )
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            let next = peer_list::hovered_after(this.hovered, index, *hovered);
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
        let list_height = peer_list::list_height(window);
        let search = peer_list::search_row(self.search.clone(), cx);

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
            peer_list::empty(tr(cx, text), cx)
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
        let (title, close, cancel) = match &self.many {
            Some((many, _)) if many.cancel == Cancel::Skip => {
                (many.title.clone(), key::lng_create_group_skip, Cancel::Skip)
            }
            Some((many, _)) => (many.title.clone(), key::lng_cancel, Cancel::Close),
            None => match &self.title {
                Some(title) => (title.clone(), key::lng_cancel, Cancel::Close),
                None => (
                    tr(cx, key::lng_contacts_header),
                    key::lng_close,
                    Cancel::Close,
                ),
            },
        };
        let done = self.many.as_ref().map(|(many, _)| {
            let entity = entity.clone();
            RoundButton::new("contacts-done", &DEFAULT_BOX.button, many.done.clone())
                .on_click(move |_, window, cx| entity.update(cx, |this, cx| this.done(window, cx)))
        });
        let frame = BoxFrame::new("contacts-box", &DEFAULT_BOX)
            .title(title)
            .row(search)
            .row(list);
        let frame = match done {
            Some(done) => frame.button(done),
            None => frame,
        };
        // shortcut: no "Add contact" button until the add-contact box in M6.
        frame.button(
            RoundButton::new("contacts-close", &DEFAULT_BOX.button, tr(cx, close)).on_click(
                move |_, window, cx| {
                    entity.update(cx, |this, cx| match cancel {
                        Cancel::Close => this.close(window, cx),
                        Cancel::Skip => this.skip(window, cx),
                    })
                },
            ),
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
