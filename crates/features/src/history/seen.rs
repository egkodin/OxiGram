//! Messages in view are reported to TDLib (`viewMessages`), as tdesktop
//! reports them: that refreshes their views and reactions, which TDLib
//! does not reload otherwise, and reads them.

use gpui::Context;
use model::MessageId;

use super::HistoryView;
use crate::store::spawn;

/// At most this many items are checked below the top one per pass.
const MAX_IN_VIEW: usize = 40;

impl HistoryView {
    /// Reports the items now in view that were not reported yet; nothing
    /// is sent with `Views::Withhold`.
    pub(super) fn view_visible(&mut self, cx: &mut Context<Self>) {
        let top = self.list.logical_scroll_top().item_ix;
        let ids: Vec<MessageId> = (top..self.shown.len().min(top + MAX_IN_VIEW))
            .take_while(|&index| self.list.item_is_below_viewport(index) == Some(false))
            .filter_map(|index| self.shown.get(index))
            .filter(|message| message.schedule.is_none() && message.id.0 > 0)
            .map(|message| message.id)
            .filter(|id| self.viewed.insert(*id))
            .collect();
        if ids.is_empty() {
            return;
        }
        let task = self.store.read(cx).session().view_messages(self.chat, ids);
        spawn(
            async move {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "reporting messages in view failed");
                }
            },
            cx,
        )
        .detach();
    }
}
