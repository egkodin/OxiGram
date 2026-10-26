//! The folder tabs above the list (`ChatsFiltersTabs` with
//! `chatsFiltersTabs`): "All chats" and each folder, with unread counters.

use std::rc::Rc;

use gpui::{
    AnyElement, Context, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Point,
    SharedString, Styled, Window, div, prelude::*,
};
use i18n::key;
use model::chat::ChatList;
use model::chats::{Chats, Unread};
use style::dialogs::CHATS_FILTERS_TABS;
use style::{ColorId, ColorRef};
use ui::{color, font, font_height};

use super::Dialogs;
use super::row::{Lit, unread_badge};
use crate::lang::tr;
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
fn tabs(chats: &Chats) -> Vec<ChatList> {
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
}

/// What a right click on a tab does, given the tab's index.
pub(super) type OnTabMenu =
    Rc<dyn Fn(&mut Dialogs, usize, Point<Pixels>, &mut Window, &mut Context<Dialogs>)>;

/// A row of tabs with the active one underlined (`Ui::SettingsSlider` with
/// `dialogsSearchTabs`); `on_click` gets the clicked tab's index.
pub(super) fn tab_strip(
    id: &'static str,
    tabs: Vec<Tab>,
    on_menu: Option<OnTabMenu>,
    cx: &mut Context<Dialogs>,
    on_click: impl Fn(&mut Dialogs, usize, &mut Context<Dialogs>) + Clone + 'static,
) -> AnyElement {
    let st = &CHATS_FILTERS_TABS;
    let label_font = st.label_style.font;
    let half_skip = px_of(st.strict_skip) / 2.0;
    let bar_overhang = px_of(st.bar_stroke) / 2.0;
    let sections: Vec<AnyElement> =
        tabs.into_iter()
            .enumerate()
            .map(|(index, tab)| {
                let fg = if tab.is_active {
                    st.label_fg_active
                } else {
                    st.label_fg
                };
                let on_click = on_click.clone();
                let on_menu = on_menu.clone();
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
                            .children(tab.counter.map(|(text, is_muted)| {
                                unread_badge(text, is_muted, Lit::Normal, cx)
                            })),
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
                    .on_click(cx.listener(move |this, _, _, cx| on_click(this, index, cx)))
                    .when_some(on_menu, |tab, on_menu| {
                        tab.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                on_menu(this, index, event.position, window, cx);
                            }),
                        )
                    })
                    .into_any_element()
            })
            .collect();
    div()
        .id(id)
        .flex()
        .flex_none()
        .px(px_of(st.padding))
        .overflow_x_scroll()
        .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
        .children(sections)
        .into_any_element()
}

impl Dialogs {
    /// The folder tabs, shown while there are folders and the list is not
    /// the archive, a search or the search suggestions.
    pub(super) fn tabs_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chats = &self.store.read(cx).state().chats;
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
            })
            .collect();
        let menu_lists = lists.clone();
        let on_menu: OnTabMenu = Rc::new(move |this, index, position, window, cx| {
            if let Some(list) = menu_lists.get(index) {
                this.show_folder_menu(*list, position, window, cx);
            }
        });
        Some(tab_strip(
            "folder-tab",
            shown,
            Some(on_menu),
            cx,
            move |this, index, cx| {
                if let Some(list) = lists.get(index) {
                    this.open_list(*list, cx);
                }
            },
        ))
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
