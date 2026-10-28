//! Checklists in messages (`messageChecklist`): the title, the tasks and
//! who did them, and marking tasks done (`markChecklistTasksAsDone`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::MarkChecklistTasksAsDone;

use crate::chat_actions::done;
use crate::ids::{ChatId, MessageId};
use crate::message::Sender;
use crate::session::{RequestError, Session};
use crate::text::FormattedText;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: i32,
    pub text: FormattedText,
    /// Who marked it done, while it is.
    pub completed_by: Option<Sender>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checklist {
    pub title: FormattedText,
    pub tasks: Vec<Task>,
    pub others_can_mark_tasks_as_done: bool,
    pub can_mark_tasks_as_done: bool,
}

impl Checklist {
    /// How many tasks are done.
    #[must_use]
    pub fn done_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.completed_by.is_some())
            .count()
    }
}

impl From<td_api::types::Checklist> for Checklist {
    fn from(list: td_api::types::Checklist) -> Self {
        Self {
            title: FormattedText::from(*list.title),
            tasks: list
                .tasks
                .into_iter()
                .map(|task| Task {
                    id: task.id,
                    text: FormattedText::from(*task.text),
                    completed_by: task.completed_by.map(|sender| Sender::from(*sender)),
                })
                .collect(),
            others_can_mark_tasks_as_done: list.others_can_mark_tasks_as_done,
            can_mark_tasks_as_done: list.can_mark_tasks_as_done,
        }
    }
}

/// Which way a task is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Done,
    NotDone,
}

impl Session {
    /// Marks task `id` done or not done.
    pub fn mark_checklist_task(
        &self,
        chat: ChatId,
        message: MessageId,
        id: i32,
        mark: Mark,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let (done_ids, not_done_ids) = match mark {
            Mark::Done => (vec![id], Vec::new()),
            Mark::NotDone => (Vec::new(), vec![id]),
        };
        self.client()
            .request(&MarkChecklistTasksAsDone {
                chat_id: chat.0,
                message_id: message.0,
                marked_as_done_task_ids: done_ids,
                marked_as_not_done_task_ids: not_done_ids,
            })
            .map(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::UserId;

    #[test]
    fn tasks_keep_who_did_them() {
        let list: td_api::types::Checklist = serde_json::from_str(
            r#"{"title":{"text":"Trip","entities":[]},"tasks":[
                {"id":1,"text":{"text":"Tickets","entities":[]},"completed_by":{"@type":"messageSenderUser","user_id":5},"completion_date":100},
                {"id":2,"text":{"text":"Hotel","entities":[]},"completion_date":0}],
                "others_can_add_tasks":false,"can_add_tasks":true,"others_can_mark_tasks_as_done":true,"can_mark_tasks_as_done":true}"#,
        )
        .expect("fixture is a valid checklist");
        let list = Checklist::from(list);
        assert_eq!(list.tasks[0].completed_by, Some(Sender::User(UserId(5))));
        assert_eq!(list.tasks[1].completed_by, None);
        assert_eq!(list.done_count(), 1);
    }
}
