//! The message field under the history (`HistoryWidget::moveFieldControls`):
//! attach on the left, the field, then emoji and send on the right; send
//! shows a microphone while the field is empty.

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement as _, ParentElement as _,
    SharedString, Styled as _, Subscription, Window, div, px,
};
use i18n::key;
use model::chat::ChatKind;
use model::thread::Key;
use style::chat_helpers::{
    HISTORY_ATTACH, HISTORY_ATTACH_EMOJI, HISTORY_COMPOSE_FIELD, HISTORY_COMPOSE_FIELD_MAX_HEIGHT,
    HISTORY_SEND, HISTORY_SEND_PADDING, HISTORY_SEND_RIGHT, HISTORY_SEND_SIZE,
};
use style::widgets::IconButton as IconButtonStyle;
use style::{ColorId, ColorRef, Icon, IconMask, Margins, MonoIcon};
use ui::{IconButton, TextArea, TextAreaEvent, color};

use super::HistoryView;
use crate::lang::tr;
use crate::px_of;
use crate::store::spawn;

/// Around the text in the field (`InitMessageFieldGeometry`).
const FIELD_MARGIN: f32 = 4.0;

/// The send button's microphone (`SendButton::paintRecord`), drawn from
/// tdesktop's icon where tdesktop plays a Lottie animation.
// shortcut: a still icon with no voice-to-round morph; the Lottie comes with voice recording (M4).
static HISTORY_RECORD: IconButtonStyle = IconButtonStyle {
    icon: Icon::new(&[MonoIcon::new(
        IconMask::File {
            name: "chat/input_record",
            modifiers: &[],
        },
        ColorRef::Palette(ColorId::HistoryComposeIconFg),
        Margins::new(0, 0, 0, 0),
    )]),
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
    _subscription: Subscription,
}

impl Compose {
    pub(super) fn new(
        placeholder: SharedString,
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
            field
        });
        let subscription = cx.subscribe(&field, |this, _, event, cx| match event {
            TextAreaEvent::Submitted => this.send(cx),
            // The send button follows whether there is text.
            TextAreaEvent::Changed => cx.notify(),
            TextAreaEvent::Focused | TextAreaEvent::Blurred => {}
        });
        Self {
            field,
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

impl HistoryView {
    /// Sends the field's text and empties it; puts the text back if TDLib
    /// refused it before making a message of it.
    fn send(&mut self, cx: &mut Context<Self>) {
        let field = self.compose.field.clone();
        let text = field.read(cx).text().to_owned();
        if text.trim().is_empty() {
            return;
        }
        field.update(cx, |field, cx| field.set_text("", cx));
        let task = spawn(
            self.store
                .read(cx)
                .session()
                .send_text(self.chat, self.topic, &text),
            cx,
        );
        self.scroll_down(cx);
        cx.spawn(async move |_, cx| {
            let Err(error) = task.await else {
                return;
            };
            tracing::warn!(%error, "sending a message failed");
            cx.update(|cx| {
                field.update(cx, |field, cx| {
                    if field.text().is_empty() {
                        field.set_text(&text, cx);
                    }
                });
            });
        })
        .detach();
    }

    pub(super) fn compose_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let field = self.compose.field.clone();
        let is_empty = field.read(cx).text().is_empty();
        let send = if is_empty {
            // shortcut: shows the microphone but records nothing; voice recording comes later in M4.
            IconButton::new("compose-record", &HISTORY_RECORD)
        } else {
            let weak = cx.weak_entity();
            IconButton::new("compose-send", &HISTORY_SEND.inner).on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.send(cx));
            })
        };
        let side = px_of(HISTORY_SEND_RIGHT);
        div()
            .flex_none()
            .flex()
            .items_end()
            .px(side)
            .bg(color(cx, ColorRef::Palette(ColorId::HistoryReplyBg)))
            // shortcut: attach and emoji do nothing yet; files come later in M4, the emoji panel in M5.
            .child(IconButton::new("compose-attach", &HISTORY_ATTACH))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .py(px_of(HISTORY_SEND_PADDING))
                    .child(field),
            )
            .child(IconButton::new(
                "compose-emoji",
                &HISTORY_ATTACH_EMOJI.inner,
            ))
            .child(send)
            .into_any_element()
    }
}
