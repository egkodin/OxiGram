//! The "Unread messages" bar (`UnreadBar`) and the corner buttons: down to
//! the newest message with the unread count (`HistoryDownButton`), and to
//! unread mentions and reactions.

use gpui::{
    AnyElement, App, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement,
    Pixels, SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use i18n::key;
use model::history::Unread;
use style::chat::{HISTORY_UNREAD_BAR_FONT, HISTORY_UNREAD_BAR_HEIGHT, HISTORY_UNREAD_BAR_MARGIN};
use style::chat_helpers::{
    HISTORY_TO_DOWN, HISTORY_TO_DOWN_POSITION, HISTORY_UNREAD_MENTIONS, HISTORY_UNREAD_REACTIONS,
    HISTORY_UNREAD_THINGS_SKIP,
};
use style::widgets::TwoIconButton;
use style::{ColorId, ColorRef};
use ui::{color, font, font_height, icon};

use super::HistoryView;
use crate::dialogs::{Lit, unread_badge};
use crate::lang::tr;
use crate::peer_menu::send;
use crate::px_of;
use crate::store::spawn;

/// The bar over the first unread message, across the whole list.
pub(super) fn bar(cx: &App) -> AnyElement {
    let line = px(1.0);
    let palette = |id| color(cx, ColorRef::Palette(id));
    div()
        .w_full()
        .mt(line + px_of(HISTORY_UNREAD_BAR_MARGIN))
        .h(px_of(HISTORY_UNREAD_BAR_HEIGHT) - line)
        .flex()
        .items_center()
        .justify_center()
        .bg(palette(ColorId::HistoryUnreadBarBg))
        .border_b(line)
        .border_color(palette(ColorId::HistoryUnreadBarBorder))
        .font(font(HISTORY_UNREAD_BAR_FONT))
        .text_size(px_of(HISTORY_UNREAD_BAR_FONT.size))
        .line_height(font_height(HISTORY_UNREAD_BAR_FONT))
        .text_color(palette(ColorId::HistoryUnreadBarFg))
        .child(tr(cx, key::lng_unread_bar_some))
        .into_any_element()
}

/// A round button at the bottom right (`CornerButtons`); the badge counts
/// unread messages, mentions or reactions.
// shortcut: the badge uses the chat list's 20 px pill; tdesktop draws it 22 px high (`historyToDownBadgeSize`).
fn corner_button(
    id: &'static str,
    st: &'static TwoIconButton,
    bottom: Pixels,
    unread: Option<(SharedString, bool)>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let layer = |normal, over| {
        div()
            .absolute()
            .top(px_of(st.icon_position.y))
            .left(px_of(st.icon_position.x))
            .child(
                div()
                    .group_hover(id, |s| s.opacity(0.0))
                    .child(icon(normal)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .opacity(0.0)
                    .group_hover(id, |s| s.opacity(1.0))
                    .child(icon(over)),
            )
    };
    div()
        .id(id)
        .group(id)
        .absolute()
        .right(px_of(HISTORY_TO_DOWN_POSITION.x))
        .bottom(bottom)
        .w(px_of(st.width))
        .h(px_of(st.height))
        .cursor_pointer()
        .child(layer(st.icon_below, st.icon_below_over))
        .child(layer(st.icon_above, st.icon_above_over))
        .children(unread.map(|(count, is_muted)| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(unread_badge(count, is_muted, Lit::Normal, cx))
        }))
        .on_click(on_click)
        .into_any_element()
}

/// The badge text for a count, none for zero.
fn badge(count: i32, is_muted: bool) -> Option<(SharedString, bool)> {
    (count > 0).then(|| (count.to_string().into(), is_muted))
}

impl HistoryView {
    /// The down button, and over it the unread mentions and reactions
    /// buttons while there are some (`CornerButtons::updatePositions`).
    pub(super) fn corner_buttons(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let counts = self
            .store
            .read(cx)
            .state()
            .chats
            .chat(self.chat)
            // shortcut: a thread shows no unread counts, the chat's are not its own; with per-thread counts (M4).
            .filter(|_| self.topic.is_none())
            .map(|chat| {
                (
                    chat.unread_count,
                    chat.is_muted(),
                    chat.unread_mentions,
                    chat.unread_reactions,
                )
            });
        let (unread, is_muted, mentions, reactions) = counts.unwrap_or_default();
        let mut buttons = Vec::new();
        let mut bottom = px_of(HISTORY_TO_DOWN_POSITION.y);
        if self.is_down_shown {
            let weak = cx.weak_entity();
            buttons.push(corner_button(
                "history-down",
                &HISTORY_TO_DOWN,
                bottom,
                badge(unread, is_muted),
                move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.scroll_down(cx));
                },
                cx,
            ));
            bottom += px_of(HISTORY_TO_DOWN.height) + px_of(HISTORY_UNREAD_THINGS_SKIP);
        }
        for (id, st, count, kind) in [
            (
                "history-mentions",
                &HISTORY_UNREAD_MENTIONS,
                mentions,
                Unread::Mentions,
            ),
            (
                "history-reactions",
                &HISTORY_UNREAD_REACTIONS,
                reactions,
                Unread::Reactions,
            ),
        ] {
            if count <= 0 {
                continue;
            }
            let weak = cx.weak_entity();
            buttons.push(corner_button(
                id,
                st,
                bottom,
                badge(count, false),
                move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.open_unread(kind, cx));
                },
                cx,
            ));
            bottom += px_of(st.height) + px_of(HISTORY_UNREAD_THINGS_SKIP);
        }
        buttons
    }

    /// Jumps to the oldest unread mention or reaction and reads it
    /// (`showNextUnreadMention`, `showNextUnreadReaction`).
    fn open_unread(&mut self, kind: Unread, cx: &mut Context<Self>) {
        let chat = self.chat;
        let session = self.store.read(cx).session().clone();
        let task = spawn(session.unread_messages(chat, kind), cx);
        cx.spawn(async move |this, cx| {
            let oldest = match task.await {
                Ok(messages) => messages.iter().map(|message| message.id).min(),
                Err(error) => {
                    tracing::warn!(%error, "loading unread mentions or reactions failed");
                    None
                }
            };
            let Some(id) = oldest else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.jump_to(id, cx);
                // shortcut: read on the jump; tdesktop reads each message as it scrolls into view, which comes with read tracking.
                send(session.view_message(chat, id), "view message", cx);
            });
        })
        .detach();
    }
}
