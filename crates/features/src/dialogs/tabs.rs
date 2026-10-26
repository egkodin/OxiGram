//! The folder tabs above the list (`ChatsFiltersTabs` with
//! `chatsFiltersTabs`): "All chats" and each folder, with unread counters.

use std::rc::Rc;

use gpui::{
    AnyElement, AppContext as _, Context, Div, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, Render, SharedString, Stateful, Styled, Window, div, prelude::*,
};
use i18n::key;
use model::chat::ChatList;
use model::chats::{Chats, Unread};
use style::dialogs::CHATS_FILTERS_TABS;
use style::{ColorId, ColorRef};
use ui::{color, font, font_height};

use super::Dialogs;
use super::pinned::reordered;
use super::row::{Lit, unread_badge};
use crate::lang::tr;
use crate::peer_menu::send;
use crate::px_of;

/// Between a tab's name and its counter (`st::lineWidth * 5`).
const UNREAD_SKIP: f32 = 5.0;

/// The counter a tab shows, if any, and whether it is grey: all of its
/// unread chats are muted. Muted chats count, as `includeMutedCounterFolders`
/// does by default.
pub(super) fn counter(unread: Unread) -> Option<(SharedString, bool)> {
    if unread.chats <= 0 {
        return None;
    }
    let text = if unread.chats > 999 {
        "99+".to_owned()
    } else {
        unread.chats.to_string()
    };
    Some((text.into(), unread.unmuted_chats == 0))
}

/// The tabs in order: the main list first, then the folders.
// shortcut: "All chats" always leads; TDLib's `main_chat_list_position` (Premium) is ignored until M13.
pub(super) fn tabs(chats: &Chats) -> Vec<ChatList> {
    std::iter::once(ChatList::Main)
        .chain(
            chats
                .folders()
                .iter()
                .map(|folder| ChatList::Folder(folder.id)),
        )
        .collect()
}

/// One tab of a [`tab_strip`].
pub(super) struct Tab {
    pub label: SharedString,
    pub counter: Option<(SharedString, bool)>,
    pub is_active: bool,
    /// Whether it can be dragged and dropped onto (`addPinnedInterval`).
    pub is_movable: bool,
}

/// What a right click on a tab does, given the tab's index.
pub(super) type OnTabMenu =
    Rc<dyn Fn(&mut Dialogs, usize, Point<Pixels>, &mut Window, &mut Context<Dialogs>)>;

/// What dropping the tab at the first index onto the second does.
pub(super) type OnTabDrop = Rc<dyn Fn(&mut Dialogs, usize, usize, &mut Context<Dialogs>)>;

/// The menu and drag handlers of a [`tab_strip`] that has them.
#[derive(Clone)]
pub(super) struct TabEvents {
    pub menu: OnTabMenu,
    pub drop: OnTabDrop,
}

/// The tab being dragged.
#[derive(Debug, Clone)]
pub(super) struct DraggedTab {
    pub index: usize,
    pub label: SharedString,
}

/// The dragged tab's name following the pointer.
// shortcut: the other tabs do not slide aside as in `ChatsFiltersTabsReorder`; upgrade with the list animations in M8.
pub(super) struct TabGhost(pub SharedString);

impl Render for TabGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &CHATS_FILTERS_TABS;
        let label_font = st.label_style.font;
        div()
            .px(px_of(st.strict_skip) / 2.0)
            .pt(px_of(st.label_top))
            .h(px_of(st.height))
            .whitespace_nowrap()
            .font(font(label_font))
            .text_size(px_of(label_font.size))
            .line_height(font_height(label_font))
            .text_color(color(cx, st.label_fg_active))
            .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
            .child(self.0.clone())
    }
}

/// A row of tabs with the active one underlined (`Ui::SettingsSlider` with
/// `dialogsSearchTabs`); `on_click` gets the clicked tab's index.
pub(super) fn tab_strip(
    id: &'static str,
    tabs: Vec<Tab>,
    events: Option<TabEvents>,
    cx: &mut Context<Dialogs>,
    on_click: impl Fn(&mut Dialogs, usize, &mut Context<Dialogs>) + Clone + 'static,
) -> AnyElement {
    let sections: Vec<AnyElement> = tabs
        .into_iter()
        .enumerate()
        .map(|(index, tab)| {
            let on_click = on_click.clone();
            section(id, index, tab, cx)
                .on_click(cx.listener(move |this, _, _, cx| on_click(this, index, cx)))
                .when_some(events.clone(), |section, events| {
                    with_events(section, index, events, cx)
                })
                .into_any_element()
        })
        .collect();
    div()
        .id(id)
        .flex()
        .flex_none()
        .px(px_of(CHATS_FILTERS_TABS.padding))
        .overflow_x_scroll()
        .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
        .children(sections)
        .into_any_element()
}

fn section(id: &'static str, index: usize, tab: Tab, cx: &mut Context<Dialogs>) -> Stateful<Div> {
    let st = &CHATS_FILTERS_TABS;
    let label_font = st.label_style.font;
    let half_skip = px_of(st.strict_skip) / 2.0;
    let bar_overhang = px_of(st.bar_stroke) / 2.0;
    let fg = if tab.is_active {
        st.label_fg_active
    } else {
        st.label_fg
    };
    let dragged = tab.is_movable.then(|| DraggedTab {
        index,
        label: tab.label.clone(),
    });
    div()
        .id((id, index))
        .relative()
        .flex_none()
        .h(px_of(st.height))
        .px(half_skip)
        .pt(px_of(st.label_top))
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_start()
                .gap(gpui::px(UNREAD_SKIP))
                .whitespace_nowrap()
                .font(font(label_font))
                .text_size(px_of(label_font.size))
                .line_height(font_height(label_font))
                .text_color(color(cx, fg))
                .child(tab.label)
                .children(
                    tab.counter
                        .map(|(text, is_muted)| unread_badge(text, is_muted, Lit::Normal, cx)),
                ),
        )
        .when(tab.is_active, |tab| {
            tab.child(
                div()
                    .absolute()
                    .left(half_skip - bar_overhang)
                    .right(half_skip - bar_overhang)
                    .top(px_of(st.bar_top))
                    .h(px_of(st.bar_stroke))
                    .rounded(px_of(st.bar_radius))
                    .bg(color(cx, st.bar_fg_active)),
            )
        })
        .when_some(dragged, |tab, dragged| {
            tab.on_drag(dragged, |dragged, _, _, cx| {
                cx.new(|_| TabGhost(dragged.label.clone()))
            })
        })
}

/// Adds the context menu and, for a movable tab, dropping another onto it.
fn with_events(
    section: Stateful<Div>,
    index: usize,
    events: TabEvents,
    cx: &mut Context<Dialogs>,
) -> Stateful<Div> {
    let TabEvents { menu, drop } = events;
    section
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                menu(this, index, event.position, window, cx);
            }),
        )
        .on_drop(cx.listener(move |this, dragged: &DraggedTab, _, cx| {
            drop(this, dragged.index, index, cx);
        }))
}

impl Dialogs {
    /// The folder tabs, shown while there are folders, the sidebar is not
    /// shown and the list is not the archive, a search or the search
    /// suggestions.
    pub(super) fn tabs_strip(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chats = &self.store.read(cx).state().chats;
        if super::is_sidebar_shown(chats, window, cx) {
            return None;
        }
        // shortcut: the narrow list drops the tabs; tdesktop's folder icons sidebar is a later M2 step.
        if chats.folders().is_empty()
            || self.list == ChatList::Archive
            || self.search.is_some()
            || self.suggestions.is_some()
            || self.narrow
        {
            return None;
        }
        let lists = tabs(chats);
        let shown: Vec<Tab> = lists
            .iter()
            .map(|&list| Tab {
                label: match list {
                    ChatList::Folder(id) => chats
                        .folders()
                        .iter()
                        .find(|folder| folder.id == id)
                        .map(|folder| folder.name.text.clone().into())
                        .unwrap_or_default(),
                    ChatList::Main | ChatList::Archive => tr(cx, key::lng_filters_all_short),
                },
                counter: counter(chats.unread(list)),
                is_active: self.list == list,
                is_movable: list != ChatList::Main,
            })
            .collect();
        let menu_lists = lists.clone();
        let events = TabEvents {
            menu: Rc::new(move |this, index, position, window, cx| {
                if let Some(list) = menu_lists.get(index) {
                    this.show_folder_menu(*list, position, window, cx);
                }
            }),
            drop: Rc::new(|this, from, to, cx| this.drop_tab(from, to, cx)),
        };
        Some(tab_strip(
            "folder-tab",
            shown,
            Some(events),
            cx,
            move |this, index, cx| {
                if let Some(list) = lists.get(index) {
                    this.open_list(*list, cx);
                }
            },
        ))
    }
}

impl Dialogs {
    /// Moves the folder of tab `from` to where tab `to` is
    /// (`ChatsFiltersTabs` `applyReorder`).
    pub(super) fn drop_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let chats = &self.store.read(cx).state().chats;
        let lists = tabs(chats);
        let (Some(ChatList::Folder(moved)), Some(ChatList::Folder(target))) =
            (lists.get(from), lists.get(to))
        else {
            return;
        };
        let folders: Vec<_> = chats.folders().iter().map(|folder| folder.id).collect();
        let Some(order) = reordered(&folders, *moved, *target) else {
            return;
        };
        let task = self.store.read(cx).session().reorder_folders(&order);
        send(task, "reorder folders", cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_hides_zero_caps_and_greys_all_muted() {
        let unread = |chats, unmuted_chats| Unread {
            chats,
            unmuted_chats,
        };
        assert_eq!(counter(unread(0, 0)), None);
        assert_eq!(counter(unread(3, 1)), Some(("3".into(), false)));
        assert_eq!(counter(unread(2, 0)), Some(("2".into(), true)));
        assert_eq!(counter(unread(1000, 5)), Some(("99+".into(), false)));
    }
}
