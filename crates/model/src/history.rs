//! The messages of open chats: loaded in pages (`getChatHistory`) around
//! where the user reads, and kept current by message updates.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use futures::FutureExt as _;
use td_api::enums::SearchMessagesFilter;
use td_api::enums::Update;
use td_api::functions::{
    CloseChat, GetChatHistory, GetChatMessageByDate, GetMessage, OpenChat, SearchChatMessages,
};
use tg::TdError;

use crate::chat_actions::done;
use crate::chats;
use crate::ids::{ChatId, MessageId};
use crate::message::{Content, Message};
use crate::session::{RequestError, Session};

/// How many messages one page asks for; TDLib may return fewer.
pub const PAGE: i32 = 50;
/// TDLib's answer when no message is that old.
const NOT_FOUND: i32 = 404;

/// The most messages one `searchChatMessages` request returns.
const SEARCH_LIMIT: i32 = 100;

/// Messages the chat counts as unread besides its messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    Mentions,
    Reactions,
}

/// A change to messages that only the history shows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    New(Box<Message>),
    Edited {
        chat: ChatId,
        message: MessageId,
        edit_date: i32,
    },
    /// Messages deleted for good; TDLib also reports ones it only dropped
    /// from its cache, which stay.
    Deleted {
        chat: ChatId,
        messages: Vec<MessageId>,
    },
    /// An outgoing message the server refused, under its new id.
    SendFailed {
        message: Box<Message>,
        old: MessageId,
    },
    Views {
        chat: ChatId,
        message: MessageId,
        views: i32,
    },
    Pinned {
        chat: ChatId,
        message: MessageId,
        is_pinned: bool,
    },
}

/// Which part of a history a page asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// The newest messages.
    Newest,
    /// Messages around `MessageId`, as jumping to the first unread does.
    Around(MessageId),
    /// Messages older than the oldest loaded one.
    Older(MessageId),
    /// Messages newer than the newest loaded one.
    Newer(MessageId),
}

impl Page {
    /// `getChatHistory`'s `from_message_id`, `offset` and `limit`. A
    /// negative offset adds that many newer messages.
    fn to_td(self) -> (i64, i32) {
        match self {
            Self::Newest => (0, 0),
            Self::Around(message) => (message.0, -PAGE / 2),
            Self::Older(message) => (message.0, 0),
            Self::Newer(message) => (message.0, 1 - PAGE),
        }
    }
}

/// Whether more messages exist past one end of what is loaded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Edge {
    #[default]
    More,
    Reached,
}

/// One chat's loaded messages: a single run without gaps, oldest first.
/// A changed message is a new `Arc`, so views can tell what to redraw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    messages: Vec<Arc<Message>>,
    older: Edge,
    newer: Edge,
    /// Messages outside the loaded run that loaded ones point at: replied
    /// and pinned ones, perhaps in other chats.
    // shortcut: references do not follow edits and deletions; a reopened chat fetches them again.
    references: HashMap<(ChatId, MessageId), Arc<Message>>,
    /// The chat's pinned messages, oldest first.
    pinned: Vec<MessageId>,
}

impl History {
    #[must_use]
    pub fn messages(&self) -> &[Arc<Message>] {
        &self.messages
    }

    #[must_use]
    pub fn message(&self, id: MessageId) -> Option<&Message> {
        self.index(id).ok().map(|index| &*self.messages[index])
    }

    /// The message `id` of `chat`, loaded or referenced.
    #[must_use]
    pub fn find(&self, chat: ChatId, id: MessageId) -> Option<&Message> {
        self.references
            .get(&(chat, id))
            .map(|message| &**message)
            .or_else(|| self.message(id).filter(|message| message.chat == chat))
    }

    /// Drops the loaded run, keeping references, so a page elsewhere can
    /// start a new one.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.older = Edge::More;
        self.newer = Edge::More;
    }

    #[must_use]
    pub fn pinned(&self) -> &[MessageId] {
        &self.pinned
    }

    /// Replaces the pinned messages with loaded ones, keeping them as
    /// references so the pinned bar can show them.
    pub fn set_pinned(&mut self, messages: Vec<Message>) {
        let mut ids: Vec<MessageId> = messages.iter().map(|message| message.id).collect();
        ids.sort_unstable();
        ids.dedup();
        self.pinned = ids;
        for message in messages {
            self.add_reference(message);
        }
    }

    pub fn add_reference(&mut self, message: Message) {
        self.references
            .insert((message.chat, message.id), Arc::new(message));
    }

    /// Whether the newest message is loaded, so new ones join the end.
    #[must_use]
    pub fn has_newest(&self) -> bool {
        self.newer == Edge::Reached
    }

    /// The page that loads older messages, unless the first one is loaded.
    #[must_use]
    pub fn older_page(&self) -> Option<Page> {
        let oldest = self.messages.first()?;
        (self.older == Edge::More).then_some(Page::Older(oldest.id))
    }

    /// The page that loads newer messages, unless the newest is loaded.
    #[must_use]
    pub fn newer_page(&self) -> Option<Page> {
        let newest = self.messages.last()?;
        (self.newer == Edge::More).then_some(Page::Newer(newest.id))
    }

    /// Adds a loaded page. A page that brings nothing past an end it asked
    /// for marks that end reached.
    pub fn add_page(&mut self, page: Page, messages: Vec<Message>) {
        let (oldest, newest) = match (self.messages.first(), self.messages.last()) {
            (Some(first), Some(last)) => (first.id, last.id),
            _ => match page {
                Page::Around(id) | Page::Older(id) | Page::Newer(id) => (id, id),
                Page::Newest => (MessageId(i64::MAX), MessageId(i64::MAX)),
            },
        };
        let brings_older = messages.iter().any(|message| message.id < oldest);
        let brings_newer = messages.iter().any(|message| message.id > newest);
        match page {
            Page::Newest => {
                self.newer = Edge::Reached;
                if messages.is_empty() {
                    self.older = Edge::Reached;
                }
            }
            Page::Older(_) if !brings_older => self.older = Edge::Reached,
            Page::Newer(_) if !brings_newer => self.newer = Edge::Reached,
            Page::Around(_) | Page::Older(_) | Page::Newer(_) => {}
        }
        for message in messages {
            self.insert(message);
        }
    }

    fn index(&self, id: MessageId) -> Result<usize, usize> {
        self.messages
            .binary_search_by_key(&id, |message| message.id)
    }

    fn insert(&mut self, message: Message) {
        match self.index(message.id) {
            Ok(index) if *self.messages[index] == message => {}
            Ok(index) => self.messages[index] = Arc::new(message),
            Err(index) => self.messages.insert(index, Arc::new(message)),
        }
    }

    fn remove(&mut self, id: MessageId) {
        if let Ok(index) = self.index(id) {
            self.messages.remove(index);
        }
    }

    fn update(&mut self, id: MessageId, change: impl FnOnce(&mut Message)) {
        if let Ok(index) = self.index(id) {
            change(Arc::make_mut(&mut self.messages[index]));
        }
    }

    /// Replaces a sent or failed message, which TDLib gives a new id.
    fn replace(&mut self, old: MessageId, message: Message) {
        if self.index(old).is_ok() {
            self.remove(old);
            self.insert(message);
        }
    }

    pub(crate) fn apply(&mut self, change: Change) {
        match change {
            Change::New(message) => {
                if self.has_newest() {
                    self.insert(*message);
                }
            }
            Change::Edited {
                message, edit_date, ..
            } => self.update(message, |message| message.edit_date = edit_date),
            Change::Deleted { messages, .. } => {
                self.pinned.retain(|id| !messages.contains(id));
                for id in messages {
                    self.remove(id);
                }
            }
            Change::SendFailed { message, old } => self.replace(old, *message),
            Change::Views { message, views, .. } => {
                self.update(message, |message| message.views = views);
            }
            Change::Pinned {
                message, is_pinned, ..
            } => match (self.pinned.binary_search(&message), is_pinned) {
                (Err(index), true) => self.pinned.insert(index, message),
                (Ok(index), false) => {
                    self.pinned.remove(index);
                }
                _ => {}
            },
        }
    }

    /// The chat list changes that touch messages.
    pub(crate) fn observe(&mut self, change: &chats::Change) {
        match change {
            chats::Change::MessageContent {
                message, content, ..
            } => {
                let content: &Content = content;
                self.update(*message, |message| message.content = content.clone());
            }
            chats::Change::MessageSent { message, old } => {
                self.replace(*old, Message::clone(message));
            }
            _ => {}
        }
    }
}

impl Change {
    /// The chat the change is about.
    #[must_use]
    pub fn chat(&self) -> ChatId {
        match self {
            Self::New(message) | Self::SendFailed { message, .. } => message.chat,
            Self::Edited { chat, .. }
            | Self::Deleted { chat, .. }
            | Self::Views { chat, .. }
            | Self::Pinned { chat, .. } => *chat,
        }
    }

    pub(crate) fn from_update(update: Update) -> Result<Self, Update> {
        Ok(match update {
            Update::NewMessage(update) => Self::New(Box::new(Message::from(*update.message))),
            Update::MessageEdited(update) => Self::Edited {
                chat: ChatId(update.chat_id),
                message: MessageId(update.message_id),
                edit_date: update.edit_date,
            },
            Update::DeleteMessages(update) if update.is_permanent => Self::Deleted {
                chat: ChatId(update.chat_id),
                messages: update.message_ids.into_iter().map(MessageId).collect(),
            },
            Update::MessageSendFailed(update) => Self::SendFailed {
                message: Box::new(Message::from(*update.message)),
                old: MessageId(update.old_message_id),
            },
            Update::MessageInteractionInfo(update) => Self::Views {
                chat: ChatId(update.chat_id),
                message: MessageId(update.message_id),
                views: update.interaction_info.map_or(0, |info| info.view_count),
            },
            Update::MessageIsPinned(update) => Self::Pinned {
                chat: ChatId(update.chat_id),
                message: MessageId(update.message_id),
                is_pinned: update.is_pinned,
            },
            other => return Err(other),
        })
    }
}

impl Session {
    /// One page of `chat`'s history, newest first as TDLib sends it.
    pub fn chat_history(
        &self,
        chat: ChatId,
        page: Page,
    ) -> impl Future<Output = Result<Vec<Message>, RequestError>> + Send + use<> {
        let (from, offset) = page.to_td();
        self.client()
            .request(&GetChatHistory {
                chat_id: chat.0,
                from_message_id: from,
                offset,
                limit: PAGE,
                only_local: false,
            })
            .map(|result| {
                result
                    .map(|page| {
                        page.messages
                            .into_iter()
                            .flatten()
                            .map(Message::from)
                            .collect()
                    })
                    .map_err(RequestError::from)
            })
    }

    /// `chat`'s pinned messages, newest first.
    // shortcut: only the newest 100 pins; page with next_from_message_id when a chat has more.
    pub fn pinned_messages(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<Vec<Message>, RequestError>> + Send + use<> {
        self.filtered_messages(
            chat,
            SearchMessagesFilter::Pinned(td_api::types::SearchMessagesFilterPinned {}),
        )
    }

    /// `chat`'s unread mentions or reactions, newest first.
    // shortcut: only the newest 100; the oldest beyond them waits until these are read.
    pub fn unread_messages(
        &self,
        chat: ChatId,
        unread: Unread,
    ) -> impl Future<Output = Result<Vec<Message>, RequestError>> + Send + use<> {
        let filter = match unread {
            Unread::Mentions => SearchMessagesFilter::UnreadMention(
                td_api::types::SearchMessagesFilterUnreadMention {},
            ),
            Unread::Reactions => SearchMessagesFilter::UnreadReaction(
                td_api::types::SearchMessagesFilterUnreadReaction {},
            ),
        };
        self.filtered_messages(chat, filter)
    }

    fn filtered_messages(
        &self,
        chat: ChatId,
        filter: SearchMessagesFilter,
    ) -> impl Future<Output = Result<Vec<Message>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchChatMessages {
                chat_id: chat.0,
                topic_id: None,
                query: String::new(),
                sender_id: None,
                from_message_id: 0,
                offset: 0,
                limit: SEARCH_LIMIT,
                filter: Some(Box::new(filter)),
            })
            .map(|result| {
                result
                    .map(|found| found.messages.into_iter().map(Message::from).collect())
                    .map_err(RequestError::from)
            })
    }

    /// The last message of `chat` sent before `date` (Unix time); none when
    /// the chat starts later, as a jump to a day needs.
    pub fn message_before(
        &self,
        chat: ChatId,
        date: i64,
    ) -> impl Future<Output = Result<Option<MessageId>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetChatMessageByDate {
                chat_id: chat.0,
                date: i32::try_from(date - 1).unwrap_or(i32::MAX),
            })
            .map(|result| match result {
                Ok(message) => Ok(Some(MessageId(message.id))),
                Err(TdError::Td {
                    code: NOT_FOUND, ..
                }) => Ok(None),
                Err(error) => Err(RequestError::from(error)),
            })
    }

    /// Tells TDLib the user looks at `chat`, so it sends view counts and
    /// other changes only open chats get.
    pub fn open_chat(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&OpenChat { chat_id: chat.0 })
            .map(done)
    }

    /// One message, as a reply or a pin points at it.
    pub fn message(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<Message, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessage {
                chat_id: chat.0,
                message_id: message.0,
            })
            .map(|result| result.map(Message::from).map_err(RequestError::from))
    }

    pub fn close_chat(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&CloseChat { chat_id: chat.0 })
            .map(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::UserId;
    use crate::message::Sender;
    use crate::text::FormattedText;

    fn message(id: i64) -> Message {
        Message::new(
            MessageId(id),
            ChatId(1),
            Sender::User(UserId(2)),
            Content::text(FormattedText::plain(id.to_string())),
        )
    }

    fn ids(history: &History) -> Vec<i64> {
        history
            .messages()
            .iter()
            .map(|message| message.id.0)
            .collect()
    }

    fn page(ids: impl IntoIterator<Item = i64>) -> Vec<Message> {
        ids.into_iter().map(message).collect()
    }

    #[test]
    fn pages_merge_in_order_and_mark_the_ends() {
        let mut history = History::default();
        history.add_page(Page::Newest, page([30, 20]));
        assert!(history.has_newest());
        assert_eq!(history.newer_page(), None);
        assert_eq!(history.older_page(), Some(Page::Older(MessageId(20))));
        // TDLib repeats the anchor message.
        history.add_page(Page::Older(MessageId(20)), page([20, 10]));
        assert_eq!(ids(&history), [10, 20, 30]);
        history.add_page(Page::Older(MessageId(10)), page([10]));
        assert_eq!(history.older_page(), None);
    }

    #[test]
    fn a_page_around_a_message_loads_both_ways() {
        let mut history = History::default();
        history.add_page(Page::Around(MessageId(20)), page([30, 20, 10]));
        assert!(!history.has_newest());
        history.apply(Change::New(Box::new(message(40))));
        assert_eq!(ids(&history), [10, 20, 30], "a gap would open");
        assert_eq!(history.newer_page(), Some(Page::Newer(MessageId(30))));
        history.add_page(Page::Newer(MessageId(30)), page([40, 30]));
        history.add_page(Page::Newer(MessageId(40)), page([40]));
        assert!(history.has_newest());
        history.apply(Change::New(Box::new(message(50))));
        assert_eq!(ids(&history), [10, 20, 30, 40, 50]);
    }

    #[test]
    fn updates_edit_replace_and_delete_messages() {
        let mut history = History::default();
        history.add_page(Page::Newest, page([3, 2, 1]));
        history.observe(&chats::Change::MessageSent {
            message: Box::new(message(10)),
            old: MessageId(2),
        });
        assert_eq!(ids(&history), [1, 3, 10]);
        history.apply(Change::Edited {
            chat: ChatId(1),
            message: MessageId(3),
            edit_date: 7,
        });
        assert_eq!(history.message(MessageId(3)).map(|m| m.edit_date), Some(7));
        history.apply(Change::Deleted {
            chat: ChatId(1),
            messages: vec![MessageId(1), MessageId(99)],
        });
        assert_eq!(ids(&history), [3, 10]);
    }

    #[test]
    fn pins_stay_sorted_and_follow_updates() {
        let mut history = History::default();
        history.set_pinned(page([30, 10, 20]));
        assert_eq!(
            history.pinned(),
            [MessageId(10), MessageId(20), MessageId(30)]
        );
        assert!(history.find(ChatId(1), MessageId(20)).is_some());
        let pin = |id, is_pinned| Change::Pinned {
            chat: ChatId(1),
            message: MessageId(id),
            is_pinned,
        };
        history.apply(pin(25, true));
        history.apply(pin(25, true));
        history.apply(pin(10, false));
        history.apply(Change::Deleted {
            chat: ChatId(1),
            messages: vec![MessageId(30)],
        });
        assert_eq!(history.pinned(), [MessageId(20), MessageId(25)]);
    }
}
