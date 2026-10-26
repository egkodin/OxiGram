//! The "Unread messages" bar (`UnreadBar`) and the button that scrolls
//! down to the newest message with the unread count (`HistoryDownButton`).

use gpui::{
    AnyElement, App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use i18n::key;
use style::chat::{HISTORY_UNREAD_BAR_FONT, HISTORY_UNREAD_BAR_HEIGHT, HISTORY_UNREAD_BAR_MARGIN};
use style::chat_helpers::{HISTORY_TO_DOWN, HISTORY_TO_DOWN_POSITION};
use style::{ColorId, ColorRef};
use ui::{color, font, font_height, icon};

use crate::dialogs::{Lit, unread_badge};
use crate::lang::tr;
use crate::px_of;

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

/// The round arrow at the bottom right; the badge counts unread messages.
// shortcut: the badge uses the chat list's 20 px pill; tdesktop draws it 22 px high (`historyToDownBadgeSize`).
pub(super) fn down_button(
    unread: Option<(SharedString, bool)>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let st = &HISTORY_TO_DOWN;
    let layer = |normal, over| {
        div()
            .absolute()
            .top(px_of(st.icon_position.y))
            .left(px_of(st.icon_position.x))
            .child(
                div()
                    .group_hover("history-down", |s| s.opacity(0.0))
                    .child(icon(normal)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .opacity(0.0)
                    .group_hover("history-down", |s| s.opacity(1.0))
                    .child(icon(over)),
            )
    };
    div()
        .id("history-down")
        .group("history-down")
        .absolute()
        .right(px_of(HISTORY_TO_DOWN_POSITION.x))
        .bottom(px_of(HISTORY_TO_DOWN_POSITION.y))
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
