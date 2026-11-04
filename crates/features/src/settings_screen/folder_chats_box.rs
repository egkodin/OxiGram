//! "Included Chats" / "Excluded Chats" (`EditFilterChatsListController`):
//! chat types and chats to tick; Save hands them back to the folder box.
// shortcut: no search field, no chat count limit box, no "in folders" status line and no ring animation around a ticked userpic; the chats are the main list's loaded ones. With row "Create and edit folders".

use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, Render, StatefulInteractiveElement as _, Styled, Window, div,
};
use i18n::key;
use model::ChatId;
use model::chat::ChatList;
use model::chat_folders::{ChatType, FolderRules};
use style::boxes::CONTACTS_WITH_STORIES;
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::widgets::DEFAULT_PEER_LIST_CHECK;
use ui::{BoxFrame, RoundButton, Userpic, color};

use super::folder_box::{chat_look, type_name, type_userpic};
use super::{Env, rows};
use crate::lang::tr;
use crate::{peer_list, px_of};

/// Which list of the folder the box edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Include,
    Exclude,
}

impl Side {
    pub(super) fn types(self) -> &'static [ChatType] {
        match self {
            Self::Include => &ChatType::INCLUDED,
            Self::Exclude => &ChatType::EXCLUDED,
        }
    }

    pub(super) fn chats(self, rules: &FolderRules) -> Vec<ChatId> {
        match self {
            Self::Include => rules.included(),
            Self::Exclude => rules.excluded(),
        }
    }

    pub(super) fn id(self, what: &'static str) -> &'static str {
        match (self, what) {
            (Self::Include, "type") => "folder-include-type",
            (Self::Include, "chat") => "folder-include-chat",
            (Self::Include, _) => "folder-include-add",
            (Self::Exclude, "type") => "folder-exclude-type",
            (Self::Exclude, "chat") => "folder-exclude-chat",
            (Self::Exclude, _) => "folder-exclude-add",
        }
    }
}

pub(super) type OnSaved = Rc<dyn Fn(FolderRules, &mut App)>;

pub(super) struct ChatsBox {
    env: Env,
    side: Side,
    rules: FolderRules,
    chats: Vec<ChatId>,
    chosen: BTreeSet<ChatId>,
    on_saved: OnSaved,
    focus: FocusHandle,
}

impl Focusable for ChatsBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(
    env: &Env,
    side: Side,
    rules: FolderRules,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let chosen: Vec<ChatId> = side.chats(&rules);
    let listed = env
        .targets
        .store
        .read(cx)
        .state()
        .chats
        .list(ChatList::Main);
    let mut chats = chosen.clone();
    chats.extend(listed.filter(|chat| !chosen.contains(chat)));
    let view = cx.new(|cx| ChatsBox {
        env: env.clone(),
        side,
        rules,
        chats,
        chosen: chosen.into_iter().collect(),
        on_saved,
        focus: cx.focus_handle(),
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

/// The round check over a ticked userpic's corner (`RoundImageCheckbox`).
fn ticked(userpic: AnyElement, is_ticked: bool, cx: &App) -> AnyElement {
    let size = px_of(CONTACTS_WITH_STORIES.item.photo_size);
    let check = &DEFAULT_PEER_LIST_CHECK;
    let side = px_of(check.size);
    let mark = is_ticked.then(|| {
        div()
            .absolute()
            .right(-px_of(check.width))
            .bottom(-px_of(check.width))
            .size(side)
            .rounded_full()
            .border(px_of(check.width))
            .border_color(color(cx, check.border))
            .bg(color(cx, check.bg_active))
            .child(ui::icon(check.check))
    });
    div()
        .relative()
        .size(size)
        .child(userpic)
        .children(mark)
        .into_any_element()
}

impl ChatsBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn save(&self, window: &mut Window, cx: &mut App) {
        let mut rules = self.rules.clone();
        let chosen: Vec<ChatId> = self
            .chats
            .iter()
            .copied()
            .filter(|chat| self.chosen.contains(chat))
            .collect();
        match self.side {
            Side::Include => rules.set_included(&chosen),
            Side::Exclude => rules.set_excluded(&chosen),
        }
        (self.on_saved)(rules, cx);
        self.close(window, cx);
    }

    fn type_row(&self, index: usize, kind: ChatType, cx: &mut Context<Self>) -> AnyElement {
        let is_on = self.rules.has(kind);
        let size = px_of(CONTACTS_WITH_STORIES.item.photo_size);
        let userpic = ticked(type_userpic(kind, size, cx).into_any_element(), is_on, cx);
        let weak = cx.weak_entity();
        peer_list::row_with(
            ("folder-chats-type", index),
            userpic,
            tr(cx, type_name(kind)),
            Default::default(),
            false,
            false,
            cx,
        )
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                this.rules.set(kind, !is_on);
                cx.notify();
            });
        })
        .into_any_element()
    }

    fn chat_row(&self, index: usize, chat: ChatId, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (name, userpic) = chat_look(&self.env, chat, cx)?;
        let size = px_of(CONTACTS_WITH_STORIES.item.photo_size);
        let is_on = self.chosen.contains(&chat);
        let userpic = ticked(Userpic::new(size, userpic).into_any_element(), is_on, cx);
        let weak = cx.weak_entity();
        let row = peer_list::row_with(
            ("folder-chats-chat", index),
            userpic,
            name.into(),
            Default::default(),
            false,
            false,
            cx,
        )
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                if !this.chosen.remove(&chat) {
                    this.chosen.insert(chat);
                }
                cx.notify();
            });
        });
        Some(row.into_any_element())
    }
}

impl Render for ChatsBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.side {
            Side::Include => key::lng_filters_include_title,
            Side::Exclude => key::lng_filters_exclude_title,
        };
        let types: Vec<_> = self
            .side
            .types()
            .iter()
            .enumerate()
            .map(|(index, kind)| self.type_row(index, *kind, cx))
            .collect();
        let chats: Vec<_> = self
            .chats
            .clone()
            .into_iter()
            .enumerate()
            .filter_map(|(index, chat)| self.chat_row(index, chat, cx))
            .collect();
        let list = div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_filters_edit_types), cx))
            .children(types)
            .child(rows::skip())
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_filters_edit_chats), cx))
            .children(chats);
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        let frame = BoxFrame::new("folder-chats-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(tr(cx, title))
            .row(list)
            .button(
                RoundButton::new(
                    "folder-chats-save",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_settings_save),
                )
                .on_click(move |_, window, cx| {
                    let _ = save.update(cx, |this, cx| this.save(window, cx));
                }),
            )
            .button(
                RoundButton::new(
                    "folder-chats-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                }),
            );
        div().track_focus(&self.focus).child(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_side_offers_its_own_types() {
        assert_eq!(Side::Include.types().len(), 5);
        assert_eq!(
            Side::Exclude.types(),
            [ChatType::Muted, ChatType::Read, ChatType::Archived]
        );
        assert_ne!(Side::Include.id("chat"), Side::Exclude.id("chat"));
    }
}
