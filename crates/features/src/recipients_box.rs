//! "Choose recipient..." (`ChooseRecipientBoxController`): Saved Messages,
//! then the main chat list in its order, filtered by name, only chats the
//! user can write in.

use std::collections::HashSet;
use std::rc::Rc;

use chrono::Local;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Styled,
    Subscription, WeakEntity, Window, prelude::*, uniform_list,
};
use i18n::{Strings, key};
use model::chat::{ChatKind, ChatList};
use model::rights::Writing;
use model::{ChatId, FileId, State};
use style::layers::DEFAULT_BOX;
use style::widgets::DEFAULT_MULTI_SELECT;
use ui::{BoxFrame, InputEvent, InputField, RoundButton, UserpicSource};

use crate::Store;
use crate::dialogs::photo_userpic;
use crate::intro::country_box::search_words;
use crate::lang::{strings, tr};
use crate::layers::{Dismiss, Layers};
use crate::peer_list;
use crate::status::{members_status, status};

/// Userpics wait behind what the user opened.
const USERPIC_PRIORITY: i32 = 8;

pub(crate) type OnChosen = Rc<dyn Fn(ChatId, &mut Window, &mut App)>;

pub(crate) struct RecipientsBox {
    store: Entity<Store>,
    layers: WeakEntity<Layers>,
    search: Entity<InputField>,
    query: Vec<String>,
    hovered: Option<usize>,
    requested_files: HashSet<FileId>,
    on_chosen: OnChosen,
    _subscriptions: [Subscription; 2],
}

impl std::fmt::Debug for RecipientsBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecipientsBox").finish_non_exhaustive()
    }
}

impl Focusable for RecipientsBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

/// Opens the box; `on_chosen` runs with the picked chat after it closes.
pub(crate) fn show(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    on_chosen: OnChosen,
    window: &mut Window,
    cx: &mut App,
) {
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
        let search_changed =
            cx.subscribe(&search, |this: &mut RecipientsBox, search, event, cx| {
                if *event == InputEvent::Changed {
                    this.query = search_words(search.read(cx).text());
                    cx.notify();
                }
            });
        let store_changed = cx.observe(store, |_, _, cx| cx.notify());
        RecipientsBox {
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

/// The chat with the user themself, if TDLib sent it.
fn saved_messages(state: &State) -> Option<ChatId> {
    let me = state.me?;
    // TDLib gives a private chat the id of its user.
    let chat = ChatId(me.0);
    state.chats.chat(chat).map(|_| chat)
}

fn name(state: &State, strings: &Strings, chat: ChatId) -> String {
    if Some(chat) == saved_messages(state) {
        return strings.text(key::lng_saved_messages).to_owned();
    }
    state
        .chats
        .chat(chat)
        .map(|chat| chat.title.clone())
        .unwrap_or_default()
}

/// Saved Messages first, then the main list, keeping those the user can
/// write in whose name has a word starting with each query word.
// shortcut: no global search and no archived chats; with search in M5.
fn shown(state: &State, strings: &Strings, query: &[String]) -> Vec<ChatId> {
    let saved = saved_messages(state);
    saved
        .into_iter()
        .chain(
            state
                .chats
                .list(ChatList::Main)
                .filter(|chat| Some(*chat) != saved),
        )
        .filter(|chat| {
            state.chats.chat(*chat).is_some_and(|chat| {
                !matches!(chat.kind, ChatKind::Secret { .. })
                    && state.chats.writing(chat.id) == Writing::Field
            })
        })
        .filter(|chat| {
            let words = search_words(&name(state, strings, *chat));
            query
                .iter()
                .all(|word| words.iter().any(|name| name.starts_with(word.as_str())))
        })
        .collect()
}

impl RecipientsBox {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    fn choose(&mut self, chat: ChatId, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        (self.on_chosen)(chat, window, cx);
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

    /// The row's look: the user's status, or the group's member count.
    fn look(&self, chat: ChatId, cx: &App) -> Option<(peer_list::Look, Option<FileId>)> {
        let state = self.store.read(cx).state();
        let strings = strings(cx);
        let found = state.chats.chat(chat)?;
        let title = name(state, strings, chat);
        if Some(chat) == saved_messages(state) {
            let look = peer_list::Look {
                name: title.into(),
                userpic: UserpicSource::SavedMessages,
                status: Default::default(),
                is_active: false,
            };
            return Some((look, None));
        }
        let (userpic, download) = photo_userpic(
            &state.chats,
            found.photo.as_ref(),
            found.accent_color,
            &title,
        );
        let (text, is_active) = match found.user().and_then(|user| state.chats.user(user)) {
            Some(user) => {
                let line = status(strings, user, &Local::now());
                (line.text, line.is_online)
            }
            None => (
                members_status(found, state.chats.member_count(found), cx).unwrap_or_default(),
                false,
            ),
        };
        let look = peer_list::Look {
            name: title.into(),
            userpic,
            status: text.into(),
            is_active,
        };
        Some((look, download))
    }

    fn row(
        &mut self,
        chat: ChatId,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (look, download) = self.look(chat, cx)?;
        if let Some(file) = download {
            self.download(file, cx);
        }
        Some(
            peer_list::row(("recipient", index), look, self.hovered == Some(index), cx)
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    let next = peer_list::hovered_after(this.hovered, index, *hovered);
                    if this.hovered != next {
                        this.hovered = next;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, window, cx| this.choose(chat, window, cx)))
                .into_any_element(),
        )
    }
}

impl Render for RecipientsBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let search = peer_list::search_row(self.search.clone(), cx);
        let ids = Rc::new(shown(self.store.read(cx).state(), strings(cx), &self.query));
        let list = if ids.is_empty() {
            peer_list::empty(tr(cx, key::lng_blocked_list_not_found), cx)
        } else {
            uniform_list(
                "recipients",
                ids.len(),
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    range
                        .filter_map(|index| this.row(ids[index], index, cx))
                        .collect::<Vec<_>>()
                }),
            )
            .h(peer_list::list_height(window))
            .into_any_element()
        };
        let entity = cx.entity();
        BoxFrame::new("recipients-box", &DEFAULT_BOX)
            .title(tr(cx, key::lng_forward_choose))
            .row(search)
            .row(list)
            .button(
                RoundButton::new(
                    "recipients-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.close(window, cx))
                }),
            )
    }
}

#[cfg(test)]
mod tests {
    use model::chat::{Chat, Position};
    use model::chats::Change;
    use model::rights::{MemberStatus, Permissions};
    use model::{SupergroupId, UserId};

    use super::*;

    fn chat(id: i64, title: &str, kind: ChatKind, order: i64) -> Chat {
        Chat {
            positions: vec![Position {
                list: ChatList::Main,
                order,
                is_pinned: false,
            }],
            ..Chat::new(ChatId(id), kind, title)
        }
    }

    #[test]
    fn saved_messages_lead_and_closed_chats_are_left_out() {
        let mut state = State::default();
        state.me = Some(UserId(7));
        let private = |user| ChatKind::Private { user: UserId(user) };
        let channel = ChatKind::Supergroup {
            supergroup: SupergroupId(3),
            is_channel: true,
        };
        let closed = Chat {
            permissions: Permissions {
                can_send_basic_messages: false,
            },
            ..chat(
                4,
                "Quiet Group",
                ChatKind::Group {
                    group: model::BasicGroupId(4),
                },
                30,
            )
        };
        for chat in [
            chat(7, "Me", private(7), 10),
            chat(1, "Ann Lee", private(1), 50),
            chat(3, "Trail News", channel, 40),
            closed,
        ] {
            state.chats.apply(Change::Chat(Box::new(chat)));
        }
        state.chats.apply(Change::Supergroup {
            supergroup: SupergroupId(3),
            count: 10,
            status: MemberStatus::Member,
        });
        state.chats.apply(Change::Group {
            group: model::BasicGroupId(4),
            count: 3,
            status: MemberStatus::Member,
        });
        let strings = Strings::english();
        assert_eq!(shown(&state, &strings, &[]), [ChatId(7), ChatId(1)]);
        assert_eq!(shown(&state, &strings, &["sav".to_owned()]), [ChatId(7)]);
        assert_eq!(shown(&state, &strings, &["lee".to_owned()]), [ChatId(1)]);
    }
}
