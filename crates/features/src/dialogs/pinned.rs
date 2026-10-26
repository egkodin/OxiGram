//! Dragging pinned chats to reorder them (`InnerWidget::updateReorderPinned`).

use gpui::{Context, IntoElement, Render, Window};
use model::ChatId;

use super::Dialogs;

/// The pinned chat being dragged.
#[derive(Debug, Clone, Copy)]
pub(super) struct DraggedPinned(pub ChatId);

/// What follows the pointer while dragging: nothing, the row under it lights up.
// shortcut: rows do not slide out of the way as in tdesktop; upgrade with the list animations in M8.
pub(super) struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// `order` with `moved` placed where `target` is, or `None` when that
/// changes nothing or either is missing.
pub(super) fn reordered<T: Copy + PartialEq>(order: &[T], moved: T, target: T) -> Option<Vec<T>> {
    let from = order.iter().position(|item| *item == moved)?;
    let to = order.iter().position(|item| *item == target)?;
    if from == to {
        return None;
    }
    let mut order = order.to_vec();
    let item = order.remove(from);
    order.insert(to, item);
    Some(order)
}

impl Dialogs {
    /// The pinned chats of the shown list, top first.
    pub(super) fn pinned(&self, cx: &Context<Self>) -> Vec<ChatId> {
        let chats = &self.store.read(cx).state().chats;
        chats
            .list(self.list)
            .take_while(|id| {
                chats
                    .chat(*id)
                    .and_then(|chat| chat.position(self.list))
                    .is_some_and(|position| position.is_pinned)
            })
            .collect()
    }

    pub(super) fn drop_pinned(&mut self, moved: ChatId, target: ChatId, cx: &mut Context<Self>) {
        let Some(order) = reordered(&self.pinned(cx), moved, target) else {
            return;
        };
        let task = crate::store::spawn(
            self.store
                .read(cx)
                .session()
                .set_pinned_order(self.list, &order),
            cx,
        );
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "reordering pinned chats failed");
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_a_pinned_chat_onto_the_target_slot() {
        let pinned = [ChatId(1), ChatId(2), ChatId(3)];
        assert_eq!(
            reordered(&pinned, ChatId(3), ChatId(1)),
            Some(vec![ChatId(3), ChatId(1), ChatId(2)])
        );
        assert_eq!(
            reordered(&pinned, ChatId(1), ChatId(2)),
            Some(vec![ChatId(2), ChatId(1), ChatId(3)])
        );
        assert_eq!(reordered(&pinned, ChatId(2), ChatId(2)), None);
        assert_eq!(reordered(&pinned, ChatId(9), ChatId(2)), None);
    }
}
