//! Reacting to messages: a chip toggles the user's reaction, a double
//! click adds the quick one, and the menu opens under a strip of the
//! reactions the message takes (`Reactions::Selector`).
// shortcut: no expanded selector with every emoji, no select animation on hover and no big-reaction effect; with the emoji panel's reaction tab.

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Task, div, img, prelude::FluentBuilder as _,
};
use model::MessageId;
use model::reaction::ReactionKind;
use style::chat_helpers::{
    REACT_STRIP_HEIGHT, REACT_STRIP_IMAGE, REACT_STRIP_SIZE, REACT_STRIP_SKIP,
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
/// At most this many reactions show in the strip.
const STRIP_COUNT: usize = 7;
/// The user's own reactions sit on this faint fill.
const CHOSEN_OPACITY: f32 = 0.15;

/// The strip over a message's menu.
pub(super) struct Strip {
    message: MessageId,
    kinds: Vec<ReactionKind>,
    chosen: Vec<ReactionKind>,
    /// The appear animations' clock, fresh each time the menu opens.
    instance: u64,
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
        let task = self
            .store
            .read(cx)
            .session()
            .available_reactions(self.chat, message);
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
        })
    }

    /// The strip's row of reactions; a pick closes the menu.
    pub(super) fn strip_element(&mut self, strip: &Strip, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        let over = color(cx, ColorRef::Palette(ColorId::WindowBgOver));
        let active = color(cx, ColorRef::Palette(ColorId::WindowBgActive)).opacity(CHOSEN_OPACITY);
        let buttons: Vec<_> = strip
            .kinds
            .iter()
            .take(STRIP_COUNT)
            .enumerate()
            .map(|(index, kind)| {
                let shown = picture(&self.store, kind, cx);
                let is_chosen = strip.chosen.contains(kind);
                let face = match self.strip_frame(kind, strip.instance, cx) {
                    Some(frame) => img(frame).size_full().into_any_element(),
                    None => emoji(kind, shown.as_deref()).into_any_element(),
                };
                let (weak, kind, message) = (weak.clone(), kind.clone(), strip.message);
                div()
                    .id(("reaction-strip", index))
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
            })
            .collect();
        div()
            .flex()
            .items_center()
            .h(px_of(REACT_STRIP_HEIGHT))
            .px(px_of(REACT_STRIP_SKIP) / 2.0)
            .mb(px_of(REACT_STRIP_SKIP) / 2.0)
            .rounded_full()
            .bg(color(cx, ColorRef::Palette(ColorId::MenuBg)))
            .shadow_md()
            .children(buttons)
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
