//! The parts of tdesktop's `PeerListBox` that boxes listing people or
//! chats share: the search row, the list's height, rows with a userpic, a
//! name and a status line, and the empty list's text.

use gpui::{
    AnyElement, App, Div, ElementId, Entity, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Pixels, SharedString, Stateful, Styled as _, Window, div, px,
};
use style::basic::{NO_CONTACTS_COLOR, NO_CONTACTS_FONT, NO_CONTACTS_HEIGHT};
use style::boxes::{CONTACTS_STATUS_FONT, CONTACTS_WITH_STORIES};
use style::layers::BOX_MAX_LIST_HEIGHT;
use style::widgets::DEFAULT_MULTI_SELECT;
use ui::{InputField, Userpic, UserpicSource, color, font, font_height};

use crate::px_of;

/// Room a box leaves around itself in a short window.
const WINDOW_MARGIN: f32 = 160.0;

/// What a row shows.
pub(crate) struct Look {
    pub name: SharedString,
    pub userpic: UserpicSource,
    pub status: SharedString,
    /// The status in the active color, as "online" is.
    pub is_active: bool,
}

/// The search field with its icon over the list.
pub(crate) fn search_row(search: Entity<InputField>, cx: &App) -> Div {
    let st = &DEFAULT_MULTI_SELECT;
    div()
        .bg(color(cx, st.bg))
        .pl(px_of(st.padding.left))
        .pr(px_of(st.padding.right))
        .pt(px_of(st.padding.top))
        .pb(px_of(st.padding.bottom))
        .flex()
        .items_center()
        .gap(px_of(st.field_icon_skip))
        .child(ui::icon(st.field_icon))
        .child(div().flex_1().child(search))
}

/// The list's height: the box's maximum under the search row, less in a
/// short window.
pub(crate) fn list_height(window: &Window) -> Pixels {
    let st = &DEFAULT_MULTI_SELECT;
    let search_height =
        px_of(st.field.height_min) + px_of(st.padding.top) + px_of(st.padding.bottom);
    let window_room = window.viewport_size().height - px(WINDOW_MARGIN);
    (px_of(BOX_MAX_LIST_HEIGHT) - search_height)
        .min(window_room)
        .max(px(100.0))
}

/// The text in place of an empty list.
pub(crate) fn empty(text: SharedString, cx: &App) -> AnyElement {
    div()
        .h(px_of(NO_CONTACTS_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .font(font(NO_CONTACTS_FONT))
        .text_size(px_of(NO_CONTACTS_FONT.size))
        .text_color(color(cx, NO_CONTACTS_COLOR))
        .child(text)
        .into_any_element()
}

/// A row; the caller adds what hovering and clicking do.
pub(crate) fn row(
    id: impl Into<ElementId>,
    look: Look,
    is_hovered: bool,
    cx: &App,
) -> Stateful<Div> {
    let st = &CONTACTS_WITH_STORIES.item;
    let status_fg = if look.is_active {
        st.status_fg_active
    } else if is_hovered {
        st.status_fg_over
    } else {
        st.status_fg
    };
    let name_font = st.name_style.font;
    let status_font = CONTACTS_STATUS_FONT;
    div()
        .id(id)
        .relative()
        .h(px_of(st.height))
        .w_full()
        .bg(color(
            cx,
            if is_hovered {
                st.button.text_bg_over
            } else {
                CONTACTS_WITH_STORIES.bg
            },
        ))
        .child(
            div()
                .absolute()
                .left(px_of(st.photo_position.x))
                .top(px_of(st.photo_position.y))
                .child(Userpic::new(px_of(st.photo_size), look.userpic)),
        )
        .child(
            div()
                .absolute()
                .left(px_of(st.name_position.x))
                .top(px_of(st.name_position.y))
                .right(px(0.0))
                .pr(px_of(st.photo_position.x))
                .font(font(name_font))
                .text_size(px_of(name_font.size))
                .line_height(font_height(name_font))
                .text_color(color(cx, st.name_fg))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(look.name),
        )
        .child(
            div()
                .absolute()
                .left(px_of(st.status_position.x))
                .top(px_of(st.status_position.y))
                .font(font(status_font))
                .text_size(px_of(status_font.size))
                .line_height(font_height(status_font))
                .text_color(color(cx, status_fg))
                .whitespace_nowrap()
                .child(look.status),
        )
}

/// The hovered row after the pointer entered or left row `index`.
pub(crate) fn hovered_after(
    hovered: Option<usize>,
    index: usize,
    is_entered: bool,
) -> Option<usize> {
    if is_entered {
        Some(index)
    } else {
        hovered.filter(|was| *was != index)
    }
}
