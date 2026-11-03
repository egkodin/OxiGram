//! The folders column left of the chat list (`Window::FiltersMenu`): the
//! main menu button, then "All chats" and each folder with its icon, name
//! and unread badge.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Div, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Pixels, SharedString, Stateful, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder as _,
};
use i18n::key;
use model::chat::ChatList;
use model::chats::Chats;
use style::filter_icons::*;
use style::widgets::SideBarButton;
use style::window::{
    WINDOW_FILTERS_BUTTON, WINDOW_FILTERS_MAIN_MENU, WINDOW_FILTERS_MAIN_MENU_UNREAD,
    WINDOW_FILTERS_MAIN_MENU_UNREAD_MUTED, WINDOW_FILTERS_WIDTH, WINDOW_MIN_WIDTH,
};
use ui::{color, font, font_height, icon, icon_size};

use super::Dialogs;
use super::tabs::{DraggedTab, TabGhost, counter, tabs};
use crate::lang::tr;
use crate::px_of;
use crate::settings::{self, FoldersPlacement};

/// What the main menu button does.
pub(crate) type OnMainMenu = Rc<dyn Fn(&mut Window, &mut App)>;

/// Whether the folders show as the sidebar: there are folders, the user did
/// not put them on top, and the window has room
/// (`SessionController::enoughSpaceForFilters`).
pub(crate) fn is_sidebar_shown(chats: &Chats, window: &Window, cx: &App) -> bool {
    let room = f32::from(px_of(WINDOW_MIN_WIDTH) + px_of(WINDOW_FILTERS_WIDTH));
    !chats.folders().is_empty()
        && settings::folders_placement(cx) == FoldersPlacement::Sidebar
        && f32::from(window.viewport_size().width) >= room
}

/// A folder's icon and its active look, by TDLib's icon name
/// (`Ui::LookupFilterIcon`); unknown names get the custom folder icon.
fn folder_icons(name: &str) -> (style::Icon, style::Icon) {
    match name {
        "All" => (FOLDERS_ALL, FOLDERS_ALL_ACTIVE),
        "Unread" => (FOLDERS_UNREAD, FOLDERS_UNREAD_ACTIVE),
        "Unmuted" => (FOLDERS_UNMUTED, FOLDERS_UNMUTED_ACTIVE),
        "Bots" => (FOLDERS_BOTS, FOLDERS_BOTS_ACTIVE),
        "Channels" => (FOLDERS_CHANNELS, FOLDERS_CHANNELS_ACTIVE),
        "Groups" => (FOLDERS_GROUPS, FOLDERS_GROUPS_ACTIVE),
        "Private" => (FOLDERS_PRIVATE, FOLDERS_PRIVATE_ACTIVE),
        "Setup" => (FOLDERS_SETUP, FOLDERS_SETUP_ACTIVE),
        "Cat" => (FOLDERS_CAT, FOLDERS_CAT_ACTIVE),
        "Crown" => (FOLDERS_CROWN, FOLDERS_CROWN_ACTIVE),
        "Favorite" => (FOLDERS_FAVORITE, FOLDERS_FAVORITE_ACTIVE),
        "Flower" => (FOLDERS_FLOWER, FOLDERS_FLOWER_ACTIVE),
        "Game" => (FOLDERS_GAME, FOLDERS_GAME_ACTIVE),
        "Home" => (FOLDERS_HOME, FOLDERS_HOME_ACTIVE),
        "Love" => (FOLDERS_LOVE, FOLDERS_LOVE_ACTIVE),
        "Mask" => (FOLDERS_MASK, FOLDERS_MASK_ACTIVE),
        "Party" => (FOLDERS_PARTY, FOLDERS_PARTY_ACTIVE),
        "Sport" => (FOLDERS_SPORT, FOLDERS_SPORT_ACTIVE),
        "Study" => (FOLDERS_STUDY, FOLDERS_STUDY_ACTIVE),
        "Trade" => (FOLDERS_TRADE, FOLDERS_TRADE_ACTIVE),
        "Travel" => (FOLDERS_TRAVEL, FOLDERS_TRAVEL_ACTIVE),
        "Work" => (FOLDERS_WORK, FOLDERS_WORK_ACTIVE),
        "Airplane" => (FOLDERS_AIRPLANE, FOLDERS_AIRPLANE_ACTIVE),
        "Book" => (FOLDERS_BOOK, FOLDERS_BOOK_ACTIVE),
        "Light" => (FOLDERS_LIGHT, FOLDERS_LIGHT_ACTIVE),
        "Like" => (FOLDERS_LIKE, FOLDERS_LIKE_ACTIVE),
        "Money" => (FOLDERS_MONEY, FOLDERS_MONEY_ACTIVE),
        "Note" => (FOLDERS_NOTE, FOLDERS_NOTE_ACTIVE),
        "Palette" => (FOLDERS_PALETTE, FOLDERS_PALETTE_ACTIVE),
        _ => (FOLDERS_CUSTOM, FOLDERS_CUSTOM_ACTIVE),
    }
}

/// One sidebar button's content.
struct Button {
    icon: style::Icon,
    label: Option<SharedString>,
    badge: Option<(SharedString, bool)>,
    is_active: bool,
}

/// A `Ui::SideBarButton`: the icon centered on top, the name under it (at
/// most three lines) and the badge over the icon's top right.
fn side_bar_button(
    id: impl Into<gpui::ElementId>,
    st: &'static SideBarButton,
    button: Button,
    cx: &App,
) -> Stateful<Div> {
    let width = px_of(WINDOW_FILTERS_WIDTH);
    let icon_box = icon_size(button.icon);
    let centered = |position: style::Px, outer: Pixels, inner: Pixels| {
        if position.0 < 0 {
            (outer - inner) / 2.0
        } else {
            px_of(position)
        }
    };
    let icon_left = centered(st.icon_position.x, width, icon_box.width);
    // Only icon-only buttons center vertically; their height is the minimum.
    let icon_top = centered(st.icon_position.y, px_of(st.min_height), icon_box.height);
    let bg = if button.is_active {
        st.text_bg_active
    } else {
        st.text_bg
    };
    div()
        .id(id)
        .relative()
        .w(width)
        .min_h(px_of(st.min_height))
        .flex_none()
        .bg(color(cx, bg))
        .child(
            div()
                .absolute()
                .left(icon_left)
                .top(icon_top)
                .child(icon(button.icon)),
        )
        .when_some(button.label, |this, text| {
            this.child(label(st, text, button.is_active, cx))
        })
        .when_some(button.badge, |this, (text, is_muted)| {
            this.child(badge(st, text, is_muted, button.is_active, bg, cx))
        })
}

/// The name under the icon, at most three lines; longer names make the
/// button taller, as `resizeGetHeight` does.
fn label(st: &'static SideBarButton, text: SharedString, is_active: bool, cx: &App) -> Div {
    let text_font = st.style.font;
    let below_text = px_of(st.min_height) - px_of(st.text_top) - font_height(text_font);
    let fg = if is_active {
        st.text_fg_active
    } else {
        st.text_fg
    };
    div()
        .pt(px_of(st.text_top))
        .pb(below_text)
        .px(px_of(st.text_skip))
        .text_center()
        .line_clamp(3)
        .font(font(text_font))
        .text_size(px_of(text_font.size))
        .line_height(font_height(text_font))
        .text_color(color(cx, fg))
        .child(text)
}

/// The unread pill; its border in the button's color stands for the cut it
/// makes in the icon (`validateIconCache`).
fn badge(
    st: &'static SideBarButton,
    text: SharedString,
    is_muted: bool,
    is_active: bool,
    bg: style::ColorRef,
    cx: &App,
) -> impl IntoElement {
    let fill = match (is_active, is_muted) {
        (false, false) => st.badge_bg,
        (false, true) => st.badge_bg_muted,
        (true, false) => st.badge_bg_active,
        (true, true) => st.badge_bg_muted_active,
    };
    let stroke = px_of(st.badge_stroke);
    let height = px_of(st.badge_height);
    let badge_font = st.badge_style.font;
    div()
        .absolute()
        .left(px_of(WINDOW_FILTERS_WIDTH) / 2.0 + px_of(st.badge_position.x) - stroke)
        .top(px_of(st.badge_position.y) - stroke)
        .border(stroke)
        .border_color(color(cx, bg))
        .rounded(height / 2.0 + stroke)
        .child(
            div()
                .h(height)
                .min_w(height)
                .px(px_of(st.badge_skip))
                .flex()
                .items_center()
                .justify_center()
                .rounded(height / 2.0)
                .bg(color(cx, fill))
                .font(font(badge_font))
                .text_size(px_of(badge_font.size))
                .line_height(height)
                .text_color(color(cx, st.badge_fg))
                .child(text),
        )
}

impl Dialogs {
    /// The sidebar, when [`is_sidebar_shown`].
    // shortcut: no Edit button at the bottom until the folders settings page (M6); no folder links (`FiltersMenu::updateFavorite`).
    pub(crate) fn sidebar(
        &mut self,
        on_main_menu: OnMainMenu,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let st = &WINDOW_FILTERS_BUTTON;
        let menu = side_bar_button(
            "sidebar-main-menu",
            &WINDOW_FILTERS_MAIN_MENU,
            Button {
                icon: crate::accounts::others_unread_icon(
                    cx,
                    WINDOW_FILTERS_MAIN_MENU_UNREAD,
                    WINDOW_FILTERS_MAIN_MENU_UNREAD_MUTED,
                )
                .unwrap_or(WINDOW_FILTERS_MAIN_MENU.icon),
                label: None,
                badge: None,
                is_active: false,
            },
            cx,
        )
        .cursor_pointer()
        .on_click(move |_, window, cx| on_main_menu(window, cx));
        let buttons: Vec<AnyElement> = tabs(&self.store.read(cx).state().chats)
            .into_iter()
            .enumerate()
            .map(|(index, list)| self.sidebar_folder(index, list, cx))
            .collect();
        div()
            .w(px_of(WINDOW_FILTERS_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(color(cx, st.text_bg))
            .child(menu)
            .child(
                div()
                    .id("sidebar-folders")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(buttons),
            )
            .into_any_element()
    }

    fn sidebar_folder(&self, index: usize, list: ChatList, cx: &mut Context<Self>) -> AnyElement {
        let chats = &self.store.read(cx).state().chats;
        let folder = match list {
            ChatList::Folder(id) => chats.folders().iter().find(|folder| folder.id == id),
            ChatList::Main | ChatList::Archive => None,
        };
        let (label, icon_name) = match folder {
            Some(folder) => (
                SharedString::from(folder.name.text.clone()),
                folder.icon.as_str(),
            ),
            None => (tr(cx, key::lng_filters_all), "All"),
        };
        let is_active = self.list == list;
        let (normal, active) = folder_icons(icon_name);
        let button = Button {
            icon: if is_active { active } else { normal },
            label: Some(label.clone()),
            badge: counter(chats.unread(list)),
            is_active,
        };
        side_bar_button(
            ("sidebar-folder", index),
            &WINDOW_FILTERS_BUTTON,
            button,
            cx,
        )
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| this.open_list(list, cx)))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                this.show_folder_menu(list, event.position, window, cx);
            }),
        )
        .when(list != ChatList::Main, |this| {
            this.on_drag(DraggedTab { index, label }, |dragged, _, _, cx| {
                cx.new(|_| TabGhost(dragged.label.clone()))
            })
        })
        .on_drop(cx.listener(move |this, dragged: &DraggedTab, _, cx| {
            this.drop_tab(dragged.index, index, cx);
        }))
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_icon_names_get_the_custom_icon() {
        assert_eq!(folder_icons("Work"), (FOLDERS_WORK, FOLDERS_WORK_ACTIVE));
        assert_eq!(
            folder_icons("Rocket"),
            (FOLDERS_CUSTOM, FOLDERS_CUSTOM_ACTIVE)
        );
    }
}
