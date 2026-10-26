//! The domain model: compact types converted from TDLib at the `tg -> model`
//! boundary, the [`State`] they build, and the [`Session`] that turns actions
//! into TDLib requests. Nothing above this crate sees a TDLib type.
//!
//! Data flows one way: TDLib update -> [`Event`] -> [`State::apply`] -> UI.
//! Responses to requests only report success or an error; every state change
//! arrives as an event.

pub mod auth;
mod event;
mod ids;
pub mod language;
pub mod phone;
mod secret;
mod session;
mod state;
pub mod text;

pub use event::{Event, events};
pub use ids::{CustomEmojiId, UserId};
pub use secret::Secret;
pub use session::{Environment, Parameters, RequestError, Session};
pub use state::{ConnectionState, State};
