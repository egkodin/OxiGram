//! The bar over an open chat (`HistoryView::TopBarWidget`): its name and
//! status, with a back arrow and the userpic when the chat fills the
//! window.

use chrono::Local;
use gpui::{
    AnyElement, App, ClickEvent, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::FluentBuilder as _,
};
use i18n::key;
use model::chat::ChatKind;
use model::{ChatId, State};
use style::basic::SEMIBOLD_FONT;
use style::chat::{
    HISTORY_SAVED_FONT, HISTORY_STATUS_FG, HISTORY_STATUS_FG_ACTIVE, HISTORY_STATUS_FG_TYPING,
};
use style::dialogs::DIALOGS_TEXT_FONT;
use style::info::{
    HISTORY_TOP_BAR_BACK, TOP_BAR_ACTION_SKIP, TOP_BAR_ARROW_PADDING, TOP_BAR_CLEAR_BUTTON,
    TOP_BAR_HEIGHT, TOP_BAR_INFO_BUTTON, TOP_BAR_MENU_TOGGLE, TOP_BAR_NAME_RIGHT_PADDING,
    TOP_BAR_SEARCH,
};
use style::{ColorId, ColorRef, Font};
use ui::{IconButton, RoundButton, Userpic, UserpicSource, color, font, font_height};

use crate::dialogs::{action_text, photo_userpic};
use crate::lang::strings;
use crate::px_of;
use crate::status::{members_status, status};

/// What the back arrow or the menu toggle does.
pub(super) type OnBack = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// The status line and its color.
fn status_line(state: &State, chat: ChatId, cx: &App) -> Option<(String, ColorRef)> {
    let chats = &state.chats;
    let chat = chats.chat(chat)?;
    if let Some(action) = action_text(strings(cx), chats, chat) {
        return Some((action, HISTORY_STATUS_FG_TYPING));
    }
    match chat.kind {
        ChatKind::Private { user } | ChatKind::Secret { user } => {
            let line = status(strings(cx), chats.user(user)?, &Local::now());
            let fg = if line.is_online {
                HISTORY_STATUS_FG_ACTIVE
            } else {
                HISTORY_STATUS_FG
            };
            Some((line.text, fg))
        }
        ChatKind::Group { .. } | ChatKind::Supergroup { .. } => {
            members_status(chat, chats.member_count(chat), cx).map(|text| (text, HISTORY_STATUS_FG))
        }
    }
}

fn text(
    value: impl Into<SharedString>,
    text_font: Font,
    fg: ColorRef,
    cx: &App,
) -> impl IntoElement {
    div()
        .truncate()
        .font(font(text_font))
        .text_size(px_of(text_font.size))
        .line_height(font_height(text_font))
        .text_color(color(cx, fg))
        .child(value.into())
}

/// What the bar's buttons do.
pub(super) struct Actions {
    /// Shows the arrow and the userpic, as one column does.
    pub on_back: Option<OnBack>,
    pub on_search: OnBack,
    pub on_menu: OnBack,
}

/// The bar of a chat.
// shortcut: no call and info buttons beside search and the menu toggle; they come with calls (M12) and the info panel (M6).
pub(super) fn top_bar(state: &State, chat: ChatId, actions: Actions, cx: &App) -> AnyElement {
    let Actions {
        on_back,
        on_search,
        on_menu,
    } = actions;
    let is_self = state
        .chats
        .chat(chat)
        .is_some_and(|chat| chat.user().is_some() && chat.user() == state.me);
    let name: SharedString = match state.chats.chat(chat) {
        _ if is_self => strings(cx).text(key::lng_saved_messages).to_owned().into(),
        Some(chat) => chat.title.clone().into(),
        None => SharedString::default(),
    };
    let name_fg = ColorRef::Palette(ColorId::DialogsNameFg);
    let status = (!is_self).then(|| status_line(state, chat, cx)).flatten();
    let has_back = on_back.is_some();
    let userpic = has_back.then(|| {
        let source = match state.chats.chat(chat) {
            _ if is_self => UserpicSource::SavedMessages,
            Some(chat) => {
                photo_userpic(
                    &state.chats,
                    chat.photo.as_ref(),
                    chat.accent_color,
                    &chat.title,
                )
                .0
            }
            None => UserpicSource::SavedMessages,
        };
        let st = &TOP_BAR_INFO_BUTTON;
        div()
            .flex_none()
            .w(px_of(st.size.width))
            .h(px_of(TOP_BAR_HEIGHT))
            .pl(px_of(st.photo_position.x))
            .pt((px_of(TOP_BAR_HEIGHT) - px_of(st.photo_size)) / 2.0 + px_of(st.photo_position.y))
            .child(Userpic::new(px_of(st.photo_size), source))
    });
    let lines = div()
        .flex_1()
        .min_w_0()
        .h(px_of(TOP_BAR_HEIGHT))
        .pr(px_of(TOP_BAR_NAME_RIGHT_PADDING))
        .flex()
        .flex_col()
        .map(|this| match status {
            // A name alone sits in the middle, larger (`historySavedFont`).
            None => this
                .justify_center()
                .child(text(name, HISTORY_SAVED_FONT, name_fg, cx)),
            Some((line, fg)) => this
                .justify_between()
                .pt(px_of(TOP_BAR_ARROW_PADDING.top))
                .pb(px_of(TOP_BAR_ARROW_PADDING.bottom))
                .child(text(name, SEMIBOLD_FONT, name_fg, cx))
                .child(text(line, DIALOGS_TEXT_FONT, fg, cx)),
        });
    div()
        .flex_none()
        .flex()
        .h(px_of(TOP_BAR_HEIGHT))
        .bg(color(cx, ColorRef::Palette(ColorId::TopBarBg)))
        .when(!has_back, |this| {
            this.pl(px_of(TOP_BAR_ARROW_PADDING.right))
        })
        .children(on_back.map(|on_back| {
            IconButton::new("history-back", &HISTORY_TOP_BAR_BACK).on_click(on_back)
        }))
        .children(userpic)
        .child(lines)
        .child(
            div()
                .flex_none()
                .flex()
                .child(IconButton::new("history-search", &TOP_BAR_SEARCH).on_click(on_search))
                .child(IconButton::new("history-menu", &TOP_BAR_MENU_TOGGLE).on_click(on_menu)),
        )
        .into_any_element()
}

/// A section's bar: the back arrow and its title, such as "3 pinned
/// messages" (`TopBarWidget` with a custom title).
pub(super) fn section_bar(title: SharedString, on_back: OnBack, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px_of(TOP_BAR_HEIGHT))
        .bg(color(cx, ColorRef::Palette(ColorId::TopBarBg)))
        .child(IconButton::new("history-back", &HISTORY_TOP_BAR_BACK).on_click(on_back))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pr(px_of(TOP_BAR_NAME_RIGHT_PADDING))
                .child(text(
                    title,
                    HISTORY_SAVED_FONT,
                    ColorRef::Palette(ColorId::DialogsNameFg),
                    cx,
                )),
        )
        .into_any_element()
}

/// The bar while messages are selected (`TopBarWidget::showSelected`).
// shortcut: no Forward and Delete buttons, which also show the count; they come with forwarding and deleting in M4.
pub(super) fn selection_bar(
    on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_end()
        .h(px_of(TOP_BAR_HEIGHT))
        .pr(px_of(TOP_BAR_ACTION_SKIP))
        .bg(color(cx, ColorRef::Palette(ColorId::TopBarBg)))
        .child(
            RoundButton::new(
                "history-selection-clear",
                &TOP_BAR_CLEAR_BUTTON,
                strings(cx).text(key::lng_selected_clear).to_owned(),
            )
            .uppercase()
            .on_click(on_cancel),
        )
        .into_any_element()
}
