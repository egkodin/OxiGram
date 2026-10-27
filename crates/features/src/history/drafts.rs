//! The field's draft (`HistoryWidget::saveDraft`): loaded when the chat
//! opens, saved a second after typing stops (at most five after the first
//! unsaved change) and when the chat closes, and taken from another device
//! while nothing typed here waits to be saved.

use std::time::{Duration, Instant};

use gpui::{App, Context, Task};
use model::thread::Key;

use super::HistoryView;
use super::field_bar::FieldBar;

/// `kSaveDraftTimeout`.
const SAVE_DELAY: Duration = Duration::from_secs(1);
/// `kSaveDraftAnywayTimeout`.
const SAVE_ANYWAY: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(super) struct Draft {
    /// The draft TDLib holds, as far as this view knows; empty for none.
    saved: String,
    /// The chat's draft at the last look, to spot one from another device.
    seen: String,
    unsaved_since: Option<Instant>,
    timer: Option<Task<()>>,
}

impl Draft {
    pub(super) fn load(state: &model::State, key: Key) -> Self {
        let text = cloud_text(state, key).to_owned();
        Self {
            saved: text.clone(),
            seen: text,
            ..Self::default()
        }
    }

    pub(super) fn text(&self) -> &str {
        &self.saved
    }

    /// The field was emptied by sending, which drops the draft in TDLib.
    pub(super) fn sent(&mut self) {
        self.saved.clear();
        self.unsaved_since = None;
        self.timer = None;
    }
}

/// The chat's draft text, empty for none.
// shortcut: topic and thread drafts are saved but not read back; load them once forum topics reach the model.
fn cloud_text(state: &model::State, key: Key) -> &str {
    if key.topic.is_some() {
        return "";
    }
    state
        .chats
        .chat(key.chat)
        .and_then(|chat| chat.draft.as_ref())
        .map_or("", |draft| draft.text.text.as_str())
}

/// Whether TDLib would keep the same draft for both texts.
fn same(a: &str, b: &str) -> bool {
    a == b || (a.trim().is_empty() && b.trim().is_empty())
}

impl HistoryView {
    /// Puts off saving while the user types.
    pub(super) fn draft_changed(&mut self, cx: &mut Context<Self>) {
        let is_saved = same(
            self.compose.field.read(cx).text(),
            &self.compose.draft.saved,
        );
        let draft = &mut self.compose.draft;
        if is_saved {
            draft.unsaved_since = None;
            draft.timer = None;
            return;
        }
        let now = Instant::now();
        let since = *draft.unsaved_since.get_or_insert(now);
        if now - since >= SAVE_ANYWAY {
            self.save_draft(cx);
            return;
        }
        draft.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            // The view is gone once its chat closed, which saved already.
            let _ = this.update(cx, |this, cx| this.save_draft(cx));
        }));
    }

    /// Hands the field's text to TDLib unless it has it already.
    pub(super) fn save_draft(&mut self, cx: &mut App) {
        let text = self.compose.draft_text(cx);
        let draft = &mut self.compose.draft;
        draft.unsaved_since = None;
        draft.timer = None;
        if same(&text, &draft.saved) {
            return;
        }
        draft.saved = if text.trim().is_empty() {
            String::new()
        } else {
            text
        };
        let request = self
            .store
            .read(cx)
            .session()
            .set_draft(self.chat, self.topic, &draft.saved);
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = request.await {
                    tracing::warn!(%error, "saving a draft failed");
                }
            })
            .detach();
    }

    /// Takes a draft changed elsewhere, unless the user has typed something
    /// here that is not saved yet.
    pub(super) fn sync_draft(&mut self, cx: &mut Context<Self>) {
        // While editing the field holds the message; the draft waits.
        if matches!(self.compose.bar, Some(FieldBar::Edit { .. })) {
            return;
        }
        let cloud = cloud_text(self.store.read(cx).state(), self.key());
        if cloud == self.compose.draft.seen {
            return;
        }
        let cloud = cloud.to_owned();
        let field = self.compose.field.clone();
        let is_edited = field.read(cx).text() != self.compose.draft.saved;
        let draft = &mut self.compose.draft;
        draft.seen.clone_from(&cloud);
        if same(&cloud, &draft.saved) || is_edited {
            return;
        }
        draft.saved.clone_from(&cloud);
        field.update(cx, |field, cx| field.set_text(&cloud, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::same;

    #[test]
    fn blank_texts_are_the_same_empty_draft() {
        assert!(same("", " \n "));
        assert!(same("hi", "hi"));
        assert!(!same("hi", "hi "));
    }
}
