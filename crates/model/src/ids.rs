//! Identifier newtypes. TDLib uses plain integers for all of them; mixing them
//! up is a type error here.

/// A Telegram user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(pub i64);

/// A custom emoji sticker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CustomEmojiId(pub i64);
