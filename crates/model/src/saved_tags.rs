//! Tags the user puts on Saved Messages (`Data::Reactions::myTags`), and
//! the search by them.

use std::future::Future;

use futures::FutureExt as _;
use td_api::functions::{GetSavedMessagesTags, SearchSavedMessages};

use crate::ids::SavedTopicId;
use crate::reaction::ReactionKind;
use crate::search::FoundMessages;
use crate::session::{RequestError, Session};

/// The newest matches only, as in the chat search.
const SEARCH_LIMIT: i32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedTag {
    pub kind: ReactionKind,
    /// The name the user gave it; empty in a topic's list.
    pub label: String,
    /// How many messages carry it; 0 for a named tag left unused.
    pub count: i32,
}

impl SavedTag {
    /// "Trips 3", "Trips", "3" or nothing (`ComposeText`).
    pub fn text(&self) -> String {
        let count = (self.count > 0).then(|| self.count.to_string());
        [
            Some(self.label.clone()).filter(|label| !label.is_empty()),
            count,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
    }
}

fn topic_id(topic: Option<SavedTopicId>) -> i64 {
    topic.map_or(0, |topic| topic.0)
}

impl Session {
    /// The tags used in Saved Messages, or in one of its topics.
    pub fn saved_messages_tags(
        &self,
        topic: Option<SavedTopicId>,
    ) -> impl Future<Output = Result<Vec<SavedTag>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetSavedMessagesTags {
                saved_messages_topic_id: topic_id(topic),
            })
            .map(|result| {
                let tags = result.map_err(RequestError::from)?;
                Ok(tags
                    .tags
                    .into_iter()
                    .map(|tag| SavedTag {
                        kind: ReactionKind::from(*tag.tag),
                        label: tag.label,
                        count: tag.count,
                    })
                    .collect())
            })
    }

    /// Saved messages carrying `tag` and containing `query`, newest first.
    // shortcut: the newest 100 matches, as `search_chat`.
    pub fn search_saved(
        &self,
        topic: Option<SavedTopicId>,
        tag: Option<&ReactionKind>,
        query: String,
    ) -> impl Future<Output = Result<FoundMessages, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchSavedMessages {
                saved_messages_topic_id: topic_id(topic),
                tag: tag.map(|tag| Box::new(tag.to_td())),
                query,
                from_message_id: 0,
                offset: 0,
                limit: SEARCH_LIMIT,
            })
            .map(|result| result.map(FoundMessages::from).map_err(RequestError::from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_reads_as_its_name_then_its_count() {
        let tag = |label: &str, count| SavedTag {
            kind: ReactionKind::Emoji("👍".to_owned()),
            label: label.to_owned(),
            count,
        };
        assert_eq!(tag("Trips", 3).text(), "Trips 3");
        assert_eq!(tag("Trips", 0).text(), "Trips");
        assert_eq!(tag("", 3).text(), "3");
        assert_eq!(tag("", 0).text(), "");
    }
}
