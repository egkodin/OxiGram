//! A shared media list (`Info::Media::ListWidget`) and the groups in
//! common (`Info::CommonGroups`), each in a box over the profile.
// shortcut: every list is rows of the message's preview and date, photos and videos too, where tdesktop draws a grid of thumbnails; add the grid with the media thumbnails.

use chrono::{Local, TimeZone as _};
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled,
    Window, div,
};
use i18n::key;
use model::message::Message;
use model::profile::SharedMedia;
use model::{ChatId, RequestError};
use style::boxes::{INFO_LABEL, INFO_LABELED};
use style::info::INFO_LAYER_TOP_MINIMAL;
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::{ColorId, ColorRef};
use ui::{BoxFrame, RoundButton, color};

use super::{InfoView, text};
use crate::dialogs::{OpenMessage, photo_userpic, preview};
use crate::history::{MediaEntry, open_media};
use crate::lang::{strings, tr, tr_plural};
use crate::links::Targets;
use crate::peer_list::{self, Look};
use crate::px_of;

/// `infoMediaMargin`'s sides around a row.
const ROW_PADDING: (style::Px, style::Px) = (style::Px(20), style::Px(8));

enum Items {
    Messages(SharedMedia, Vec<Message>),
    Chats(Vec<ChatId>),
}

struct ListBox {
    title: SharedString,
    items: Items,
    targets: Targets,
    chat: ChatId,
    hovered: Option<usize>,
    focus: FocusHandle,
}

impl Focusable for ListBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl InfoView {
    pub(super) fn show_media(
        &mut self,
        media: SharedMedia,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self
            .counts
            .iter()
            .find(|(m, _)| *m == media)
            .map_or(0, |(_, c)| *c);
        let (phrase, _) = super::rows::media_phrase(media);
        let title = tr_plural(cx, phrase, i64::from(count), &[]).into();
        let task = self
            .store()
            .read(cx)
            .session()
            .shared_media(self.chat, media);
        self.show_list(
            title,
            task,
            move |messages| Items::Messages(media, messages),
            window,
            cx,
        );
    }

    pub(super) fn show_common_groups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let super::Subject::User(user) = self.subject else {
            return;
        };
        let count = self
            .user
            .as_ref()
            .map_or(0, |profile| profile.common_groups);
        let title = tr_plural(cx, key::lng_profile_common_groups, i64::from(count), &[]).into();
        let task = self.store().read(cx).session().groups_in_common(user);
        self.show_list(title, task, Items::Chats, window, cx);
    }

    fn show_list<T: Send + 'static>(
        &self,
        title: SharedString,
        task: impl std::future::Future<Output = Result<T, RequestError>> + Send + 'static,
        items: impl FnOnce(T) -> Items + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (targets, chat) = (self.targets.clone(), self.chat);
        let task = crate::store::spawn(task, cx);
        cx.spawn_in(window, async move |this, cx| match task.await {
            Ok(found) => {
                let _ = this.update_in(cx, |_, window, cx| {
                    let layers = targets.layers.clone();
                    let view = cx.new(|cx| ListBox {
                        title,
                        items: items(found),
                        targets,
                        chat,
                        hovered: None,
                        focus: cx.focus_handle(),
                    });
                    layers.update(cx, |layers, cx| layers.show_above(view, window, cx));
                });
            }
            Err(error) => tracing::warn!(%error, "shared media failed"),
        })
        .detach();
    }
}

impl ListBox {
    fn open_message(&self, index: usize, window: &mut Window, cx: &mut App) {
        let Items::Messages(media, messages) = &self.items else {
            return;
        };
        let Some(message) = messages.get(index) else {
            return;
        };
        if matches!(media, SharedMedia::Photo | SharedMedia::Video) {
            // The viewer pages oldest to newest, as the chat does.
            let entries: Vec<MediaEntry> = messages
                .iter()
                .rev()
                .filter_map(|m| MediaEntry::of(m, String::new(), date(m.date)))
                .collect();
            if let Some(at) = entries.iter().position(|entry| entry.message == message.id) {
                let store = self.targets.store.clone();
                open_media(store, &self.targets.layers, entries, at, window, cx);
                return;
            }
        }
        let (chat, id) = (self.chat, message.id);
        self.targets
            .layers
            .update(cx, |layers, cx| layers.close_all(window, cx));
        self.targets
            .dialogs
            .update(cx, |_, cx| cx.emit(OpenMessage(chat, id)));
    }

    fn open_chat(&self, chat: ChatId, window: &mut Window, cx: &mut App) {
        self.targets
            .layers
            .update(cx, |layers, cx| layers.close_all(window, cx));
        self.targets
            .dialogs
            .update(cx, |dialogs, cx| dialogs.select(chat, cx));
    }

    fn message_rows(&self, messages: &[Message], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let strings = strings(cx);
        let hover = color(cx, ColorRef::Palette(ColorId::WindowBgOver));
        messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let line = preview::content(strings, &message.content)
                    .0
                    .hiding_spoilers();
                let entity = cx.entity();
                div()
                    .id(("info-list-message", index))
                    .px(px_of(ROW_PADDING.0))
                    .py(px_of(ROW_PADDING.1))
                    .cursor_pointer()
                    .hover(move |this| this.bg(hover))
                    .child(text(line, &INFO_LABELED, cx).truncate())
                    .child(text(date(message.date), &INFO_LABEL, cx))
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| this.open_message(index, window, cx));
                    })
                    .into_any_element()
            })
            .collect()
    }

    fn chat_rows(&self, chats: &[ChatId], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = self.targets.store.read(cx).state();
        chats
            .iter()
            .filter_map(|id| state.chats.chat(*id))
            .enumerate()
            .map(|(index, chat)| {
                let (userpic, _) = photo_userpic(
                    &state.chats,
                    chat.photo.as_ref(),
                    chat.accent_color,
                    &chat.title,
                );
                let look = Look {
                    name: chat.title.clone().into(),
                    userpic,
                    status: crate::status::members_status(chat, None, cx)
                        .unwrap_or_default()
                        .into(),
                    is_active: false,
                };
                let (id, entity) = (chat.id, cx.entity());
                peer_list::row(
                    ("info-list-chat", index),
                    look,
                    self.hovered == Some(index),
                    cx,
                )
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    let next = peer_list::hovered_after(this.hovered, index, *hovered);
                    if this.hovered != next {
                        this.hovered = next;
                        cx.notify();
                    }
                }))
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.open_chat(id, window, cx))
                })
                .into_any_element()
            })
            .collect()
    }
}

// shortcut: day.month.year in every language; use the chat's date badges when the list gets month headers.
fn date(unix_time: i32) -> String {
    Local
        .timestamp_opt(i64::from(unix_time), 0)
        .single()
        .map(|date| date.format("%d.%m.%Y").to_string())
        .unwrap_or_default()
}

impl Render for ListBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = match &self.items {
            Items::Messages(_, messages) => {
                let messages = messages.clone();
                self.message_rows(&messages, cx)
            }
            Items::Chats(chats) => {
                let chats = chats.clone();
                self.chat_rows(&chats, cx)
            }
        };
        let max_height = window.viewport_size().height * 0.6 - px_of(INFO_LAYER_TOP_MINIMAL);
        let layers = self.targets.layers.clone();
        div().track_focus(&self.focus).child(
            BoxFrame::new("info-list", &DEFAULT_BOX)
                .width(px_of(BOX_WIDE_WIDTH))
                .title(self.title.clone())
                .row(
                    div()
                        .id("info-list-scroll")
                        .max_h(max_height)
                        .overflow_y_scroll()
                        .children(rows),
                )
                .button(
                    RoundButton::new(
                        "info-list-close",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_close),
                    )
                    .on_click(move |_, window, cx| {
                        layers.update(cx, |layers, cx| layers.close(window, cx));
                    }),
                ),
        )
    }
}
