//! Polls and quizzes in messages (`messagePoll`): the question, the
//! options with their results, and voting (`setPollAnswer`, `stopPoll`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::PollType;
use td_api::functions::{SetPollAnswer, StopPoll};

use crate::chat_actions::done;
use crate::ids::{ChatId, MessageId};
use crate::session::{RequestError, Session};
use crate::text::FormattedText;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollOption {
    /// The 0-based id `setPollAnswer` takes: the option's place in TDLib's
    /// list, which `option_order` may show in another order.
    pub index: i32,
    pub text: FormattedText,
    /// Known once the results show.
    pub voters: i32,
    /// 0 to 100.
    pub percentage: i32,
    pub is_chosen: bool,
    /// A vote for it is on its way.
    pub is_being_chosen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollKind {
    Regular,
    /// The correct options' indexes and the explanation, once answered.
    Quiz {
        correct: Vec<i32>,
        explanation: FormattedText,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poll {
    pub id: i64,
    pub question: FormattedText,
    /// In the order to show them.
    pub options: Vec<PollOption>,
    pub total_voters: i32,
    pub is_anonymous: bool,
    pub allows_multiple_answers: bool,
    pub allows_revoting: bool,
    pub can_see_results: bool,
    pub is_closed: bool,
    /// When it closes by itself; 0 when it does not.
    pub close_date: i32,
    pub kind: PollKind,
}

impl Poll {
    /// Whether the user voted: then, or once closed, the results show.
    #[must_use]
    pub fn is_voted(&self) -> bool {
        self.options.iter().any(|option| option.is_chosen)
    }

    /// Results show instead of the options to choose (`showVotes`).
    #[must_use]
    pub fn shows_results(&self) -> bool {
        self.is_voted() || self.is_closed
    }
}

impl From<td_api::types::Poll> for Poll {
    fn from(poll: td_api::types::Poll) -> Self {
        let mut options: Vec<PollOption> = poll
            .options
            .into_iter()
            .zip(0..)
            .map(|(option, index)| PollOption {
                index,
                text: FormattedText::from(*option.text),
                voters: option.voter_count,
                percentage: option.vote_percentage,
                is_chosen: option.is_chosen,
                is_being_chosen: option.is_being_chosen,
            })
            .collect();
        if poll.option_order.len() == options.len() {
            options.sort_by_key(|option| {
                poll.option_order
                    .iter()
                    .position(|&index| index == option.index)
            });
        }
        let kind = match *poll.r#type {
            PollType::Regular(_) => PollKind::Regular,
            PollType::Quiz(quiz) => PollKind::Quiz {
                correct: quiz.correct_option_ids,
                explanation: FormattedText::from(*quiz.explanation),
            },
        };
        Self {
            id: poll.id,
            question: FormattedText::from(*poll.question),
            options,
            total_voters: poll.total_voter_count,
            is_anonymous: poll.is_anonymous,
            allows_multiple_answers: poll.allows_multiple_answers,
            allows_revoting: poll.allows_revoting,
            can_see_results: poll.can_see_results,
            is_closed: poll.is_closed,
            close_date: poll.close_date,
            kind,
        }
    }
}

impl Session {
    /// Votes for `options` (their indexes); none takes the vote back.
    pub fn set_poll_answer(
        &self,
        chat: ChatId,
        message: MessageId,
        options: Vec<i32>,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&SetPollAnswer {
                chat_id: chat.0,
                message_id: message.0,
                option_ids: options,
            })
            .map(done)
    }

    /// Closes the poll for everyone; only its author can.
    pub fn stop_poll(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&StopPoll {
                chat_id: chat.0,
                message_id: message.0,
                reply_markup: None,
            })
            .map(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(text: &str, is_chosen: bool) -> String {
        format!(
            r#"{{"id":"","text":{{"text":"{text}","entities":[]}},"voter_count":1,"vote_percentage":50,"recent_voter_ids":[],"is_chosen":{is_chosen},"is_being_chosen":false,"addition_date":0}}"#
        )
    }

    fn poll(order: &str, is_closed: bool, chosen: bool) -> Poll {
        let json = format!(
            r#"{{"id":"7","question":{{"text":"Q","entities":[]}},"options":[{},{}],"total_voter_count":2,"recent_voter_ids":[],"can_get_voters":false,"can_see_results":true,"is_anonymous":true,"allows_multiple_answers":false,"allows_revoting":true,"members_only":false,"country_codes":[],"option_order":{order},"type":{{"@type":"pollTypeRegular"}},"open_period":0,"close_date":0,"is_closed":{is_closed}}}"#,
            option("a", false),
            option("b", chosen),
        );
        let poll: td_api::types::Poll =
            serde_json::from_str(&json).expect("fixture is a valid poll");
        Poll::from(poll)
    }

    #[test]
    fn options_follow_option_order_and_keep_their_ids() {
        let poll = poll("[1,0]", false, false);
        let order: Vec<(i32, &str)> = poll
            .options
            .iter()
            .map(|option| (option.index, option.text.text.as_str()))
            .collect();
        assert_eq!(order, [(1, "b"), (0, "a")]);
        assert_eq!(poll.question.text, "Q");
    }

    #[test]
    fn results_show_once_voted_or_closed() {
        assert!(!poll("[]", false, false).shows_results());
        assert!(poll("[]", false, true).shows_results());
        assert!(poll("[]", true, false).shows_results());
    }
}
