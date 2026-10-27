//! Sending files (`ApiWrap::sendFiles`): photos and videos, alone or in
//! albums of up to ten, and files, grouped the way tdesktop groups them
//! (`Ui::DivideByGroups`). TDLib uploads them and reports the progress
//! through `updateFile`.

use std::future::Future;
use std::ops::Range;
use std::path::PathBuf;

use base64::Engine as _;
use td_api::enums::{InputFile, InputMessageContent};
use td_api::functions::{SendMessage, SendMessageAlbum};
use td_api::types::{
    FormattedText, InputDocument, InputFileLocal, InputMessageDocument, InputMessagePhoto,
    InputMessageVideo, InputMessageVoiceNote, InputPhoto, InputVideo, InputVoiceNote,
};

use crate::ids::{ChatId, MessageId};
use crate::message::Topic;
use crate::send::{SendOptions, reply};
use crate::session::{RequestError, Session};
use crate::thread::Key;

/// An album holds at most this many (`kMaxAlbumCount`).
pub const MAX_ALBUM_COUNT: usize = 10;

/// How a file goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    /// Compressed by the server, shown as a photo.
    Photo {
        width: u32,
        height: u32,
    },
    Video,
    /// As it is, whatever it holds.
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub path: PathBuf,
    pub kind: AttachmentKind,
}

/// Whether consecutive files of one kind go as one album.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    Albums,
    Separate,
}

/// What an album holds (`Ui::AlbumType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Album {
    PhotoVideo,
    Files,
}

fn album_of(kind: AttachmentKind, grouping: Grouping) -> Option<Album> {
    match (grouping, kind) {
        (Grouping::Separate, _) => None,
        (Grouping::Albums, AttachmentKind::Photo { .. } | AttachmentKind::Video) => {
            Some(Album::PhotoVideo)
        }
        (Grouping::Albums, AttachmentKind::File) => Some(Album::Files),
    }
}

/// Runs of attachments, each with its album if it has more than one; a
/// run without one goes file by file (`GroupRanges`).
fn groups(attachments: &[Attachment], grouping: Grouping) -> Vec<(Range<usize>, Option<Album>)> {
    let mut result = Vec::new();
    let mut from = 0;
    let mut album = None;
    for (index, attachment) in attachments.iter().enumerate() {
        let next = album_of(attachment.kind, grouping);
        let count = index - from;
        if (count > 0 && album != next) || (album.is_some() && count == MAX_ALBUM_COUNT) {
            result.push((from..index, album.filter(|_| count > 1)));
            from = index;
        }
        album = next;
    }
    let count = attachments.len() - from;
    if count > 0 {
        result.push((from..attachments.len(), album.filter(|_| count > 1)));
    }
    result
}

/// Where the caption goes: on the last group, its first item in a photo
/// and video album, else its last (`SendFilesBox::send`).
fn captioned(groups: &[(Range<usize>, Option<Album>)]) -> Option<usize> {
    let (range, album) = groups.last()?;
    Some(if *album == Some(Album::PhotoVideo) {
        range.start
    } else {
        range.end - 1
    })
}

fn content(attachment: &Attachment, caption: Option<String>) -> InputMessageContent {
    let file = Box::new(InputFile::Local(InputFileLocal {
        path: attachment.path.to_string_lossy().into_owned(),
    }));
    let caption = caption.map(|text| {
        Box::new(FormattedText {
            text,
            entities: Vec::new(),
        })
    });
    let side = |side: u32| i32::try_from(side).unwrap_or(0);
    match attachment.kind {
        AttachmentKind::Photo { width, height } => {
            InputMessageContent::InputMessagePhoto(InputMessagePhoto {
                photo: Box::new(InputPhoto {
                    photo: file,
                    thumbnail: None,
                    video: None,
                    added_sticker_file_ids: Vec::new(),
                    width: side(width),
                    height: side(height),
                }),
                caption,
                show_caption_above_media: false,
                self_destruct_type: None,
                has_spoiler: false,
            })
        }
        // shortcut: no duration, size or thumbnail; read them with the video decoder in M5.
        AttachmentKind::Video => InputMessageContent::InputMessageVideo(InputMessageVideo {
            video: Box::new(InputVideo {
                video: file,
                thumbnail: None,
                cover: None,
                start_timestamp: 0,
                added_sticker_file_ids: Vec::new(),
                duration: 0,
                width: 0,
                height: 0,
                supports_streaming: false,
            }),
            caption,
            show_caption_above_media: false,
            self_destruct_type: None,
            has_spoiler: false,
        }),
        // shortcut: music and GIFs go as files too; as audio and animations with the media work in M5.
        AttachmentKind::File => InputMessageContent::InputMessageDocument(InputMessageDocument {
            document: Box::new(InputDocument {
                document: file,
                thumbnail: None,
                disable_content_type_detection: true,
            }),
            caption,
        }),
    }
}

impl Session {
    /// Sends `attachments` to the chat or topic `to`, replying to
    /// `reply_to`, with `caption` where tdesktop puts it; one group at a
    /// time, so they arrive in order.
    pub fn send_files(
        &self,
        to: Key,
        reply_to: Option<MessageId>,
        attachments: &[Attachment],
        caption: &str,
        grouping: Grouping,
        options: SendOptions,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let groups = groups(attachments, grouping);
        let caption_at = captioned(&groups).filter(|_| !caption.trim().is_empty());
        let caption = caption.trim().to_owned();
        let reply = || reply(reply_to);
        let Key { chat, topic } = to;
        let topic = || topic.map(|topic| Box::new(topic.to_td()));
        let mut requests = Vec::new();
        for (range, album) in groups {
            let mut contents = range.map(|index| {
                let caption = (caption_at == Some(index)).then(|| caption.clone());
                content(&attachments[index], caption)
            });
            if album.is_some() {
                let request = self.client().request(&SendMessageAlbum {
                    chat_id: chat.0,
                    topic_id: topic(),
                    reply_to: reply(),
                    options: options.to_td(),
                    input_message_contents: contents.collect(),
                });
                requests.push(futures::future::Either::Left(async move {
                    request.await.map(drop)
                }));
                continue;
            }
            // Outside an album each file is a message of its own.
            for content in contents.by_ref() {
                let request = self.client().request(&SendMessage {
                    chat_id: chat.0,
                    topic_id: topic(),
                    reply_to: reply(),
                    options: options.to_td(),
                    reply_markup: None,
                    input_message_content: Box::new(content),
                });
                requests.push(futures::future::Either::Right(async move {
                    request.await.map(drop)
                }));
            }
        }
        async move {
            for request in requests {
                request.await.map_err(RequestError::from)?;
            }
            Ok(())
        }
    }
}

/// A recorded voice message (`inputMessageVoiceNote`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceNote {
    /// OGG Opus, one channel.
    pub path: PathBuf,
    pub duration: std::time::Duration,
    /// 5 bits per value.
    pub waveform: Vec<u8>,
}

impl Session {
    /// Sends `voice` to `chat` or its topic, replying to `reply_to`.
    pub fn send_voice(
        &self,
        chat: ChatId,
        topic: Option<Topic>,
        reply_to: Option<MessageId>,
        voice: &VoiceNote,
        options: SendOptions,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        let content = InputMessageContent::InputMessageVoiceNote(InputMessageVoiceNote {
            voice_note: Box::new(InputVoiceNote {
                voice_note: Box::new(InputFile::Local(InputFileLocal {
                    path: voice.path.to_string_lossy().into_owned(),
                })),
                duration: i32::try_from(voice.duration.as_secs()).unwrap_or(i32::MAX),
                // TDLib's JSON carries bytes in base64.
                waveform: base64::engine::general_purpose::STANDARD.encode(&voice.waveform),
            }),
            caption: None,
            self_destruct_type: None,
        });
        let request = self.client().request(&SendMessage {
            chat_id: chat.0,
            topic_id: topic.map(|topic| Box::new(topic.to_td())),
            reply_to: reply(reply_to),
            options: options.to_td(),
            reply_markup: None,
            input_message_content: Box::new(content),
        });
        async move { request.await.map(drop).map_err(RequestError::from) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(kind: AttachmentKind) -> Attachment {
        Attachment {
            path: PathBuf::from("/tmp/x"),
            kind,
        }
    }

    const PHOTO: AttachmentKind = AttachmentKind::Photo {
        width: 10,
        height: 10,
    };

    #[test]
    fn albums_split_by_kind_and_at_ten() {
        let mut list: Vec<_> = (0..12).map(|_| attachment(PHOTO)).collect();
        list.push(attachment(AttachmentKind::Video));
        list.push(attachment(AttachmentKind::File));
        list.push(attachment(AttachmentKind::File));
        assert_eq!(
            groups(&list, Grouping::Albums),
            [
                (0..10, Some(Album::PhotoVideo)),
                (10..13, Some(Album::PhotoVideo)),
                (13..15, Some(Album::Files)),
            ]
        );
    }

    #[test]
    fn a_lone_item_or_no_grouping_goes_alone() {
        let list = [attachment(PHOTO), attachment(AttachmentKind::File)];
        assert_eq!(
            groups(&list, Grouping::Albums),
            [(0..1, None), (1..2, None)]
        );
        assert_eq!(groups(&list[..1], Grouping::Separate), [(0..1, None)]);
        let two = [attachment(PHOTO), attachment(PHOTO)];
        assert_eq!(groups(&two, Grouping::Separate), [(0..2, None)]);
    }

    #[test]
    fn the_caption_goes_first_in_an_album_and_last_among_files() {
        assert_eq!(captioned(&[(0..3, Some(Album::PhotoVideo))]), Some(0));
        assert_eq!(
            captioned(&[(0..1, None), (1..4, Some(Album::Files))]),
            Some(3)
        );
        assert_eq!(captioned(&[]), None);
    }
}
