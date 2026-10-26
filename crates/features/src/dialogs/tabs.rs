//! The folder tabs above the list (`ChatsFiltersTabs` with
//! `chatsFiltersTabs`): "All chats" and each folder, with unread counters.

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
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

impl Dialogs {
    /// The tab strip, shown while there are folders and the list is not
    /// the archive or a search.
    pub(super) fn tabs_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chats = &self.store.read(cx).state().chats;
        if chats.folders().is_empty() || self.list == ChatList::Archive || self.search.is_some() {
            return None;
        }
        let st = &CHATS_FILTERS_TABS;
        let label_font = st.label_style.font;
        let half_skip = px_of(st.strict_skip) / 2.0;
        let bar_overhang = px_of(st.bar_stroke) / 2.0;
        let sections: Vec<AnyElement> = tabs(chats)
            .into_iter()
            .enumerate()
            .map(|(index, list)| {
                let is_active = self.list == list;
                let name: SharedString = match list {
                    ChatList::Folder(id) => chats
                        .folders()
                        .iter()
                        .find(|folder| folder.id == id)
                        .map(|folder| folder.name.text.clone().into())
                        .unwrap_or_default(),
                    ChatList::Main | ChatList::Archive => tr(cx, key::lng_filters_all_short),
                };
                let fg = if is_active {
                    st.label_fg_active
                } else {
                    st.label_fg
                };
                div()
                    .id(("folder-tab", index))
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
                            .child(name)
                            .children(counter(chats.unread(list)).map(|(text, is_muted)| {
                                unread_badge(text, is_muted, Lit::Normal, cx)
                            })),
                    )
                    .when(is_active, |tab| {
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
                    .on_click(cx.listener(move |this, _, _, cx| this.open_list(list, cx)))
                    .into_any_element()
            })
            .collect();
        Some(
            div()
                .id("folder-tabs")
                .flex()
                .flex_none()
                .px(px_of(st.padding))
                .overflow_x_scroll()
                .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
                .children(sections)
                .into_any_element(),
        )
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
