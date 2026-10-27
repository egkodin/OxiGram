//! Sending messages (`Api::sendMessage`). The request answers with the
//! pending message; TDLib then reports it as new, and later as sent or
//! failed, through updates the history already applies.

use std::future::Future;

use td_api::enums::{InputMessageContent, MessageTopic};
use td_api::functions::{SendBotStartMessage, SendMessage};
use td_api::types::{
    FormattedText, InputMessageText, MessageTopicSavedMessages, MessageTopicThread,
};

use crate::ids::{ChatId, UserId};
use crate::message::Topic;
use crate::session::{RequestError, Session};

/// The longest text one message takes, in UTF-16 units (`MaxMessageSize`).
// shortcut: the default limit; read the `message_text_length_max` option once options reach the model.
pub const MAX_MESSAGE_LENGTH: usize = 4096;

impl Topic {
    fn to_td(self) -> MessageTopic {
        match self {
            Self::Thread(message) => MessageTopic::Thread(MessageTopicThread {
                message_thread_id: message.0,
            }),
            Self::Saved(topic) => MessageTopic::SavedMessages(MessageTopicSavedMessages {
                saved_messages_topic_id: topic.0,
            }),
        }
    }
}

impl Session {
    /// Sends /start to a bot (`sendBotStartMessage`), for the Start bar.
    pub fn start_bot(
        &self,
        bot: UserId,
        chat: ChatId,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let request = self.client().request(&SendBotStartMessage {
            bot_user_id: bot.0,
            chat_id: chat.0,
            parameter: String::new(),
        });
        async move { request.await.map(drop).map_err(RequestError::from) }
    }

    /// Sends `text` without its outer whitespace, split into as many
    /// messages as its length needs, in order; nothing when it is blank.
    pub fn send_text(
        &self,
        chat: ChatId,
        topic: Option<Topic>,
        text: &str,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let requests: Vec<_> = split_for_sending(text, MAX_MESSAGE_LENGTH)
            .into_iter()
            .enumerate()
            .map(|(index, part)| {
                self.client().request(&SendMessage {
                    chat_id: chat.0,
                    topic_id: topic.map(|topic| Box::new(topic.to_td())),
                    reply_to: None,
                    options: None,
                    reply_markup: None,
                    input_message_content: Box::new(InputMessageContent::InputMessageText(
                        InputMessageText {
                            text: Box::new(FormattedText {
                                text: part,
                                entities: Vec::new(),
                            }),
                            link_preview_options: None,
                            // The draft goes with the first part.
                            clear_draft: index == 0,
                        },
                    )),
                })
            })
            .collect();
        async move {
            // One at a time, so the parts arrive in order.
            for request in requests {
                request.await.map_err(RequestError::from)?;
            }
            Ok(())
        }
    }
}

/// `text` trimmed and cut into parts of at most `max` UTF-16 units, each
/// at the last line break, else the last space, before the limit
/// (`TextUtilities::CutPart`).
#[must_use]
pub fn split_for_sending(text: &str, max: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let limit = utf16_prefix(rest, max);
        if limit == rest.len() {
            parts.push(rest.to_owned());
            break;
        }
        let head = &rest[..limit];
        let cut = head
            .rfind('\n')
            .or_else(|| head.rfind(char::is_whitespace))
            .filter(|cut| *cut > 0)
            .unwrap_or(limit);
        parts.push(rest[..cut].trim_end().to_owned());
        rest = rest[cut..].trim_start();
    }
    parts
}

/// The byte length of the longest prefix of `text` within `max` UTF-16
/// units.
fn utf16_prefix(text: &str, max: usize) -> usize {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        units += c.len_utf16();
        if units > max {
            return index;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_text_sends_nothing() {
        assert!(split_for_sending(" \n\t ", 10).is_empty());
    }

    #[test]
    fn short_text_is_one_trimmed_message() {
        assert_eq!(split_for_sending("  hi there \n", 4096), ["hi there"]);
    }

    #[test]
    fn long_text_breaks_at_lines_then_spaces() {
        assert_eq!(
            split_for_sending("one two\nthree four", 12),
            ["one two", "three four"]
        );
        assert_eq!(
            split_for_sending("alpha beta gamma", 12),
            ["alpha beta", "gamma"]
        );
        assert_eq!(split_for_sending("abcdefgh", 3), ["abc", "def", "gh"]);
    }

    #[test]
    fn the_limit_counts_utf16_units() {
        // Each emoji is two units.
        assert_eq!(split_for_sending("😀😀😀", 4), ["😀😀", "😀"]);
    }
}
