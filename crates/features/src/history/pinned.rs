//! The pinned message bar over a chat (`HistoryWidget::_pinnedBar`,
//! `PinnedTracker`): it shows the newest pin at or above the bottom of the
//! view, and each click jumps to it and moves on to the one before.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use i18n::key;
use model::MessageId;
use style::chat::{MSG_REPLY_BAR_POS, MSG_REPLY_BAR_SIZE, MSG_REPLY_BAR_SKIP, MSG_REPLY_PADDING};
use style::chat_helpers::{DEFAULT_MESSAGE_BAR, HISTORY_REPLY_CANCEL, HISTORY_REPLY_HEIGHT};
use style::{ColorId, ColorRef, Font};
use ui::{IconButton, color, font, font_height};

use super::HistoryView;
use super::headers::Replied;
use crate::boxes::{self, ConfirmArgs};
use crate::dialogs::content_preview;
use crate::lang::{strings, tr};
use crate::peer_menu::send;
use crate::px_of;
use crate::store::spawn;

/// The bar shows at most this many segments (`MessageBar`).
const MAX_SEGMENTS: usize = 4;
/// The gap between segments of the bar.
const SEGMENT_GAP: f32 = 2.0;
/// Inactive segments are this much fainter than the active one.
const INACTIVE_SEGMENT_ALPHA: f32 = 1.0 / 3.0;

/// Where the bar stands in the chat's pins.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Pins {
    /// The pin the user clicked last; the bar then shows the one before it.
    clicked: Option<MessageId>,
    /// The last message at or above the bottom of the view.
    bottom: Option<MessageId>,
    /// The newest pin when the user hid the bar; it shows again for a new one.
    hidden: Option<MessageId>,
}

/// The pin the bar shows: its id, place among the pins (oldest first) and
/// how many there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Shown {
    pub id: MessageId,
    pub index: usize,
    pub count: usize,
}

/// The newest pin before both the bottom of the view and the last clicked
/// pin, else the oldest one (`HistoryWidget::updatePinnedViewer`).
pub(super) fn current(
    pinned: &[MessageId],
    bottom: Option<MessageId>,
    clicked: Option<MessageId>,
) -> Option<Shown> {
    let oldest = *pinned.first()?;
    let limit = if clicked == Some(oldest) {
        // After the oldest pin the bar starts over from the newest.
        None
    } else {
        [bottom.map(|id| MessageId(id.0 + 1)), clicked]
            .into_iter()
            .flatten()
            .min()
    };
    let index = limit.map_or(pinned.len() - 1, |limit| {
        pinned.partition_point(|&id| id < limit).saturating_sub(1)
    });
    Some(Shown {
        id: pinned[index],
        index,
        count: pinned.len(),
    })
}

/// The bar's title (`PinnedBarTitle`).
fn title(shown: Shown, cx: &App) -> SharedString {
    if shown.index + 1 >= shown.count {
        tr(cx, key::lng_pinned_message)
    } else if shown.count == 2 {
        tr(cx, key::lng_pinned_previous)
    } else {
        format!("{} #{}", tr(cx, key::lng_pinned_message), shown.index + 1).into()
    }
}

/// The first segment shown and the active one's place among those shown.
// shortcut: a window that follows the active pin without tdesktop's scrolling animation; the animation comes with M8 polish.
fn segments(shown: Shown) -> (usize, usize) {
    let visible = shown.count.min(MAX_SEGMENTS);
    let first = shown
        .index
        .saturating_sub(visible / 2)
        .min(shown.count - visible);
    (visible, shown.index - first)
}

impl HistoryView {
    pub(super) fn load_pins(&self, cx: &mut Context<Self>) {
        let chat = self.chat;
        let task = spawn(self.store.read(cx).session().pinned_messages(chat), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(messages) => this
                    .store
                    .update(cx, |store, cx| store.set_pinned(chat, messages, cx)),
                Err(error) => tracing::warn!(%error, "loading pinned messages failed"),
            });
        })
        .detach();
    }

    /// A scroll by the user; scrolling down lets the view lead again
    /// (`checkLastPinnedClickedIdReset`).
    pub(super) fn on_scroll_pins(&mut self, cx: &mut Context<Self>) {
        let was = self.pins.bottom;
        self.update_pins(cx);
        if self.pins.clicked.is_some() && self.pins.bottom > was {
            self.pins.clicked = None;
            cx.notify();
        }
    }

    /// Follows the view: the bar shows the pin above its bottom.
    pub(super) fn update_pins(&mut self, cx: &mut Context<Self>) {
        let top = self.list.logical_scroll_top().item_ix;
        let bottom = (top..self.shown.len())
            .take_while(|&index| self.list.item_is_below_viewport(index) == Some(false))
            .last()
            .map(|index| self.shown[index].id);
        let mut pins = Pins {
            bottom,
            ..self.pins
        };
        // Scrolled above the clicked pin: the view leads again.
        if pins
            .clicked
            .is_some_and(|clicked| bottom.is_some_and(|b| b < clicked))
        {
            pins.clicked = None;
        }
        if (pins.bottom, pins.clicked) != (self.pins.bottom, self.pins.clicked) {
            self.pins = pins;
            cx.notify();
        }
    }

    fn open_pin(&mut self, id: MessageId, cx: &mut Context<Self>) {
        self.pins.clicked = Some(id);
        self.jump_to(id, cx);
        cx.notify();
    }

    /// Offers to unpin when the user may, else hides the bar
    /// (`HistoryWidget::hidePinnedMessage`).
    fn close_pin(&mut self, id: MessageId, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        let task = spawn(session.message_abilities(self.chat, id), cx);
        let (chat, layers) = (self.chat, self.layers.clone());
        cx.spawn_in(window, async move |this, cx| {
            let can_unpin = match task.await {
                Ok(abilities) => abilities.can_be_pinned,
                Err(error) => {
                    tracing::warn!(%error, "message abilities unknown");
                    false
                }
            };
            this.update_in(cx, |this, window, cx| {
                if !can_unpin {
                    // shortcut: hidden until rgram restarts; tdesktop keeps it in the session settings, which come with M7.
                    this.pins.hidden = Some(id);
                    cx.notify();
                    return;
                }
                let mut args = ConfirmArgs::new(ui::Markup::plain(
                    tr(cx, key::lng_pinned_unpin_sure).to_string(),
                ));
                args.confirm_text = Some(tr(cx, key::lng_pinned_unpin));
                args.cancel_text = Some(tr(cx, key::lng_cancel));
                args.confirmed = Some(Rc::new(move |_, cx| {
                    send(session.unpin_message(chat, id), "unpin", cx);
                }));
                boxes::confirm(&layers, args, window, cx);
            })
        })
        .detach();
    }

    /// The bar, while the chat has pins the user did not hide.
    // shortcut: no "show all" button with several pins, which opens the pinned section (M3, later).
    pub(super) fn pinned_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let history = self.store.read(cx).state().history(self.chat)?;
        let newest = history.pinned().last().copied();
        if newest.is_some() && newest == self.pins.hidden {
            return None;
        }
        let shown = current(history.pinned(), self.pins.bottom, self.pins.clicked)?;
        self.ensure_reference(self.chat, shown.id, cx);
        let state = self.store.read(cx).state();
        let text: SharedString = match self.replied(state, self.chat, shown.id) {
            Replied::Loaded(message) => {
                content_preview(strings(cx), &message.content).0.text.into()
            }
            Replied::Loading => tr(cx, key::lng_profile_loading),
            Replied::Deleted => tr(cx, key::lng_deleted_message),
        };
        let st = &DEFAULT_MESSAGE_BAR;
        let body = div()
            .flex_1()
            .min_w_0()
            .pl(px_of(MSG_REPLY_BAR_SKIP) * 2.0)
            .pr(px_of(MSG_REPLY_PADDING.right))
            .pt(px_of(MSG_REPLY_PADDING.top))
            .child(line(title(shown, cx), st.title.font, st.title_fg, cx))
            .child(line(text, st.text.font, st.text_fg, cx));
        let weak = cx.weak_entity();
        let close = (shown.count == 1).then(|| {
            let weak = weak.clone();
            IconButton::new("history-pinned-close", &HISTORY_REPLY_CANCEL).on_click(
                move |_, window, cx| {
                    cx.stop_propagation();
                    let _ = weak.update(cx, |this, cx| this.close_pin(shown.id, window, cx));
                },
            )
        });
        Some(
            div()
                .id("history-pinned")
                .relative()
                .flex_none()
                .flex()
                .h(px_of(HISTORY_REPLY_HEIGHT))
                .bg(color(cx, ColorRef::Palette(ColorId::HistoryPinnedBg)))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.open_pin(shown.id, cx));
                })
                .child(bar(shown, cx))
                .child(body)
                .children(close)
                .into_any_element(),
        )
    }
}

fn line(value: SharedString, text_font: Font, fg: ColorRef, cx: &App) -> impl IntoElement {
    div()
        .truncate()
        .font(font(text_font))
        .text_size(px_of(text_font.size))
        .line_height(font_height(text_font))
        .text_color(color(cx, fg))
        .child(value)
}

/// The bar on the left, a segment per pin (up to four), the shown one
/// brighter.
fn bar(shown: Shown, cx: &App) -> impl IntoElement {
    let (visible, active) = segments(shown);
    let width = px_of(MSG_REPLY_BAR_SIZE.width);
    let height = px_of(MSG_REPLY_BAR_SIZE.height);
    #[expect(clippy::cast_precision_loss, reason = "at most four segments")]
    let count = visible as f32;
    let segment = (height - px(SEGMENT_GAP) * (count - 1.0)) / count;
    let fg = color(cx, ColorRef::Palette(ColorId::MsgInReplyBarColor));
    div()
        .absolute()
        .left(px_of(MSG_REPLY_BAR_SKIP) + px_of(MSG_REPLY_BAR_POS.x))
        .top(px_of(MSG_REPLY_PADDING.top) + px_of(MSG_REPLY_BAR_POS.y))
        .w(width)
        .flex()
        .flex_col()
        .gap(px(SEGMENT_GAP))
        .children((0..visible).map(|index| {
            let alpha = if index == active {
                1.0
            } else {
                INACTIVE_SEGMENT_ALPHA
            };
            div()
                .w(width)
                .h(segment)
                .rounded(width / 2.0)
                .bg(fg.opacity(alpha))
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINS: [MessageId; 3] = [MessageId(10), MessageId(20), MessageId(30)];

    fn shown_id(bottom: Option<i64>, clicked: Option<i64>) -> Option<i64> {
        current(&PINS, bottom.map(MessageId), clicked.map(MessageId)).map(|s| s.id.0)
    }

    #[test]
    fn shows_the_newest_pin_at_or_above_the_view_bottom() {
        assert_eq!(shown_id(None, None), Some(30));
        assert_eq!(shown_id(Some(25), None), Some(20));
        assert_eq!(shown_id(Some(20), None), Some(20));
        assert_eq!(
            shown_id(Some(5), None),
            Some(10),
            "above all pins: the oldest"
        );
        assert_eq!(current(&[], None, None), None);
    }

    #[test]
    fn clicks_step_to_older_pins_and_wrap_after_the_oldest() {
        assert_eq!(shown_id(Some(40), Some(30)), Some(20));
        assert_eq!(shown_id(Some(40), Some(20)), Some(10));
        assert_eq!(shown_id(Some(15), Some(10)), Some(30));
    }

    #[test]
    fn the_active_segment_follows_the_shown_pin() {
        let shown = |index, count| Shown {
            id: MessageId(1),
            index,
            count,
        };
        assert_eq!(segments(shown(0, 1)), (1, 0));
        assert_eq!(segments(shown(2, 3)), (3, 2));
        assert_eq!(segments(shown(9, 10)), (4, 3));
        assert_eq!(segments(shown(5, 10)), (4, 2));
        assert_eq!(segments(shown(0, 10)), (4, 0));
    }
}
