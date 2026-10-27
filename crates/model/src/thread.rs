//! Parts of a chat with a history of their own: a post's comments, a reply
//! thread, a Saved Messages topic (`Data::RepliesList`, `Data::SavedSublist`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{GetMessageThread, GetMessageThreadHistory, GetSavedMessagesTopicHistory};
use td_api::types::Messages;

use crate::history::{PAGE, Page};
use crate::ids::{ChatId, MessageId};
use crate::message::{Message, Replies, Topic};
use crate::session::{RequestError, Session};

/// Which messages a history holds: a whole chat or one topic of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub chat: ChatId,
    pub topic: Option<Topic>,
}

impl From<ChatId> for Key {
    fn from(chat: ChatId) -> Self {
        Self { chat, topic: None }
    }
}

/// A thread as `getMessageThread` tells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    /// The thread's own history: for a channel post, in its discussion group.
    pub key: Key,
    /// The messages it starts from, oldest first: the post, or an album.
    pub starters: Vec<Message>,
    pub replies: Option<Replies>,
}

impl Session {
    /// One page of the messages `key` holds, newest first as TDLib sends them.
    pub fn history_page(
        &self,
        key: Key,
        page: Page,
    ) -> impl Future<Output = Result<Vec<Message>, RequestError>> + Send + use<> {
        let (from_message_id, offset) = page.to_td();
        match key.topic {
            None => self.chat_history(key.chat, page).boxed(),
            Some(Topic::Thread(root)) => self
                .client()
                .request(&GetMessageThreadHistory {
                    chat_id: key.chat.0,
                    message_id: root.0,
                    from_message_id,
                    offset,
                    limit: PAGE,
                })
                .map(messages)
                .boxed(),
            Some(Topic::Saved(topic)) => self
                .client()
                .request(&GetSavedMessagesTopicHistory {
                    saved_messages_topic_id: topic.0,
                    from_message_id,
                    offset,
                    limit: PAGE,
                })
                .map(messages)
                .boxed(),
        }
    }

    /// The thread of replies to `message`, which for a channel post lives
    /// in the discussion group.
    pub fn message_thread(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<Thread, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessageThread {
                chat_id: chat.0,
                message_id: message.0,
            })
            .map(|result| {
                let info = result.map_err(RequestError::from)?;
                let mut starters: Vec<Message> =
                    info.messages.into_iter().map(Message::from).collect();
                starters.reverse();
                Ok(Thread {
                    key: Key {
                        chat: ChatId(info.chat_id),
                        topic: Some(Topic::Thread(MessageId(info.message_thread_id))),
                    },
                    starters,
                    replies: info.reply_info.map(|replies| Replies::from(*replies)),
                })
            })
    }
}

fn messages(result: Result<Messages, tg::TdError>) -> Result<Vec<Message>, RequestError> {
    result
        .map(|page| {
            page.messages
                .into_iter()
                .flatten()
                .map(Message::from)
                .collect()
        })
        .map_err(RequestError::from)
}
