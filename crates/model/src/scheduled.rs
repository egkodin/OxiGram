//! A chat's scheduled messages (`Data::ScheduledMessages`): a list apart
//! from the history, soonest first, with those waiting for the other user
//! to come online last.

use std::future::Future;
use std::sync::Arc;

use td_api::functions::{EditMessageSchedulingState, GetChatScheduledMessages};

use crate::chats;
use crate::history::Change;
use crate::ids::{ChatId, MessageId};
use crate::message::{Content, Message, Schedule};
use crate::session::{RequestError, Session};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Scheduled {
    messages: Vec<Arc<Message>>,
}

/// Where a message goes in the list: by its send time, then by id.
fn order(message: &Message) -> (i32, MessageId) {
    let when = match message.schedule {
        Some(Schedule::At(date)) => date,
        Some(Schedule::WhenOnline) | None => i32::MAX,
    };
    (when, message.id)
}

impl Scheduled {
    pub(crate) fn messages(&self) -> &[Arc<Message>] {
        &self.messages
    }

    pub(crate) fn set(&mut self, messages: Vec<Message>) {
        self.messages = messages
            .into_iter()
            .filter(|message| message.schedule.is_some())
            .map(Arc::new)
            .collect();
        self.messages.sort_by_key(|message| order(message));
    }

    fn remove(&mut self, id: MessageId) {
        self.messages.retain(|message| message.id != id);
    }

    /// Adds `message` or puts it where its new time places it.
    fn upsert(&mut self, message: &Message) {
        self.remove(message.id);
        if message.schedule.is_none() {
            return;
        }
        let key = order(message);
        let at = self.messages.partition_point(|known| order(known) < key);
        self.messages.insert(at, Arc::new(message.clone()));
    }

    pub(crate) fn apply(&mut self, change: &Change) {
        match change {
            Change::New(message) if message.schedule.is_some() => self.upsert(message),
            Change::SendFailed { message, old } if message.schedule.is_some() => {
                self.remove(*old);
                self.upsert(message);
            }
            Change::Deleted { messages, .. } => {
                self.messages
                    .retain(|message| !messages.contains(&message.id));
            }
            _ => {}
        }
    }

    pub(crate) fn observe(&mut self, change: &chats::Change) {
        match change {
            chats::Change::MessageSent { message, old } if message.schedule.is_some() => {
                self.remove(*old);
                self.upsert(message);
            }
            chats::Change::MessageContent {
                message, content, ..
            } => {
                if let Some(index) = self.messages.iter().position(|known| known.id == *message) {
                    let mut edited = Message::clone(&self.messages[index]);
                    edited.content = Content::clone(content);
                    self.messages[index] = Arc::new(edited);
                }
            }
            _ => {}
        }
    }
}

impl Session {
    /// `chat`'s scheduled messages (`getChatScheduledMessages`).
    pub fn scheduled_messages(
        &self,
        chat: ChatId,
    ) -> impl Future<Output = Result<Vec<Message>, RequestError>> + Send + use<> {
        let request = self
            .client()
            .request(&GetChatScheduledMessages { chat_id: chat.0 });
        async move {
            let page = request.await.map_err(RequestError::from)?;
            Ok(page
                .messages
                .into_iter()
                .flatten()
                .map(Message::from)
                .collect())
        }
    }

    /// Moves a scheduled message to `schedule`, or sends it now when `None`
    /// (`editMessageSchedulingState`).
    pub fn reschedule(
        &self,
        chat: ChatId,
        message: MessageId,
        schedule: Option<Schedule>,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let request = self.client().request(&EditMessageSchedulingState {
            chat_id: chat.0,
            message_id: message.0,
            scheduling_state: schedule.map(|schedule| Box::new(schedule.to_td())),
        });
        async move { request.await.map(drop).map_err(RequestError::from) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::UserId;
    use crate::message::Sender;

    fn scheduled(id: i64, schedule: Schedule) -> Message {
        let mut message = Message::new(
            MessageId(id),
            ChatId(1),
            Sender::User(UserId(1)),
            Content::Unsupported,
        );
        message.schedule = Some(schedule);
        message
    }

    fn ids(list: &Scheduled) -> Vec<i64> {
        list.messages().iter().map(|message| message.id.0).collect()
    }

    #[test]
    fn the_soonest_comes_first_and_when_online_last() {
        let mut list = Scheduled::default();
        list.set(vec![
            scheduled(1, Schedule::WhenOnline),
            scheduled(2, Schedule::At(300)),
            scheduled(3, Schedule::At(100)),
        ]);
        assert_eq!(ids(&list), [3, 2, 1]);
        list.apply(&Change::New(Box::new(scheduled(4, Schedule::At(200)))));
        assert_eq!(ids(&list), [3, 4, 2, 1]);
    }

    #[test]
    fn sent_and_deleted_messages_leave_or_replace() {
        let mut list = Scheduled::default();
        list.apply(&Change::New(Box::new(scheduled(10, Schedule::At(100)))));
        list.observe(&chats::Change::MessageSent {
            message: Box::new(scheduled(11, Schedule::At(100))),
            old: MessageId(10),
        });
        assert_eq!(ids(&list), [11]);
        list.apply(&Change::Deleted {
            chat: ChatId(1),
            messages: vec![MessageId(11)],
        });
        assert!(list.messages().is_empty());
    }
}
