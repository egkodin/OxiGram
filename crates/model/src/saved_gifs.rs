//! Saved animations and reuse of their TDLib file IDs when sending.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::{InputFile, InputMessageContent};
use td_api::functions::{GetSavedAnimations, SendMessage};
use td_api::types as td;

use crate::file::{File, Minithumbnail};
use crate::send::reply;
use crate::thread::Key;
use crate::{MessageId, RequestError, Session};

#[derive(Debug, Clone)]
pub struct SavedGif {
    pub file: File,
    pub width: i32,
    pub height: i32,
    pub duration: i32,
    pub preview: Option<Minithumbnail>,
    pub thumbnail: Option<File>,
}

impl From<td::Animation> for SavedGif {
    fn from(animation: td::Animation) -> Self {
        Self {
            file: File::from(*animation.animation),
            width: animation.width,
            height: animation.height,
            duration: animation.duration,
            preview: animation
                .minithumbnail
                .and_then(|preview| Minithumbnail::from_td(*preview)),
            thumbnail: crate::message::still(animation.thumbnail),
        }
    }
}

impl Session {
    pub fn saved_gifs(
        &self,
    ) -> impl Future<Output = Result<Vec<SavedGif>, RequestError>> + Send + use<> {
        self.client().request(&GetSavedAnimations {}).map(|result| {
            result
                .map(|saved| saved.animations.into_iter().map(SavedGif::from).collect())
                .map_err(RequestError::from)
        })
    }

    pub fn send_saved_gif(
        &self,
        key: Key,
        reply_to: Option<MessageId>,
        gif: &SavedGif,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&send_request(key, reply_to, gif))
            .map(|result| result.map(drop).map_err(RequestError::from))
    }
}

fn send_request(key: Key, reply_to: Option<MessageId>, gif: &SavedGif) -> SendMessage {
    SendMessage {
        chat_id: key.chat.0,
        topic_id: key.topic.map(|topic| Box::new(topic.to_td())),
        reply_to: reply(reply_to),
        options: None,
        reply_markup: None,
        input_message_content: Box::new(InputMessageContent::InputMessageAnimation(
            td::InputMessageAnimation {
                animation: Box::new(td::InputAnimation {
                    animation: Box::new(InputFile::Id(td::InputFileId { id: gif.file.id.0 })),
                    thumbnail: None,
                    added_sticker_file_ids: vec![],
                    duration: gif.duration,
                    width: gif.width,
                    height: gif.height,
                }),
                caption: None,
                show_caption_above_media: false,
                has_spoiler: false,
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Topic;
    use crate::{ChatId, FileId};

    #[test]
    fn a_saved_gif_sends_by_file_id_in_the_current_topic() {
        let gif = SavedGif {
            file: File {
                id: FileId(17),
                path: None,
                is_downloading: false,
                downloaded: 0,
                size: 0,
                uploaded: 0,
                is_uploading: false,
                prefix: Default::default(),
            },
            width: 480,
            height: 270,
            duration: 3,
            preview: None,
            thumbnail: None,
        };
        let key = Key {
            chat: ChatId(42),
            topic: Some(Topic::Thread(MessageId(5))),
        };
        let request = send_request(key, Some(MessageId(6)), &gif);
        assert_eq!(request.chat_id, 42);
        assert_eq!(
            request.topic_id,
            key.topic.map(|topic| Box::new(topic.to_td()))
        );
        assert_eq!(request.reply_to, reply(Some(MessageId(6))));
        let InputMessageContent::InputMessageAnimation(content) = *request.input_message_content
        else {
            panic!("saved GIFs must send as animations");
        };
        let animation = content.animation;
        assert_eq!(
            *animation.animation,
            InputFile::Id(td::InputFileId { id: 17 })
        );
        assert_eq!(
            (animation.width, animation.height, animation.duration),
            (480, 270, 3)
        );
    }
}
