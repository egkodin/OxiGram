//! "Blocked users" (`Settings::Blocked`): Block user, what blocking does,
//! then the list with Unblock on each row.
// shortcut: the first PER_PAGE blocked only, Block user picks from contacts (tdesktop lists chats) and the empty list has no animation; load more on scroll with row "Blocked users".

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement as _, Styled, Task, Window, div, prelude::*,
};
use i18n::key;
use model::privacy::{Blocked, BlockedSlice};
use model::user::UserKind;
use model::{ChatId, UserId};
use style::boxes::CONTACTS_WITH_STORIES;
use style::menu_icons::MENU_ICON_BLOCK_SETTINGS;
use style::settings::SETTINGS_BUTTON_ACTIVE;
use ui::{SettingsButton, color, font};

use super::{Env, rows};
use crate::contacts_box;
use crate::dialogs::photo_userpic;
use crate::lang::{format_phone, tr, tr_plural, tr_with};
use crate::peer_list;
use crate::px_of;

/// `kPerPage` doubled: one request covers most lists.
const PER_PAGE: i32 = 100;

pub(super) struct BlockedPage {
    env: Env,
    /// None while Telegram is asked.
    slice: Option<BlockedSlice>,
    hovered: Option<usize>,
    _load: Option<Task<()>>,
}

impl BlockedPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            env,
            slice: None,
            hovered: None,
            _load: None,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .blocked(0, PER_PAGE),
            cx,
        );
        self._load = Some(cx.spawn(async move |this, cx| match task.await {
            Ok(slice) => {
                let _ = this.update(cx, |this, cx| {
                    this.slice = Some(slice);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "blocked list failed"),
        }));
    }

    /// Runs `request`, then asks for the list again.
    fn change(
        &mut self,
        request: impl Future<Output = Result<(), model::RequestError>> + Send + 'static,
        what: &'static str,
        cx: &mut Context<Self>,
    ) {
        let task = crate::store::spawn(request, cx);
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                tracing::warn!(%error, what, "blocked list change failed");
            }
            let _ = this.update(cx, |this, cx| this.reload(cx));
        })
        .detach();
    }

    fn unblock(&mut self, sender: Blocked, cx: &mut Context<Self>) {
        let task = self.env.targets.store.read(cx).session().unblock(sender);
        self.change(task, "unblock", cx);
    }

    /// `BlockNewPeer`: a contact, then the confirmation.
    fn block_new(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (env, weak) = (self.env.clone(), cx.weak_entity());
        let on_chosen: contacts_box::OnChosen = std::rc::Rc::new(move |user, window, cx| {
            confirm_block(&env, weak.clone(), user, window, cx);
        });
        let targets = &self.env.targets;
        contacts_box::show(&targets.store, &targets.layers, on_chosen, window, cx);
    }

    fn row(&self, index: usize, sender: Blocked, cx: &mut Context<Self>) -> Option<AnyElement> {
        let look = look(&self.env, sender, cx)?;
        let st = &CONTACTS_WITH_STORIES.item;
        let action = &st.button;
        let is_hovered = self.hovered == Some(index);
        let (weak, open, env) = (cx.weak_entity(), cx.weak_entity(), self.env.clone());
        let chat = match sender {
            Blocked::User(user) => ChatId(user.0),
            Blocked::Chat(chat) => chat,
        };
        let unblock = div()
            .id(("blocked-unblock", index))
            .absolute()
            .right(px_of(st.photo_position.x))
            .top((px_of(st.height) - ui::font_height(action.font)) / 2.0)
            .font(font(action.font))
            .text_size(px_of(action.font.size))
            .text_color(color(cx, action.text_fg))
            .cursor_pointer()
            .child(tr(cx, key::lng_blocked_list_unblock))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                let _ = weak.update(cx, |this, cx| this.unblock(sender, cx));
            });
        Some(
            peer_list::row(("blocked-row", index), look, is_hovered, cx)
                .cursor_pointer()
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.hovered = peer_list::hovered_after(this.hovered, index, *hovered);
                    cx.notify();
                }))
                .on_click(move |_, window, cx| {
                    let _ = open.update(cx, |_, cx| {
                        crate::info::show_above(env.targets.clone(), chat, window, cx);
                    });
                })
                .child(unblock)
                .into_any_element(),
        )
    }
}

/// The row's userpic, name and `BlockedBoxController::createRow`'s status.
fn look(env: &Env, sender: Blocked, cx: &mut App) -> Option<peer_list::Look> {
    let state = env.targets.store.read(cx).state();
    let (name, photo, accent, status) = match sender {
        Blocked::User(user) => {
            let user = state.chats.user(user)?;
            let status: SharedString = if !user.phone.is_empty() {
                format_phone(user.phone.expose()).into()
            } else if !user.username.is_empty() {
                format!("@{}", user.username).into()
            } else if user.kind == UserKind::Bot {
                tr(cx, key::lng_status_bot)
            } else {
                tr(cx, key::lng_blocked_list_unknown_phone)
            };
            (user.name(), user.photo.as_ref(), user.accent_color, status)
        }
        Blocked::Chat(chat) => {
            let chat = state.chats.chat(chat)?;
            let status = tr(cx, key::lng_group_status);
            (
                chat.title.clone(),
                chat.photo.as_ref(),
                chat.accent_color,
                status,
            )
        }
    };
    let (userpic, _) = photo_userpic(&state.chats, photo, accent, &name);
    Some(peer_list::Look {
        name: name.into(),
        userpic,
        status,
        is_active: false,
    })
}

fn confirm_block(
    env: &Env,
    page: gpui::WeakEntity<BlockedPage>,
    user: UserId,
    window: &mut Window,
    cx: &mut App,
) {
    let name = env
        .targets
        .store
        .read(cx)
        .state()
        .chats
        .user(user)
        .map(model::user::User::name)
        .unwrap_or_default();
    let confirmed: crate::boxes::Callback = std::rc::Rc::new(move |_, cx| {
        let _ = page.update(cx, |page, cx| {
            let task = page.env.targets.store.read(cx).session().block_user(user);
            page.change(task, "block", cx);
        });
    });
    crate::boxes::confirm_attention(
        &env.targets.layers,
        tr_with(cx, key::lng_blocked_list_confirm_text, &[("name", &name)]),
        tr(cx, key::lng_blocked_list_confirm_ok),
        confirmed,
        window,
        cx,
    );
}

impl Render for BlockedPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let block = SettingsButton::new(
            "blocked-add",
            &SETTINGS_BUTTON_ACTIVE,
            tr(cx, key::lng_blocked_list_add),
        )
        .icon(MENU_ICON_BLOCK_SETTINGS)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.block_new(window, cx));
        });
        let senders = self
            .slice
            .as_ref()
            .map(|slice| slice.senders.clone())
            .unwrap_or_default();
        let total = self.slice.as_ref().map_or(0, |slice| slice.total);
        let rows: Vec<_> = senders
            .into_iter()
            .enumerate()
            .filter_map(|(index, sender)| self.row(index, sender, cx))
            .collect();
        let empty = self.slice.is_some() && total == 0;
        div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(block)
            .child(rows::skip())
            .child(rows::divider_text(tr(cx, key::lng_blocked_list_about), cx))
            .when(total > 0, |this| {
                this.child(rows::skip()).child(rows::subtitle(
                    tr_plural(cx, key::lng_blocked_list_subtitle, i64::from(total), &[]),
                    cx,
                ))
            })
            .children(rows)
            .when(empty, |this| {
                this.child(rows::skip())
                    .child(rows::subtitle(
                        tr(cx, key::lng_blocked_list_empty_title),
                        cx,
                    ))
                    .child(rows::divider_text(
                        tr(cx, key::lng_blocked_list_empty_description),
                        cx,
                    ))
            })
            .child(rows::skip())
    }
}
