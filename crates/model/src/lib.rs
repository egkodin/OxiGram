//! The domain model: compact types converted from TDLib at the `tg -> model`
//! boundary, the [`State`] they build, and the [`Session`] that turns actions
//! into TDLib requests. Nothing above this crate sees a TDLib type.
//!
//! Data flows one way: TDLib update -> [`Event`] -> [`State::apply`] -> UI.
//! Responses to requests only report success or an error; every state change
//! arrives as an event.

pub mod action_bar;
pub mod auth;
pub mod chat;
pub mod chat_actions;
pub mod chats;
mod event;
pub mod file;
pub mod history;
mod ids;
pub mod language;
pub mod message;
pub mod message_actions;
pub mod phone;
pub mod reaction;
pub mod rights;
pub mod saved_tags;
pub mod search;
mod secret;
pub mod send;
mod session;
pub mod sponsored;
mod state;
pub mod text;
pub mod text_links;
pub mod thread;
pub mod user;

pub use event::{Event, events};
pub use ids::{
    BasicGroupId, ChatId, CustomEmojiId, FileId, FolderId, MessageId, SavedTopicId, SupergroupId,
    UserId,
};
pub use secret::Secret;
pub use session::{Environment, ListLoad, Parameters, RequestError, Session, Views};
pub use state::{ConnectionState, State};
