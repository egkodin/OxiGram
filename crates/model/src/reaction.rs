//! Reactions under messages (`messageReactions`) and who added them
//! (`getMessageAddedReactions`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::ReactionType;
use td_api::functions::GetMessageAddedReactions;
use td_api::types::{ReactionTypeCustomEmoji, ReactionTypeEmoji, ReactionTypePaid};

use crate::ids::{ChatId, CustomEmojiId, MessageId};
use crate::message::Sender;
use crate::session::{RequestError, Session};

/// A message's reactions, in the order TDLib sends them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reactions {
    pub list: Vec<Reaction>,
    /// Saved Messages tags instead of reactions.
    pub are_tags: bool,
    /// Whether `getMessageAddedReactions` may list who reacted.
    pub can_get_added: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub kind: ReactionKind,
    pub count: i32,
    /// The user's own.
    pub is_chosen: bool,
    /// Who reacted last, as far as TDLib tells.
    pub recent: Vec<Sender>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReactionKind {
    Emoji(String),
    CustomEmoji(CustomEmojiId),
    /// Telegram Stars.
    Paid,
}

impl From<td_api::types::MessageReactions> for Reactions {
    fn from(reactions: td_api::types::MessageReactions) -> Self {
        Self {
            list: reactions
                .reactions
                .into_iter()
                .map(|reaction| Reaction {
                    kind: ReactionKind::from(*reaction.r#type),
                    count: reaction.total_count,
                    is_chosen: reaction.is_chosen,
                    recent: reaction
                        .recent_sender_ids
                        .into_iter()
                        .map(Sender::from)
                        .collect(),
                })
                .collect(),
            are_tags: reactions.are_tags,
            can_get_added: reactions.can_get_added_reactions,
        }
    }
}

/// A page of who reacted to a message, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddedReactions {
    /// All of them, not only this page.
    pub total: i32,
    pub list: Vec<AddedReaction>,
    /// Empty after the last page.
    pub next_offset: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedReaction {
    pub sender: Sender,
    pub kind: ReactionKind,
}

impl Session {
    /// Who reacted to a message, all reactions or one `kind`, from an
    /// `offset` a previous page gave.
    pub fn added_reactions(
        &self,
        chat: ChatId,
        message: MessageId,
        kind: Option<&ReactionKind>,
        offset: String,
        limit: i32,
    ) -> impl Future<Output = Result<AddedReactions, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessageAddedReactions {
                chat_id: chat.0,
                message_id: message.0,
                reaction_type: kind.map(|kind| Box::new(kind.to_td())),
                offset,
                limit,
            })
            .map(|result| {
                result
                    .map(|added| AddedReactions {
                        total: added.total_count,
                        list: added
                            .reactions
                            .into_iter()
                            .map(|reaction| AddedReaction {
                                sender: Sender::from(*reaction.sender_id),
                                kind: ReactionKind::from(*reaction.r#type),
                            })
                            .collect(),
                        next_offset: added.next_offset,
                    })
                    .map_err(RequestError::from)
            })
    }
}

impl ReactionKind {
    pub(crate) fn to_td(&self) -> ReactionType {
        match self {
            Self::Emoji(emoji) => ReactionType::Emoji(ReactionTypeEmoji {
                emoji: emoji.clone(),
            }),
            Self::CustomEmoji(id) => ReactionType::CustomEmoji(ReactionTypeCustomEmoji {
                custom_emoji_id: id.0,
            }),
            Self::Paid => ReactionType::Paid(ReactionTypePaid {}),
        }
    }
}

impl From<ReactionType> for ReactionKind {
    fn from(kind: ReactionType) -> Self {
        match kind {
            ReactionType::Emoji(emoji) => Self::Emoji(emoji.emoji),
            ReactionType::CustomEmoji(custom) => {
                Self::CustomEmoji(CustomEmojiId(custom.custom_emoji_id))
            }
            ReactionType::Paid(_) => Self::Paid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaction_kinds_survive_the_trip_to_tdlib() {
        for kind in [
            ReactionKind::Emoji("👍".to_owned()),
            ReactionKind::CustomEmoji(CustomEmojiId(7)),
            ReactionKind::Paid,
        ] {
            assert_eq!(ReactionKind::from(kind.to_td()), kind);
        }
    }
}
