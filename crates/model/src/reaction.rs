//! Reactions under messages (`messageReactions`).

use td_api::enums::ReactionType;

use crate::ids::CustomEmojiId;
use crate::message::Sender;

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
