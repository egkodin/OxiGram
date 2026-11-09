//! Identifier newtypes. TDLib uses plain integers for all of them; mixing them
//! up is a type error here.

/// A Telegram user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(pub i64);

/// A custom emoji sticker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CustomEmojiId(pub i64);

/// A sticker or custom emoji set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StickerSetId(pub i64);

/// A chat: private, group, channel or secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChatId(pub i64);

/// A message inside one chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MessageId(pub i64);

/// A file TDLib knows, local or remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub i32);

/// A legacy small group, as TDLib's `basicGroup`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasicGroupId(pub i64);

/// A supergroup or channel, as TDLib's `supergroup`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SupergroupId(pub i64);

/// A chat folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FolderId(pub i32);

/// A Saved Messages topic: the messages saved from one chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SavedTopicId(pub i64);
