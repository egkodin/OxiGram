//! A post's comments (`ChatWidget` over a `RepliesList`): the thread in the
//! discussion group, under the post it starts from and a divider.

use std::sync::Arc;

use gpui::{AnyElement, Context, Entity, FocusHandle, IntoElement as _};
use i18n::key;
use model::message::Message;
use model::thread::Thread;

use super::service::plural;
use super::{HistoryEvent, HistoryView, bubble, top_bar};
use crate::Store;
use crate::lang::{strings, tr};
use crate::layers::Layers;

/// What a thread shows over its replies.
#[derive(Debug, Clone)]
pub(super) struct ThreadHead {
    /// The post, or its album, oldest first.
    starters: Vec<Arc<Message>>,
    // shortcut: the count as of opening; follows new comments with the replies list's full count (M4, sending).
    count: i32,
}

impl HistoryView {
    pub(crate) fn thread(
        store: Entity<Store>,
        layers: Entity<Layers>,
        thread: Thread,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::new(store, layers, thread.key, focus, cx);
        view.thread = Some(ThreadHead {
            starters: thread.starters.into_iter().map(Arc::new).collect(),
            count: thread.replies.map_or(0, |replies| replies.count),
        });
        view.sync(cx);
        view
    }

    /// The replies after the post they answer, once the oldest one is
    /// loaded; `None` outside threads.
    pub(super) fn with_starters(
        &self,
        replies: &[Arc<Message>],
        has_oldest: bool,
    ) -> Option<Vec<Arc<Message>>> {
        let head = self.thread.as_ref()?;
        Some(compose(&head.starters, replies, has_oldest))
    }

    /// "Discussion started" under the post, or "No comments here yet..."
    /// (`RepliesList::injectRootDivider`).
    pub(super) fn thread_divider(&self, index: usize, cx: &Context<Self>) -> Option<AnyElement> {
        let head = self.thread.as_ref()?;
        let last = head.starters.last()?;
        if self
            .shown
            .get(index)
            .is_none_or(|message| message.id != last.id)
        {
            return None;
        }
        let text = if self.shown.len() > index + 1 {
            key::lng_replies_discussion_started
        } else {
            key::lng_replies_no_comments
        };
        Some(bubble::service_pill(tr(cx, text), cx).into_any_element())
    }

    /// "12 comments" after the back arrow.
    pub(super) fn thread_bar(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let head = self.thread.as_ref()?;
        let title = if head.count > 0 {
            plural(strings(cx), key::lng_comments_header, head.count)
        } else {
            tr(cx, key::lng_comments_header_none).to_string()
        };
        let weak = cx.weak_entity();
        Some(top_bar::section_bar(
            title.into(),
            Box::new(move |_, _, cx| {
                let _ = weak.update(cx, |_, cx| cx.emit(HistoryEvent::Close));
            }),
            cx,
        ))
    }

    /// Opens the comments under a channel post (`ShowCommentsClickHandler`).
    pub(super) fn open_comments(&mut self, post: model::MessageId, cx: &mut Context<Self>) {
        let task = crate::store::spawn(
            self.store
                .read(cx)
                .session()
                .message_thread(self.chat, post),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |_, cx| match result {
                Ok(thread) => cx.emit(HistoryEvent::OpenThread(Box::new(thread))),
                Err(error) => tracing::warn!(%error, "opening comments failed"),
            });
        })
        .detach();
    }
}

/// The starters, then the replies without them; the starters wait until
/// no older reply is left to load.
fn compose(
    starters: &[Arc<Message>],
    replies: &[Arc<Message>],
    has_oldest: bool,
) -> Vec<Arc<Message>> {
    if !has_oldest {
        return replies.to_vec();
    }
    let starts = |message: &Arc<Message>| starters.iter().any(|s| s.id == message.id);
    starters
        .iter()
        .cloned()
        .chain(replies.iter().filter(|m| !starts(m)).cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::message::{Content, Sender};
    use model::text::FormattedText;
    use model::{ChatId, MessageId, UserId};

    fn messages(ids: &[i64]) -> Vec<Arc<Message>> {
        ids.iter()
            .map(|&id| {
                Arc::new(Message::new(
                    MessageId(id),
                    ChatId(-100),
                    Sender::User(UserId(2)),
                    Content::text(FormattedText::plain("")),
                ))
            })
            .collect()
    }

    fn ids(messages: &[Arc<Message>]) -> Vec<i64> {
        messages.iter().map(|message| message.id.0).collect()
    }

    #[test]
    fn the_post_heads_its_replies_once_the_oldest_reply_is_loaded() {
        let (post, replies) = (messages(&[10]), messages(&[10, 11, 12]));
        assert_eq!(ids(&compose(&post, &replies, false)), [10, 11, 12]);
        assert_eq!(
            ids(&compose(&post, &replies[1..], false)),
            [11, 12],
            "older replies may still come between"
        );
        assert_eq!(
            ids(&compose(&post, &replies, true)),
            [10, 11, 12],
            "a post TDLib also sent as a reply shows once"
        );
        assert_eq!(ids(&compose(&post, &[], true)), [10]);
    }
}
