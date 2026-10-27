//! The message field under the history (`HistoryWidget::moveFieldControls`):
//! attach on the left, the field, then emoji and send on the right; send
//! shows a microphone while the field is empty.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement as _,
    MouseButton, ParentElement as _, SharedString, Styled as _, Subscription, Window, div, px,
};
use i18n::key;
use model::chat::ChatKind;
use model::send::SendOptions;
use model::thread::Key;
use style::chat_helpers::{
    HISTORY_ATTACH, HISTORY_COMPOSE_FIELD, HISTORY_COMPOSE_FIELD_MAX_HEIGHT, HISTORY_SEND,
    HISTORY_SEND_PADDING, HISTORY_SEND_RIGHT, HISTORY_SEND_SIZE,
};
use style::widgets::IconButton as IconButtonStyle;
use style::{ColorId, ColorRef, Icon, IconMask, Margins, MonoIcon};
use ui::{IconButton, TextArea, TextAreaEvent, color};

use super::HistoryView;
use super::drafts::Draft;
use super::field_bar::{self, FieldBar};
use super::send_menu::OnOptions;
use crate::lang::tr;
use crate::px_of;

/// Around the text in the field (`InitMessageFieldGeometry`).
const FIELD_MARGIN: f32 = 4.0;

/// The send button's microphone (`SendButton::paintRecord`), drawn from
/// tdesktop's icon where tdesktop plays a Lottie animation.
// shortcut: a still icon with no voice-to-round morph; round video messages come with M5.
pub(super) const RECORD_ICON: Icon = Icon::new(&[MonoIcon::new(
    IconMask::File {
        name: "chat/input_record",
        modifiers: &[],
    },
    ColorRef::Palette(ColorId::HistoryComposeIconFg),
    Margins::new(0, 0, 0, 0),
)]);

static HISTORY_RECORD: IconButtonStyle = IconButtonStyle {
    icon: RECORD_ICON,
    icon_over: Icon::new(&[MonoIcon::new(
        IconMask::File {
            name: "chat/input_record",
            modifiers: &[],
        },
        ColorRef::Palette(ColorId::HistoryComposeIconFgOver),
        Margins::new(0, 0, 0, 0),
    )]),
    ..HISTORY_SEND.inner
};

pub(super) struct Compose {
    pub(super) field: Entity<TextArea>,
    pub(super) bar: Option<FieldBar>,
    pub(super) draft: Draft,
    _subscription: Subscription,
}

impl Compose {
    pub(super) fn new(
        placeholder: SharedString,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<HistoryView>,
    ) -> Self {
        let min_height = px_of(HISTORY_SEND_SIZE.height) - px_of(HISTORY_SEND_PADDING) * 2.0;
        let field = cx.new(|cx| {
            let mut field = TextArea::new(&HISTORY_COMPOSE_FIELD, placeholder, window, cx)
                .with_geometry(
                    min_height,
                    px_of(HISTORY_COMPOSE_FIELD_MAX_HEIGHT),
                    px(FIELD_MARGIN),
                );
            field.set_submit_key(crate::settings::send_by(cx).submit_key());
            field.set_text(draft.text(), cx);
            field
        });
        let subscription = cx.subscribe(&field, |this, _, event, cx| match event {
            TextAreaEvent::Submitted => this.send(SendOptions::default(), cx),
            TextAreaEvent::Changed => {
                this.draft_changed(cx);
                // The send button follows whether there is text.
                cx.notify();
            }
            TextAreaEvent::Focused | TextAreaEvent::Blurred => {}
        });
        Self {
            field,
            bar: None,
            draft,
            _subscription: subscription,
        }
    }
}

/// "Broadcast a message..." in a channel, else "Write a message..."
/// (`HistoryWidget::updateFieldPlaceholder`).
pub(super) fn placeholder(state: &model::State, key: Key, cx: &App) -> SharedString {
    let chat = state.chats.chat(key.chat);
    let key = match chat.map(|chat| chat.kind) {
        Some(ChatKind::Supergroup {
            is_channel: true, ..
        }) if key.topic.is_none() => key::lng_broadcast_ph,
        _ => key::lng_message_ph,
    };
    tr(cx, key)
}

impl Compose {
    /// What the draft holds: the field, or while editing what the field
    /// held before.
    pub(super) fn draft_text(&self, cx: &App) -> String {
        match &self.bar {
            Some(FieldBar::Edit { stash, .. }) => stash.clone(),
            _ => self.field.read(cx).text().to_owned(),
        }
    }
}

impl HistoryView {
    /// Sends the field's text, as a reply if one is open, with the
    /// forward after it; while editing, saves the edit instead.
    fn send(&mut self, options: SendOptions, cx: &mut Context<Self>) {
        let text = self.compose.field.read(cx).text().to_owned();
        let is_blank = text.trim().is_empty();
        match self.compose.bar {
            Some(FieldBar::Edit { message, .. }) if !is_blank => self.save_edit(message, text, cx),
            Some(FieldBar::Edit { .. }) => {}
            Some(FieldBar::Forward(_)) => self.send_with_bar(text, options, cx),
            Some(FieldBar::Reply(_)) | None if !is_blank => self.send_with_bar(text, options, cx),
            Some(FieldBar::Reply(_)) | None => {}
        }
    }

    /// The field with its buttons, or the bar that replaces it.
    pub(super) fn compose_bar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let writing = self.store.read(cx).state().chats.writing(self.chat);
        if let Some(bar) = self.write_bar(writing, cx) {
            return bar;
        }
        let row = match self.voice_bar(window, cx) {
            Some(bar) => bar,
            None => self.field_row(cx),
        };
        div()
            .flex_none()
            .flex()
            .flex_col()
            .key_context(field_bar::CONTEXT)
            .on_action(cx.listener(Self::edit_last))
            .on_action(cx.listener(Self::reply_previous))
            .on_action(cx.listener(Self::reply_next))
            // Files and images in the clipboard go to the send box, text to the field.
            .capture_action(cx.listener(|this, _: &ui::Paste, window, cx| {
                if this.paste_files(window, cx) {
                    cx.stop_propagation();
                }
            }))
            .bg(color(cx, ColorRef::Palette(ColorId::HistoryReplyBg)))
            .children(self.field_bar(cx))
            .child(row)
            .into_any_element()
    }

    /// Attach, the field, emoji, and send or the microphone.
    fn field_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let field = self.compose.field.clone();
        let is_forwarding = matches!(self.compose.bar, Some(FieldBar::Forward(_)));
        let send = if field.read(cx).text().is_empty() && !is_forwarding {
            self.record_button(IconButton::new("compose-record", &HISTORY_RECORD), cx)
        } else {
            self.send_button(cx)
        };
        let weak = cx.weak_entity();
        div()
            .flex()
            .items_end()
            .px(px_of(HISTORY_SEND_RIGHT))
            .child(IconButton::new("compose-attach", &HISTORY_ATTACH).on_click(
                move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.choose_files(window, cx));
                },
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .py(px_of(HISTORY_SEND_PADDING))
                    .child(field),
            )
            .children(self.scheduled_button(cx))
            .child(self.emoji_button(cx))
            .child(send)
            .into_any_element()
    }

    /// Send, with the send menu on a right click unless an edit is open.
    fn send_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        let button =
            IconButton::new("compose-send", &HISTORY_SEND.inner).on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.send(SendOptions::default(), cx));
            });
        if matches!(self.compose.bar, Some(FieldBar::Edit { .. })) {
            return button.into_any_element();
        }
        let weak = cx.weak_entity();
        div()
            .child(button)
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                let sender = weak.clone();
                let on_send: OnOptions = Rc::new(move |options, _, cx| {
                    let _ = sender.update(cx, |this, cx| this.send(options, cx));
                });
                let _ = weak.update(cx, |this, cx| {
                    this.show_send_menu(on_send, event.position, window, cx);
                });
            })
            .into_any_element()
    }
}
