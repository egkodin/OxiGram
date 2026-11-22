//! Reacting to messages: a chip toggles the user's reaction, a double
//! click adds the quick one, and the menu opens under a strip of the
//! reactions the message takes (`Reactions::Selector`).
// shortcut: the expanded list has the reactions the message takes, not the emoji panel with Premium custom emoji; no select animation on hover and no big-reaction effect.

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Task, div, img, prelude::FluentBuilder as _,
};
use model::MessageId;
use model::reaction::ReactionKind;
use style::chat_helpers::{
    DEFAULT_COMPOSE_ICONS, REACT_STRIP_HEIGHT, REACT_STRIP_IMAGE, REACT_STRIP_SIZE,
    REACT_STRIP_SKIP,
};
use style::{ColorId, ColorRef};
use ui::color;

use super::HistoryView;
use super::reactions::{emoji, picture};
use crate::px_of;
use crate::store::spawn;

/// The double-click reaction when the message takes it (tdesktop's
/// default, until the account's own choice is tracked).
const QUICK: &str = "\u{2764}";
/// At most this many reactions show in the strip; with more, the last
/// place expands it (`AddedButton::Expand`).
const STRIP_COUNT: usize = 7;
/// The expanded list's columns and visible rows (`kDefaultColumns`).
const EXPANDED_COLUMNS: usize = 8;
const EXPANDED_ROWS: f32 = 6.5;
/// The user's own reactions sit on this faint fill.
const CHOSEN_OPACITY: f32 = 0.15;

fn strip_layout(count: usize, expanded: bool) -> (usize, bool) {
    if expanded || count <= STRIP_COUNT {
        (count, false)
    } else {
        (STRIP_COUNT - 1, true)
    }
}

/// The strip over a message's menu.
pub(super) struct Strip {
    message: MessageId,
    kinds: Vec<ReactionKind>,
    chosen: Vec<ReactionKind>,
    /// The appear animations' clock, fresh each time the menu opens.
    instance: u64,
    /// Every reaction shows, not the first few.
    is_expanded: bool,
}

impl Strip {
    pub(super) fn expand(&mut self) {
        self.is_expanded = true;
    }
}

impl HistoryView {
    /// Adds the reaction, or takes back the user's own; the paid one is
    /// never sent, as it spends Stars.
    pub(super) fn toggle_reaction(
        &mut self,
        message: MessageId,
        kind: &ReactionKind,
        is_chosen: bool,
        cx: &mut Context<Self>,
    ) {
        if *kind == ReactionKind::Paid {
            return;
        }
        let session = self.store.read(cx).session();
        let task = if is_chosen {
            self.stop_flying(message, kind);
            spawn(session.remove_reaction(self.chat, message, kind), cx)
        } else {
            self.start_flying(message, kind, cx);
            spawn(session.add_reaction(self.chat, message, kind), cx)
        };
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "changing a reaction failed");
            }
        })
        .detach();
    }

    /// The reactions a message takes, for the strip or the quick one.
    pub(super) fn available_reactions(
        &self,
        message: MessageId,
        cx: &App,
    ) -> Task<Vec<ReactionKind>> {
        if let Some(preset) = super::menu::preset_reactions(cx) {
            return Task::ready(preset);
        }
        let store = self.store.read(cx);
        let state = store.state();
        let is_premium = state
            .me
            .and_then(|me| state.chats.user(me))
            .is_some_and(|me| me.is_premium);
        let task = store
            .session()
            .available_reactions(self.chat, message, is_premium);
        spawn(
            async move {
                task.await
                    .inspect_err(|error| tracing::warn!(%error, "available reactions unknown"))
                    .unwrap_or_default()
            },
            cx,
        )
    }

    /// A double click on a message toggles the quick reaction.
    pub(super) fn quick_react(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(message) = self.shown.get(index) else {
            return;
        };
        let id = message.id;
        let chosen = chosen(message);
        let task = self.available_reactions(id, cx);
        cx.spawn(async move |this, cx| {
            let Some(kind) = quick(&task.await) else {
                return;
            };
            let is_chosen = chosen.contains(&kind);
            let _ = this.update(cx, |this, cx| {
                this.toggle_reaction(id, &kind, is_chosen, cx)
            });
        })
        .detach();
    }

    pub(super) fn strip(&mut self, message: MessageId, kinds: Vec<ReactionKind>) -> Option<Strip> {
        let found = self.shown.iter().find(|shown| shown.id == message)?;
        let chosen = chosen(found);
        (!kinds.is_empty()).then(|| Strip {
            message,
            kinds,
            chosen,
            instance: self.motions.next_instance(),
            is_expanded: false,
        })
    }

    /// The strip's row of reactions, or every one once expanded; a pick
    /// closes the menu.
    pub(super) fn strip_element(&mut self, strip: &Strip, cx: &mut Context<Self>) -> AnyElement {
        let (shown, show_expand) = strip_layout(strip.kinds.len(), strip.is_expanded);
        let mut buttons: Vec<AnyElement> = strip
            .kinds
            .iter()
            .take(shown)
            .enumerate()
            .map(|(index, kind)| self.strip_button(strip, index, kind, cx))
            .collect();
        let bg = color(cx, ColorRef::Palette(ColorId::MenuBg));
        if strip.is_expanded {
            let cell = px_of(REACT_STRIP_SIZE);
            #[expect(clippy::cast_precision_loss, reason = "a few columns")]
            let width = cell * shown.min(EXPANDED_COLUMNS) as f32 + px_of(REACT_STRIP_SKIP);
            return div()
                .id("reaction-list")
                .w(width)
                .max_h(cell * EXPANDED_ROWS + px_of(REACT_STRIP_SKIP))
                .overflow_y_scroll()
                .p(px_of(REACT_STRIP_SKIP) / 2.0)
                .mb(px_of(REACT_STRIP_SKIP) / 2.0)
                .flex()
                .flex_wrap()
                .rounded(px_of(REACT_STRIP_SKIP) * 2.0)
                .bg(bg)
                .shadow_md()
                .children(buttons)
                .into_any_element();
        }
        if show_expand {
            buttons.push(self.expand_button(cx));
        }
        div()
            .flex()
            .items_center()
            .h(px_of(REACT_STRIP_HEIGHT))
            .px(px_of(REACT_STRIP_SKIP) / 2.0)
            .mb(px_of(REACT_STRIP_SKIP) / 2.0)
            .rounded_full()
            .bg(bg)
            .shadow_md()
            .children(buttons)
            .into_any_element()
    }

    /// One reaction: its appear animation over the still, a click toggles.
    fn strip_button(
        &mut self,
        strip: &Strip,
        index: usize,
        kind: &ReactionKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let over = color(cx, ColorRef::Palette(ColorId::WindowBgOver));
        let active = color(cx, ColorRef::Palette(ColorId::WindowBgActive)).opacity(CHOSEN_OPACITY);
        let shown = picture(&self.store, kind, cx);
        let is_chosen = strip.chosen.contains(kind);
        let face = match self.strip_frame(kind, strip.instance, cx) {
            Some(frame) => img(frame).size_full().into_any_element(),
            None => emoji(kind, shown.as_deref(), cx).into_any_element(),
        };
        let (weak, kind, message) = (cx.weak_entity(), kind.clone(), strip.message);
        div()
            .id(("reaction-strip", index))
            .flex_none()
            .size(px_of(REACT_STRIP_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .when(is_chosen, |this| this.bg(active))
            .hover(move |this| this.bg(over))
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.toggle_reaction(message, &kind, is_chosen, cx);
                    this.menu = None;
                    cx.notify();
                });
            })
            .child(
                div()
                    .size(px_of(REACT_STRIP_IMAGE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(face),
            )
            .into_any_element()
    }

    /// The strip's last place: the arrow that shows every reaction.
    fn expand_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let over = color(cx, ColorRef::Palette(ColorId::WindowBgOver));
        let weak = cx.weak_entity();
        div()
            .id("reaction-strip-expand")
            .flex_none()
            .size(px_of(REACT_STRIP_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover(move |this| this.bg(over))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                let _ = weak.update(cx, |this, cx| {
                    this.expand_strip();
                    cx.notify();
                });
            })
            .child(ui::icon(DEFAULT_COMPOSE_ICONS.strip_expand_panel))
            .into_any_element()
    }
}

/// The user's own reactions on a message.
fn chosen(message: &model::message::Message) -> Vec<ReactionKind> {
    message
        .reactions
        .iter()
        .flat_map(|reactions| &reactions.list)
        .filter(|reaction| reaction.is_chosen)
        .map(|reaction| reaction.kind.clone())
        .collect()
}

/// ❤ when the message takes it, else the first reaction it takes.
fn quick(available: &[ReactionKind]) -> Option<ReactionKind> {
    let heart = ReactionKind::Emoji(QUICK.to_owned());
    if available.contains(&heart) {
        return Some(heart);
    }
    available.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quick_reaction_is_the_heart_when_offered_else_the_first() {
        let fire = ReactionKind::Emoji("🔥".to_owned());
        let heart = ReactionKind::Emoji(QUICK.to_owned());
        assert_eq!(quick(&[fire.clone(), heart.clone()]), Some(heart));
        assert_eq!(quick(std::slice::from_ref(&fire)), Some(fire));
        assert_eq!(quick(&[]), None);
    }
}

#[cfg(test)]
mod strip_layout_tests {
    use super::*;

    #[test]
    fn only_offers_to_expand_when_there_are_hidden_reactions() {
        assert_eq!(strip_layout(4, false), (4, false));
        assert_eq!(strip_layout(7, false), (7, false));
        assert_eq!(strip_layout(8, false), (6, true));
        assert_eq!(strip_layout(8, true), (8, false));
    }
}
