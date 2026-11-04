//! "Personal channel" (`ShowEditPersonalChannel`): one's public channels;
//! the chosen one shows on one's profile. Remove takes it off.

use std::collections::HashSet;

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, StatefulInteractiveElement as _, Styled, Task, Window, div,
};
use i18n::key;
use model::{ChatId, FileId};
use style::boxes::CONTACTS_WITH_STORIES;
use style::layers::DEFAULT_BOX;
use style::widgets::DEFAULT_TOAST;
use ui::{BoxFrame, RoundButton, color};

use super::Env;
use crate::dialogs::photo_userpic;
use crate::info::Saved;
use crate::lang::tr;
use crate::peer_list;
use crate::status::members_status;

const USERPIC_PRIORITY: i32 = 8;

pub(super) struct PersonalChannelBox {
    env: Env,
    current: Option<ChatId>,
    /// None while Telegram is asked.
    channels: Option<Vec<ChatId>>,
    on_saved: Saved,
    requested: HashSet<FileId>,
    focus: FocusHandle,
    _load: Task<()>,
}

impl std::fmt::Debug for PersonalChannelBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersonalChannelBox").finish_non_exhaustive()
    }
}

impl Focusable for PersonalChannelBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(
    env: &Env,
    current: Option<ChatId>,
    on_saved: Saved,
    window: &mut Window,
    cx: &mut App,
) {
    let task = crate::store::spawn(
        env.targets
            .store
            .read(cx)
            .session()
            .suitable_personal_chats(),
        cx,
    );
    let env = env.clone();
    let layers = env.targets.layers.clone();
    let view = cx.new(|cx| {
        let load = cx.spawn(async move |this, cx| {
            let channels = task.await.unwrap_or_else(|error| {
                tracing::warn!(%error, "personal channels failed");
                Vec::new()
            });
            let _ = this.update(cx, |this: &mut PersonalChannelBox, cx| {
                this.channels = Some(channels);
                cx.notify();
            });
        });
        PersonalChannelBox {
            env,
            current,
            channels: None,
            on_saved,
            requested: HashSet::new(),
            focus: cx.focus_handle(),
            _load: load,
        }
    });
    layers.update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl PersonalChannelBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    /// `SavePersonalChannel`.
    fn save(&self, chat: Option<ChatId>, window: &mut Window, cx: &mut App) {
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .set_personal_chat(chat),
            cx,
        );
        let (layers, on_saved) = (self.env.targets.layers.clone(), self.on_saved.clone());
        let text = tr(
            cx,
            if chat.is_some() {
                key::lng_settings_channel_saved
            } else {
                key::lng_settings_channel_removed
            },
        );
        cx.spawn(async move |cx| match task.await {
            Ok(()) => {
                cx.update(|cx| {
                    on_saved(cx);
                    layers.update(cx, |layers, cx| {
                        layers.show_toast(text, &DEFAULT_TOAST, crate::toast::DEFAULT_DURATION, cx);
                    });
                });
            }
            Err(error) => tracing::warn!(%error, "saving the personal channel failed"),
        })
        .detach();
        self.close(window, cx);
    }

    fn row(
        &mut self,
        index: usize,
        chat: ChatId,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let state = self.env.targets.store.read(cx).state();
        let found = state.chats.chat(chat)?;
        let (userpic, missing) = photo_userpic(
            &state.chats,
            found.photo.as_ref(),
            found.accent_color,
            &found.title,
        );
        let status = members_status(found, state.chats.member_count(found), cx).unwrap_or_default();
        let look = peer_list::Look {
            name: found.title.clone().into(),
            userpic,
            status: status.into(),
            is_active: false,
        };
        if let Some(file) = missing.filter(|file| self.requested.insert(*file)) {
            let task = self
                .env
                .targets
                .store
                .read(cx)
                .session()
                .download_file(file, USERPIC_PRIORITY);
            crate::peer_menu::send(async move { task.await.map(drop) }, "channel photo", cx);
        }
        let entity = cx.entity();
        let hover = color(cx, CONTACTS_WITH_STORIES.item.button.text_bg_over);
        Some(
            peer_list::row(("personal-channel", index), look, false, cx)
                .cursor_pointer()
                .hover(move |this| this.bg(hover))
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.save(Some(chat), window, cx));
                })
                .into_any_element(),
        )
    }
}

impl Render for PersonalChannelBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = match self.channels.clone() {
            None => div().into_any_element(),
            Some(channels) if channels.is_empty() => {
                peer_list::empty(tr(cx, key::lng_settings_channel_no_yet), cx)
            }
            Some(channels) => {
                let rows: Vec<_> = channels
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, chat)| self.row(index, chat, cx))
                    .collect();
                div()
                    .id("personal-channels")
                    .max_h(peer_list::list_height(window))
                    .overflow_y_scroll()
                    .children(rows)
                    .into_any_element()
            }
        };
        let entity = cx.entity();
        let mut frame = BoxFrame::new("personal-channel-box", &DEFAULT_BOX)
            .title(tr(cx, key::lng_settings_channel_label))
            .row(list);
        if self.current.is_some() {
            let remove = entity.clone();
            // shortcut: Remove sits with Done, not on the box's left as tdesktop's `addLeftButton`.
            frame = frame.button(
                RoundButton::new(
                    "personal-channel-remove",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_settings_channel_remove),
                )
                .on_click(move |_, window, cx| {
                    remove.update(cx, |this, cx| this.save(None, window, cx));
                }),
            );
        }
        div().track_focus(&self.focus).child(
            frame.button(
                RoundButton::new(
                    "personal-channel-done",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_box_done),
                )
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.close(window, cx));
                }),
            ),
        )
    }
}
