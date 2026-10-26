//! What a chat's row says while someone in it acts: typing, recording,
//! uploading (`SendActionPainter`).

use i18n::{Key, Strings, key};
use model::chat::Chat;
use model::chats::{ChatAction, Chats};
use model::message::Sender;

use super::preview;

/// "typing" in private chats, "Ann is typing" in groups, for the first
/// one acting (`SendActionPainter`).
// shortcut: one person and no animation; "Ann and Bob are typing" and the animated dots come with the chat top bar in M3.
pub(crate) fn action_text(strings: &Strings, chats: &Chats, chat: &Chat) -> Option<String> {
    let (sender, action) = chats.actions(chat.id).first()?;
    let (alone, with_user) = action_keys(action);
    let emoji = match action {
        ChatAction::WatchingAnimations(emoji) => emoji.as_str(),
        _ => "",
    };
    if chat.user().is_some() {
        return Some(i18n::fill(strings.text(alone), &[("emoji", emoji)]));
    }
    let name = match sender {
        Sender::User(user) => chats
            .user(*user)
            .map(|user| preview::short_name(strings, user)),
        Sender::Chat(id) => chats.chat(*id).map(|chat| chat.title.clone()),
    }?;
    Some(i18n::fill(
        strings.text(with_user),
        &[("user", name.as_str()), ("emoji", emoji)],
    ))
}

fn action_keys(action: &ChatAction) -> (Key, Key) {
    match action {
        ChatAction::Typing | ChatAction::ChoosingLocation | ChatAction::ChoosingContact => {
            (key::lng_typing, key::lng_user_typing)
        }
        ChatAction::RecordingVideo => (
            key::lng_send_action_record_video,
            key::lng_user_action_record_video,
        ),
        ChatAction::UploadingVideo => (
            key::lng_send_action_upload_video,
            key::lng_user_action_upload_video,
        ),
        ChatAction::RecordingVoice => (
            key::lng_send_action_record_audio,
            key::lng_user_action_record_audio,
        ),
        ChatAction::UploadingVoice => (
            key::lng_send_action_upload_audio,
            key::lng_user_action_upload_audio,
        ),
        ChatAction::RecordingVideoNote => (
            key::lng_send_action_record_round,
            key::lng_user_action_record_round,
        ),
        ChatAction::UploadingVideoNote => (
            key::lng_send_action_upload_round,
            key::lng_user_action_upload_round,
        ),
        ChatAction::UploadingPhoto => (
            key::lng_send_action_upload_photo,
            key::lng_user_action_upload_photo,
        ),
        ChatAction::UploadingDocument => (
            key::lng_send_action_upload_file,
            key::lng_user_action_upload_file,
        ),
        ChatAction::ChoosingSticker => (
            key::lng_send_action_choose_sticker,
            key::lng_user_action_choose_sticker,
        ),
        ChatAction::PlayingGame => (key::lng_playing_game, key::lng_user_playing_game),
        ChatAction::WatchingAnimations(_) => (
            key::lng_user_action_watching_animations,
            key::lng_user_action_watching_animations,
        ),
    }
}
