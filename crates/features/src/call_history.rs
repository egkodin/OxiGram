//! Account-scoped call history from TDLib's call-message search.

use std::rc::Rc;

use chrono::Local;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, Window, div, prelude::FluentBuilder as _, px,
};
use i18n::{Key, key};
use model::message::{Content, Message};
use model::{ChatId, MessageId};
use style::layers::DEFAULT_BOX;
use style::{ColorId, ColorRef};
use ui::{BoxFrame, RoundButton, color};

use crate::Store;
use crate::history::{duration_text, layout};
use crate::lang::{strings, tr, tr_with};
use crate::layers::{Dismiss, Layers};

type OpenMessage = Rc<dyn Fn(ChatId, MessageId, &mut Window, &mut App)>;

enum Page {
    Ready(String),
    Loading,
    Failed(String),
    End,
}

struct Calls {
    store: Entity<Store>,
    layers: WeakEntity<Layers>,
    focus: FocusHandle,
    messages: Vec<Message>,
    page: Page,
    open_message: OpenMessage,
}

pub(crate) fn show(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    open_message: OpenMessage,
    window: &mut Window,
    cx: &mut App,
) {
    let calls = cx.new(|cx| {
        let mut calls = Calls {
            store: store.clone(),
            layers: layers.downgrade(),
            focus: cx.focus_handle(),
            messages: Vec::new(),
            page: Page::Ready(String::new()),
            open_message,
        };
        calls.load(cx);
        calls
    });
    layers.update(cx, |layers, cx| {
        layers.show(calls, Dismiss::Allowed, None, window, cx);
    });
}

impl Focusable for Calls {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Calls {
    fn load(&mut self, cx: &mut Context<Self>) {
        let (Page::Ready(offset) | Page::Failed(offset)) = &self.page else {
            return;
        };
        let offset = offset.clone();
        self.page = Page::Loading;
        let task = crate::store::spawn(self.store.read(cx).session().search_calls(&offset, 50), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.page = match result {
                    Ok(page) => {
                        this.messages.extend(
                            page.messages
                                .into_iter()
                                .filter(|message| matches!(message.content, Content::Call { .. })),
                        );
                        match page.next_offset.filter(|next| *next != offset) {
                            Some(next) => Page::Ready(next),
                            None => Page::End,
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "loading calls failed");
                        Page::Failed(offset)
                    }
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn close(&self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }
}

fn call_label(is_group: bool, is_video: bool, outgoing: bool, missed: bool) -> Key {
    match (is_group, is_video, outgoing, missed) {
        (true, _, _, true) => key::lng_call_group_missed,
        (true, _, true, false) => key::lng_call_group_outgoing,
        (true, _, false, false) => key::lng_call_group_incoming,
        (false, true, _, true) => key::lng_call_video_missed,
        (false, true, true, false) => key::lng_call_video_outgoing,
        (false, true, false, false) => key::lng_call_video_incoming,
        (false, false, _, true) => key::lng_call_missed,
        (false, false, true, false) => key::lng_call_outgoing,
        (false, false, false, false) => key::lng_call_incoming,
    }
}

impl Render for Calls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Local::now();
        let chats = &self.store.read(cx).state().chats;
        let secondary = color(cx, ColorRef::Palette(ColorId::WindowSubTextFg));
        let hover = color(cx, ColorRef::Palette(ColorId::WindowBgOver));
        let rows = self.messages.iter().filter_map(|message| {
            let Content::Call {
                is_video,
                is_group,
                is_missed,
                duration,
            } = message.content
            else {
                return None;
            };
            let (chat, id) = (message.chat, message.id);
            let kind = tr(
                cx,
                call_label(is_group, is_video, message.is_outgoing, is_missed),
            );
            let date = layout::date_time_full(strings(cx), message.date, &now);
            let date = if duration > 0 {
                tr_with(
                    cx,
                    key::lng_call_duration_info,
                    &[("time", &date), ("duration", &duration_text(duration))],
                )
            } else {
                date
            };
            let (title, detail) = match chats.chat(chat).filter(|chat| !chat.title.is_empty()) {
                Some(chat) => (
                    gpui::SharedString::from(chat.title.clone()),
                    format!("{kind} · {date}"),
                ),
                None => (kind, date),
            };
            let open = self.open_message.clone();
            let layers = self.layers.clone();
            Some(
                div()
                    .id(gpui::SharedString::from(format!(
                        "call-{}-{}",
                        chat.0, id.0
                    )))
                    .px(px(24.0))
                    .py(px(10.0))
                    .cursor_pointer()
                    .hover(move |row| row.bg(hover))
                    .on_click(move |_, window, cx| {
                        let _ = layers.update(cx, |layers, cx| layers.close(window, cx));
                        open(chat, id, window, cx);
                    })
                    .child(div().truncate().child(title))
                    .child(div().text_sm().text_color(secondary).child(detail)),
            )
        });
        let status = match self.page {
            Page::Loading => Some(key::lng_contacts_loading),
            Page::Failed(_) => Some(key::lng_passkey_cable_error_title),
            Page::End if self.messages.is_empty() => Some(key::lng_call_box_about),
            _ => None,
        };
        let more = match self.page {
            Page::Ready(_) => Some(key::lng_view_button_full_article),
            Page::Failed(_) => Some(key::lng_bot_download_retry),
            _ => None,
        };
        BoxFrame::new("call-history", &DEFAULT_BOX)
            .title(tr(cx, key::lng_call_box_title))
            .row(div().children(rows).when_some(status, |body, key| {
                body.child(div().p(px(24.0)).text_color(secondary).child(tr(cx, key)))
            }))
            .button(
                RoundButton::new("calls-close", &DEFAULT_BOX.button, tr(cx, key::lng_close))
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            )
            .when_some(more, |frame, key| {
                frame.left_button(
                    RoundButton::new("calls-more", &DEFAULT_BOX.button, tr(cx, key))
                        .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_incoming_video_outgoing_voice_and_missed_group_calls() {
        assert_eq!(
            call_label(false, true, false, false),
            key::lng_call_video_incoming
        );
        assert_eq!(
            call_label(false, false, true, false),
            key::lng_call_outgoing
        );
        assert_eq!(
            call_label(true, true, false, true),
            key::lng_call_group_missed
        );
    }
}
