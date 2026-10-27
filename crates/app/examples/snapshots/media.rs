//! Media in the history: photos in Saved Messages, one known only by its
//! minithumbnail and two downloaded; files in Maria's chat; videos and a GIF
//! in Saved Messages.

use std::io::Cursor;
use std::path::Path;

use features::settings::FoldersPlacement;
use gpui::MouseButton;
use model::auth::State;
use model::file::{Dimensions, File, Minithumbnail};
use model::message::{Content, Media, Message, Sender};
use model::text::FormattedText;
use model::{ChatId, FileId, MessageId, UserId};

use super::Scenario;
use super::chats::{self, HOUR, ME, now};
use super::scenarios::{CHAT_AREA, MARIA_ROW, SAVED_ROW, auth, fixture_files};

/// Maria's chat, where the documents are.
const MARIA: i64 = 2;
/// The downloaded file at the bottom of Maria's chat, for its menu.
const NOTES_ROW: (f32, f32) = (640.0, 520.0);

pub(crate) fn scenarios() -> Vec<Scenario> {
    let (photo, notes) = fixture_files();
    vec![
        Scenario {
            setup: chats::events(),
            clicks: vec![("152-photos-saved", MouseButton::Left, SAVED_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("153-photos-scrolled", CHAT_AREA, 400.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), photos(&photo))],
            steps: vec![("151-photos-list", auth(State::Ready))],
        },
        // Files: one to download, one loading with a caption, one on disk
        // with "Show in Finder" in its menu.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("155-files-chat", MouseButton::Left, MARIA_ROW),
                ("156-file-menu", MouseButton::Right, NOTES_ROW),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), documents(&notes))],
            steps: vec![("154-files-list", auth(State::Ready))],
        },
        // Videos: one to download, one loading with a caption, a GIF on disk.
        Scenario {
            setup: chats::events(),
            clicks: vec![("158-videos-saved", MouseButton::Left, SAVED_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("159-videos-scrolled", CHAT_AREA, 400.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), videos(&photo))],
            steps: vec![("157-videos-list", auth(State::Ready))],
        },
    ]
}

fn document(id: i64, from: i64, ago: i32, name: &str, caption: &str, file: File) -> Message {
    let media = Media {
        caption: FormattedText::plain(caption),
        thumbnail: None,
        dimensions: None,
        preview: None,
        duration: 0,
        file: Some(file),
    };
    Message {
        is_outgoing: from == ME,
        date: now() - ago,
        ..Message::new(
            MessageId(id),
            ChatId(MARIA),
            Sender::User(UserId(from)),
            Content::Document {
                name: name.to_owned(),
                media,
            },
        )
    }
}

fn documents(notes: &Path) -> Vec<Message> {
    let remote = File {
        size: 2_516_582,
        ..file(970, None)
    };
    let loading = File {
        size: 50_331_648,
        downloaded: 12_582_912,
        is_downloading: true,
        ..file(971, None)
    };
    let local = File {
        size: 28,
        ..file(972, Some(notes))
    };
    vec![
        document(1, MARIA, 3 * HOUR, "route-map.pdf", "", remote),
        document(
            2,
            MARIA,
            2 * HOUR,
            "saturday-photos.zip",
            "All of Saturday's photos",
            loading,
        ),
        document(3, ME, HOUR, "trail-notes.txt", "", local),
    ]
}

/// A 40x23 JPEG, as small as TDLib's minithumbnails.
fn minithumbnail() -> Option<Minithumbnail> {
    let image = image::RgbImage::from_fn(40, 23, |x, y| {
        let [x, y] = [x, y].map(|value| u8::try_from(value * 6).unwrap_or(u8::MAX));
        image::Rgb([200 - x / 2, 120 + y / 2, 90])
    });
    let mut jpeg = Cursor::new(Vec::new());
    if let Err(error) = image.write_to(&mut jpeg, image::ImageFormat::Jpeg) {
        eprintln!("encoding the minithumbnail: {error}");
        return None;
    }
    Some(Minithumbnail {
        width: 40,
        height: 23,
        jpeg: jpeg.into_inner(),
    })
}

fn saved(id: i64, ago: i32, content: Content) -> Message {
    Message {
        is_outgoing: true,
        date: now() - ago,
        ..Message::new(MessageId(id), ChatId(ME), Sender::User(UserId(ME)), content)
    }
}

fn photo(id: i64, ago: i32, caption: &str, dimensions: (i32, i32), file: File) -> Message {
    let media = Media {
        caption: FormattedText::plain(caption),
        thumbnail: minithumbnail(),
        dimensions: Dimensions::new(dimensions.0, dimensions.1),
        preview: None,
        duration: 0,
        file: Some(file),
    };
    saved(id, ago, Content::Photo(media))
}

/// A video, or a GIF when `duration` is `None`, with its still.
fn video(
    id: i64,
    ago: i32,
    caption: &str,
    duration: Option<i32>,
    [still, file]: [File; 2],
) -> Message {
    let media = Media {
        caption: FormattedText::plain(caption),
        thumbnail: minithumbnail(),
        dimensions: Dimensions::new(1280, 720),
        preview: Some(still),
        duration: duration.unwrap_or(0),
        file: Some(file),
    };
    saved(
        id,
        ago,
        match duration {
            Some(_) => Content::Video(media),
            None => Content::Animation(media),
        },
    )
}

fn videos(still: &Path) -> Vec<Message> {
    let remote = File {
        size: 8_808_038,
        ..file(981, None)
    };
    let loading = File {
        size: 12_582_912,
        downloaded: 3_145_728,
        is_downloading: true,
        ..file(983, None)
    };
    vec![
        video(1, 3 * HOUR, "", Some(75), [file(980, None), remote]),
        video(
            2,
            2 * HOUR,
            "Ridge line",
            Some(754),
            [file(982, Some(still)), loading],
        ),
        video(
            3,
            HOUR,
            "",
            None,
            [file(984, Some(still)), file(985, Some(still))],
        ),
    ]
}

fn file(id: i32, path: Option<&Path>) -> File {
    File {
        id: FileId(id),
        path: path.map(|path| path.to_string_lossy().into_owned()),
        is_downloading: false,
        downloaded: 0,
        size: 120_000,
        uploaded: 120_000,
        is_uploading: false,
    }
}

fn photos(downloaded: &Path) -> Vec<Message> {
    vec![
        photo(1, 3 * HOUR, "", (1080, 1920), file(950, None)),
        photo(
            2,
            2 * HOUR,
            "Summit at sunrise",
            (640, 427),
            file(951, Some(downloaded)),
        ),
        photo(3, HOUR, "", (640, 427), file(952, Some(downloaded))),
    ]
}
