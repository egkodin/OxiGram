//! The pinned messages section (`PinnedWidget`): a chat's pins in a list
//! of their own, under a bar with their count, over a button that unpins
//! them all or hides the pinned bar.

use std::rc::Rc;

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};
use i18n::key;
use style::chat_helpers::{HISTORY_COMPOSE_BUTTON, HISTORY_PINNED_SHOW_ALL};
use style::{ColorId, ColorRef};
use ui::{IconButton, color, font, font_height};

use super::HistoryView;
use super::service::plural;
use super::top_bar;
use crate::boxes::{self, ConfirmArgs};
use crate::lang::{strings, tr};
use crate::peer_menu::send;
use crate::px_of;
use crate::store::spawn;

/// What the history shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Section {
    #[default]
    Chat,
    /// The pins; whether the user may unpin them, once TDLib told.
    Pinned { can_unpin: Option<bool> },
}

impl HistoryView {
    /// The pinned bar's button that lists all pins (`historyPinnedShowAll`).
    pub(super) fn show_all_pins_button(&self, cx: &Context<Self>) -> IconButton {
        let weak = cx.weak_entity();
        IconButton::new("history-pinned-show-all", &HISTORY_PINNED_SHOW_ALL).on_click(
            move |_, _, cx| {
                cx.stop_propagation();
                let _ = weak.update(cx, |this, cx| this.open_pinned_section(cx));
            },
        )
    }

    fn open_pinned_section(&mut self, cx: &mut Context<Self>) {
        let Some(newest) = self
            .store
            .read(cx)
            .state()
            .history(self.chat)
            .and_then(|history| history.pinned().last().copied())
        else {
            return;
        };
        self.set_section(Section::Pinned { can_unpin: None }, cx);
        // Pinning rights are the same for every message of the chat.
        let task = spawn(
            self.store
                .read(cx)
                .session()
                .message_abilities(self.chat, newest),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let can_unpin = task
                .await
                .inspect_err(|error| tracing::warn!(%error, "message abilities unknown"))
                .is_ok_and(|abilities| abilities.can_be_pinned);
            let _ = this.update(cx, |this, cx| {
                if let Section::Pinned { .. } = this.section {
                    this.section = Section::Pinned {
                        can_unpin: Some(can_unpin),
                    };
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Back to the chat; `false` when it already shows.
    // shortcut: the chat opens at its newest message, not where it was left; tdesktop restores the scroll from its section stack (M8).
    pub(super) fn close_section(&mut self, cx: &mut Context<Self>) -> bool {
        if self.section == Section::Chat {
            return false;
        }
        self.set_section(Section::Chat, cx);
        true
    }

    fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        self.clear_selection(cx);
        self.shown.clear();
        self.list.reset(0);
        self.unread_bar_index = None;
        self.sync(cx);
        cx.notify();
    }

    /// Escape: drops the selection, else leaves the section; `false` when
    /// there was neither.
    pub(crate) fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        self.clear_selection(cx) || self.close_section(cx)
    }

    /// The section's bar: "3 pinned messages" after the back arrow.
    pub(super) fn pinned_section_bar(&self, cx: &Context<Self>) -> AnyElement {
        let count = i32::try_from(self.shown.len()).unwrap_or(i32::MAX);
        let title = plural(strings(cx), key::lng_pinned_messages_title, count);
        let weak = cx.weak_entity();
        top_bar::section_bar(
            title.into(),
            Box::new(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.close_section(cx));
            }),
            cx,
        )
    }

    /// "Unpin all 3 messages", or "Don't show pinned messages" for those
    /// who may not unpin (`refreshClearButtonText`).
    // shortcut: no ripple on press; ripples come with M8 polish.
    pub(super) fn pinned_section_button(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let Section::Pinned {
            can_unpin: Some(can_unpin),
        } = self.section
        else {
            return None;
        };
        let label = if can_unpin {
            let count = i32::try_from(self.shown.len().max(1)).unwrap_or(i32::MAX);
            plural(strings(cx), key::lng_pinned_unpin_all, count)
        } else {
            tr(cx, key::lng_pinned_hide_all).to_string()
        };
        let st = &HISTORY_COMPOSE_BUTTON;
        let weak = cx.weak_entity();
        Some(
            div()
                .id("history-pinned-clear")
                .flex_none()
                .h(px_of(st.height))
                .pt(px_of(st.text_top))
                .text_center()
                .cursor_pointer()
                .bg(color(cx, st.bg_color))
                .hover(|this| this.bg(color(cx, st.over_bg_color)))
                .border_t_1()
                .border_color(color(cx, ColorRef::Palette(ColorId::ShadowFg)))
                .font(font(st.font))
                .text_size(px_of(st.font.size))
                .line_height(font_height(st.font))
                .text_color(color(cx, st.color))
                .child(label.to_uppercase())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.clear_pins(can_unpin, window, cx));
                })
                .into_any_element(),
        )
    }

    /// Asks, then unpins every message (`UnpinAllMessages`) or hides the
    /// bar until a new pin (`HidePinnedBar`).
    fn clear_pins(&mut self, can_unpin: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (sure, confirm) = if can_unpin {
            (key::lng_pinned_unpin_all_sure, key::lng_pinned_unpin)
        } else {
            (key::lng_pinned_hide_all_sure, key::lng_pinned_hide_all_hide)
        };
        let mut args = ConfirmArgs::new(ui::Markup::plain(tr(cx, sure).to_string()));
        args.confirm_text = Some(tr(cx, confirm));
        args.cancel_text = Some(tr(cx, key::lng_cancel));
        let weak = cx.weak_entity();
        args.confirmed = Some(Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| {
                if can_unpin {
                    let session = this.store.read(cx).session().clone();
                    send(session.unpin_all_messages(this.chat), "unpin all", cx);
                    // TDLib tells only about loaded pins; the section is empty now.
                    this.store
                        .update(cx, |store, cx| store.set_pinned(this.chat, Vec::new(), cx));
                } else {
                    this.hide_pinned_bar(cx);
                    this.close_section(cx);
                }
            });
        }));
        boxes::confirm(&self.layers, args, window, cx);
    }
}
