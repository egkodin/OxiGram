//! Reactions under messages (`messageReactions`) and who added them
//! (`getMessageAddedReactions`).

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::ReactionType;
use td_api::functions::{
    AddMessageReaction, GetMessageAddedReactions, GetMessageAvailableReactions,
    RemoveMessageReaction,
};
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

/// How many reactions TDLib lays out per row of the picker.
const PICKER_ROW: i32 = 8;

impl Session {
    /// Adds the user's reaction (`addMessageReaction`); never the paid one,
    /// which costs Stars.
    pub fn add_reaction(
        &self,
        chat: ChatId,
        message: MessageId,
        kind: &ReactionKind,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&AddMessageReaction {
                chat_id: chat.0,
                message_id: message.0,
                reaction_type: Box::new(kind.to_td()),
                is_big: false,
                update_recent_reactions: true,
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    pub fn remove_reaction(
        &self,
        chat: ChatId,
        message: MessageId,
        kind: &ReactionKind,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&RemoveMessageReaction {
                chat_id: chat.0,
                message_id: message.0,
                reaction_type: Box::new(kind.to_td()),
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    /// The reactions the user may add to a message, top ones first, without
    /// the paid one and those that need Premium.
    pub fn available_reactions(
        &self,
        chat: ChatId,
        message: MessageId,
    ) -> impl Future<Output = Result<Vec<ReactionKind>, RequestError>> + Send + use<> {
        self.client()
            .request(&GetMessageAvailableReactions {
                chat_id: chat.0,
                message_id: message.0,
                row_size: PICKER_ROW,
            })
            .map(|result| {
                result.map_err(RequestError::from).map(|available| {
                    let all = available
                        .top_reactions
                        .into_iter()
                        .chain(available.recent_reactions)
                        .chain(available.popular_reactions);
                    pickable(
                        all.filter(|reaction| !reaction.needs_premium)
                            .map(|reaction| ReactionKind::from(*reaction.r#type)),
                    )
                })
            })
    }
}

/// Each reaction once, in order, without the paid one.
fn pickable(all: impl Iterator<Item = ReactionKind>) -> Vec<ReactionKind> {
    let mut list: Vec<ReactionKind> = Vec::new();
    for kind in all {
        if kind != ReactionKind::Paid && !list.contains(&kind) {
            list.push(kind);
        }
    }
    list
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

    #[test]
    fn the_picker_keeps_each_reaction_once_and_drops_the_paid_one() {
        let heart = ReactionKind::Emoji("❤".to_owned());
        let fire = ReactionKind::Emoji("🔥".to_owned());
        let all = [
            heart.clone(),
            ReactionKind::Paid,
            fire.clone(),
            heart.clone(),
        ];
        assert_eq!(pickable(all.into_iter()), [heart, fire]);
    }
}
