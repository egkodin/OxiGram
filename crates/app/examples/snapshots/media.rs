//! Media in the history: photos in Saved Messages, one known only by its
//! minithumbnail and two downloaded; files in Maria's chat; videos and a GIF
//! in Saved Messages; albums there too; voice messages, contact cards and
//! places with Maria.

use std::io::Cursor;
use std::path::Path;

use features::settings::FoldersPlacement;
use gpui::MouseButton;
use model::Event;
use model::auth::State;
use model::checklist::{Checklist, Task};
use model::downloads::{self, Download, Totals};
use model::file::{Dimensions, File, Minithumbnail};
use model::location::Point;
use model::message::{
    Content, Expired, Media, Message, Music, SelfDestruct, Sender, Service, SharedContact,
    VoiceNote,
};
use model::poll::{Poll, PollKind, PollOption};
use model::sticker::{StickerKind, StickerPicture};
use model::text::FormattedText;
use model::{ChatId, FileId, MessageId, Secret, UserId};

use super::chats::{self, HOUR, ME, now};
use super::scenarios::{CHAT_AREA, MARIA_ROW, SAVED_ROW, auth, fixture_files};
use super::{Input, Scenario};

/// Maria's chat, where the documents are.
const MARIA: i64 = 2;
/// The downloaded file at the bottom of Maria's chat, for its menu.
const NOTES_ROW: (f32, f32) = (640.0, 520.0);
/// The spoiler photo in Saved Messages.
const SPOILER_PHOTO: (f32, f32) = (700.0, 250.0);
/// The download bar under the chat list.
const DOWNLOAD_BAR: (f32, f32) = (150.0, 577.0);
/// The newest row's date in the downloads list.
const FIRST_DOWNLOAD_DATE: (f32, f32) = (420.0, 118.0);
/// Maria's view-once photo, the last message in her chat.
const ONCE_PHOTO: (f32, f32) = (517.0, 400.0);
/// "Summit at sunrise", the middle of the three photos.
const CAPTIONED_PHOTO: (f32, f32) = (550.0, 150.0);
/// The play button of the downloaded video in Saved Messages.
const PLAYABLE_VIDEO: (f32, f32) = (550.0, 300.0);
/// The round's middle in Saved Messages, between the GIF and the sticker.
const PLAYABLE_ROUND: (f32, f32) = (664.0, 192.0);
/// A one-second 64x36 H.264 clip (`media`'s test fixture).
const CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../media/fixtures/clip.mp4");
/// The first song's play circle in Saved Messages.
const FIRST_SONG: (f32, f32) = (550.0, 445.0);
/// A WebM sticker's clip: VP9 with alpha, clear but for a red square.
const ALPHA_CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../media/fixtures/alpha.webm");
/// An 8 s square clip, long enough to still play when the harness draws.
const ROUND_CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../media/fixtures/round.mp4");

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
        // Albums: a portrait beside two landscapes with one caption, and
        // four without a caption or a bubble, one of them a video.
        Scenario {
            setup: chats::events(),
            clicks: vec![("161-albums-saved", MouseButton::Left, SAVED_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("162-albums-scrolled", CHAT_AREA, 300.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), albums(&photo))],
            steps: vec![("160-albums-list", auth(State::Ready))],
        },
        // Voices: one to download and unheard, one loading, one heard on
        // disk, and one sent with a caption nobody played yet.
        Scenario {
            setup: chats::events(),
            clicks: vec![("164-voices-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), voices(&notes))],
            steps: vec![("163-voices-list", auth(State::Ready))],
        },
        // Contacts: a contact's account, an account to add, a name and a
        // phone alone, and one sent.
        Scenario {
            setup: chats::events(),
            clicks: vec![("166-contacts-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), contacts())],
            steps: vec![("165-contacts-list", auth(State::Ready))],
        },
        // Places: a point, a venue with its address, and a live location.
        Scenario {
            setup: chats::events(),
            clicks: vec![("168-places-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("169-places-scrolled", CHAT_AREA, 400.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), places())],
            steps: vec![("167-places-list", auth(State::Ready))],
        },
        // Spoilers: a photo and a video under dots; a click reveals the photo.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("171-spoilers-saved", MouseButton::Left, SAVED_ROW),
                ("172-spoiler-revealed", MouseButton::Left, SPOILER_PHOTO),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), spoilers(&photo))],
            steps: vec![("170-spoilers-list", auth(State::Ready))],
        },
        // Stickers: a still one, and an animated one shown by its still.
        Scenario {
            setup: chats::events(),
            clicks: vec![("174-stickers-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), stickers())],
            steps: vec![("173-stickers-list", auth(State::Ready))],
        },
        // Round videos: one on disk nobody watched, one sent to download.
        Scenario {
            setup: chats::events(),
            clicks: vec![("176-rounds-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), rounds(&photo))],
            steps: vec![("175-rounds-list", auth(State::Ready))],
        },
        // Polls: one to vote in, one with several answers, a quiz answered
        // wrong with its explanation, and one sent and voted.
        Scenario {
            setup: chats::events(),
            clicks: vec![("178-polls-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("179-polls-scrolled", CHAT_AREA, 400.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), polls())],
            steps: vec![("177-polls-list", auth(State::Ready))],
        },
        // Big emoji: one, three, a die without its sticker, an animated
        // emoji with one, a text with words, and one sent.
        Scenario {
            setup: chats::events(),
            clicks: vec![("181-emoji-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), big_emoji())],
            steps: vec![("180-emoji-list", auth(State::Ready))],
        },
        // Checklists: one Maria lets others mark, one only she marks, and
        // one sent.
        Scenario {
            setup: chats::events(),
            clicks: vec![("183-checklists-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), checklists())],
            steps: vec![("182-checklists-list", auth(State::Ready))],
        },
        // The media viewer: a photo opened from Saved Messages, the next
        // one by the arrow key, then closed.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("185-viewer-chat", MouseButton::Left, SAVED_ROW),
                ("186-viewer-photo", MouseButton::Left, CAPTIONED_PHOTO),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![
                ("187-viewer-next", vec![Input::Key("right")]),
                ("188-viewer-closed", vec![Input::Key("escape")]),
            ],
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), photos(&photo))],
            steps: vec![("184-viewer-list", auth(State::Ready))],
        },
        // A downloaded video plays in the viewer under its controller;
        // Space pauses it.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("203-video-chat", MouseButton::Left, SAVED_ROW),
                ("204-video-viewer", MouseButton::Left, PLAYABLE_VIDEO),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![("205-video-paused", vec![Input::Key("space")])],
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), playable(&photo))],
            steps: vec![("202-video-list", auth(State::Ready))],
        },
        // A GIF and a WebM sticker on disk play by themselves, the sticker
        // clear around its square; a click on the round plays it in its
        // circle with the played part around the edge.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("207-inline-chat", MouseButton::Left, SAVED_ROW),
                ("208-round-playing", MouseButton::Left, PLAYABLE_ROUND),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), inline_videos(&photo))],
            steps: vec![("206-inline-list", auth(State::Ready))],
        },
        // Songs: "Performer – Title" or the file name, the length (and
        // size until downloaded); Play plays it, silently here, with the
        // time played.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("210-music-chat", MouseButton::Left, SAVED_ROW),
                ("211-music-playing", MouseButton::Left, FIRST_SONG),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), songs())],
            steps: vec![("209-music-list", auth(State::Ready))],
        },
        // View-once: Maria's photo under its fire with the "1", a timed
        // one sent, two that expired; a click opens hers alone.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("190-once-chat", MouseButton::Left, MARIA_ROW),
                ("191-once-viewer", MouseButton::Left, ONCE_PHOTO),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), view_once(&photo))],
            steps: vec![("189-once-list", auth(State::Ready))],
        },
        // The same chat scrolled up to the expired ones.
        Scenario {
            setup: chats::events(),
            clicks: vec![("193-expired-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("194-expired-scrolled", CHAT_AREA, 400.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), view_once(&photo))],
            steps: vec![("192-expired-list", auth(State::Ready))],
        },
        // Video timestamps: a quarter in with a caption, most of the way
        // in without one, the line's ends following the corners.
        Scenario {
            setup: chats::events(),
            clicks: vec![("196-timestamps-saved", MouseButton::Left, SAVED_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(ME), timestamped(&photo))],
            steps: vec![("195-timestamps-list", auth(State::Ready))],
        },
        // The download bar: one file loading, then a click opens its
        // message in Maria's chat.
        Scenario {
            setup: downloading(1, (12_582_912, 50_331_648)),
            clicks: vec![("198-download-opened", MouseButton::Left, DOWNLOAD_BAR)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), documents(&notes))],
            steps: vec![("197-download-bar", auth(State::Ready))],
        },
        // Three files done: "3 files", "View in downloads", the full circle;
        // a click opens the downloads list, a date there the message.
        Scenario {
            setup: [
                downloading(3, (75_000_000, 75_000_000)),
                vec![Event::Downloads(downloads::Change::Listed(listed(&notes)))],
            ]
            .concat(),
            clicks: vec![
                ("200-downloads-list", MouseButton::Left, DOWNLOAD_BAR),
                (
                    "201-download-message",
                    MouseButton::Left,
                    FIRST_DOWNLOAD_DATE,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: vec![(ChatId(MARIA), documents(&notes))],
            steps: vec![("199-downloads-done", auth(State::Ready))],
        },
    ]
}

fn rounds(still: &Path) -> Vec<Message> {
    let round = |id: i64, from: i64, ago: i32, duration: i32, file: File| {
        let media = Media {
            thumbnail: minithumbnail(),
            dimensions: Dimensions::new(384, 384),
            preview: Some(self::file(
                950 + i32::try_from(id).unwrap_or(0),
                Some(still),
            )),
            duration,
            file: Some(file),
            ..Media::default()
        };
        place(
            id,
            from,
            ago,
            Content::VideoNote {
                media: Box::new(media),
                is_viewed: false,
            },
        )
    };
    vec![
        round(1, MARIA, 2 * HOUR, 9, file(955, Some(still))),
        round(2, ME, HOUR, 21, file(956, None)),
    ]
}

/// A round face on a clear background, as a sticker's picture.
fn sticker_file() -> Option<std::path::PathBuf> {
    let path = std::env::temp_dir()
        .join("rgram-snapshots")
        .join("sticker.png");
    let image = image::RgbaImage::from_fn(512, 512, |x, y| {
        let distance = |cx: f32, cy: f32| {
            #[expect(clippy::cast_precision_loss, reason = "pixel coordinates")]
            let (x, y) = (x as f32, y as f32);
            ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()
        };
        if distance(186.0, 200.0) < 30.0 || distance(326.0, 200.0) < 30.0 {
            image::Rgba([60, 40, 20, 255])
        } else if distance(256.0, 256.0) < 240.0 {
            image::Rgba([250, 200, 60, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    });
    image
        .save(&path)
        .inspect_err(|error| eprintln!("writing the sticker fixture: {error:#}"))
        .ok()
        .map(|()| path)
}

fn big_emoji() -> Vec<Message> {
    let text =
        |id, from, ago, text: &str| place(id, from, ago, Content::text(FormattedText::plain(text)));
    let path = sticker_file();
    let picture = StickerPicture {
        kind: StickerKind::Animated,
        dimensions: Dimensions::new(512, 512),
        file: file(990, path.as_deref()),
        still: Some(file(991, path.as_deref())),
    };
    vec![
        text(1, MARIA, 5 * HOUR, "😂"),
        text(2, MARIA, 4 * HOUR, "👍🏽❤\u{fe0f}🔥"),
        place(
            3,
            MARIA,
            3 * HOUR,
            Content::Emoji {
                emoji: "🎲".to_owned(),
                sticker: None,
            },
        ),
        place(
            4,
            MARIA,
            2 * HOUR,
            Content::Emoji {
                emoji: "😀".to_owned(),
                sticker: Some(Box::new(picture)),
            },
        ),
        text(5, MARIA, 90 * 60, "ok 👍"),
        text(6, ME, HOUR, "🎉"),
    ]
}

fn checklists() -> Vec<Message> {
    let list = |id, from, ago, title: &str, tasks: &[(&str, Option<i64>)], can_mark: bool| {
        let checklist = Checklist {
            title: FormattedText::plain(title),
            tasks: tasks
                .iter()
                .zip(1..)
                .map(|(&(text, by), id)| Task {
                    id,
                    text: FormattedText::plain(text),
                    completed_by: by.map(|by| Sender::User(UserId(by))),
                })
                .collect(),
            others_can_mark_tasks_as_done: can_mark,
            can_mark_tasks_as_done: can_mark || from == ME,
        };
        place(id, from, ago, Content::Checklist(Box::new(checklist)))
    };
    vec![
        list(
            1,
            MARIA,
            3 * HOUR,
            "Trip to Lisbon",
            &[
                ("Book flights", Some(MARIA)),
                ("Find a hotel", None),
                ("Pack sunscreen", None),
            ],
            true,
        ),
        list(
            2,
            MARIA,
            2 * HOUR,
            "Her errands",
            &[("Post office", Some(MARIA)), ("Groceries", None)],
            false,
        ),
        list(
            3,
            ME,
            HOUR,
            "Weekend",
            &[("Clean the flat", Some(ME)), ("Call grandma", None)],
            true,
        ),
    ]
}

fn stickers() -> Vec<Message> {
    let path = sticker_file();
    let sticker = |id: i64, from: i64, ago: i32, kind: StickerKind, still: Option<File>| {
        let picture = StickerPicture {
            kind,
            dimensions: Dimensions::new(512, 512),
            file: file(i32::try_from(960 + id).unwrap_or(960), path.as_deref()),
            still,
        };
        place(
            id,
            from,
            ago,
            Content::Sticker {
                emoji: "😀".to_owned(),
                picture: Some(Box::new(picture)),
            },
        )
    };
    vec![
        sticker(1, MARIA, 2 * HOUR, StickerKind::Still, None),
        sticker(
            2,
            ME,
            HOUR,
            StickerKind::Animated,
            Some(file(969, path.as_deref())),
        ),
    ]
}

fn spoilers(downloaded: &Path) -> Vec<Message> {
    let hide = |message: Message| Message {
        content: match message.content {
            Content::Photo(media) => Content::Photo(Media {
                has_spoiler: true,
                ..media
            }),
            Content::Video(media) => Content::Video(Media {
                has_spoiler: true,
                ..media
            }),
            other => other,
        },
        ..message
    };
    vec![
        hide(photo(
            1,
            2 * HOUR,
            "",
            (1280, 853),
            file(970, Some(downloaded)),
        )),
        hide(video(
            2,
            HOUR,
            "Guess where",
            Some(12),
            [file(971, Some(downloaded)), file(972, None)],
        )),
    ]
}

fn place(id: i64, from: i64, ago: i32, content: Content) -> Message {
    Message {
        is_outgoing: from == ME,
        date: now() - ago,
        ..Message::new(
            MessageId(id),
            ChatId(MARIA),
            Sender::User(UserId(from)),
            content,
        )
    }
}

fn view_once(downloaded: &Path) -> Vec<Message> {
    let covered = |id: i64, from: i64, ago: i32, kind: SelfDestruct| {
        let Content::Photo(media) = photo(
            id,
            ago,
            "",
            (640, 427),
            file(960 + i32::try_from(id).unwrap_or(0), Some(downloaded)),
        )
        .content
        else {
            unreachable!("photo() makes a photo");
        };
        Message {
            self_destruct: Some(kind),
            can_be_saved: false,
            ..place(id, from, ago, Content::Photo(media))
        }
    };
    let expired = |id: i64, from: i64, ago: i32, kind: Expired| {
        place(id, from, ago, Content::Service(Service::Expired(kind)))
    };
    vec![
        expired(1, MARIA, 4 * HOUR, Expired::Photo),
        expired(2, ME, 3 * HOUR, Expired::VoiceNote),
        covered(3, ME, 2 * HOUR, SelfDestruct::Timer(10)),
        covered(4, MARIA, HOUR, SelfDestruct::Immediately),
    ]
}

fn places() -> Vec<Message> {
    let trailhead = Point::new(46.558_9, 7.983_4);
    vec![
        place(1, MARIA, 4 * HOUR, Content::Location(trailhead)),
        place(
            2,
            ME,
            3 * HOUR,
            Content::Venue {
                point: Point::new(46.624_1, 8.041_4),
                title: "Bergrestaurant First".to_owned(),
                address: "First 1, 3818 Grindelwald, Switzerland".to_owned(),
            },
        ),
        place(
            3,
            MARIA,
            2 * HOUR,
            Content::LiveLocation(Point::new(46.577_2, 7.991_2)),
        ),
    ]
}

fn poll(
    id: i64,
    from: i64,
    ago: i32,
    question: &str,
    options: &[(&str, i32, bool)],
    kind: PollKind,
) -> Message {
    let voted = options.iter().any(|option| option.2);
    let options = options
        .iter()
        .zip(0..)
        .map(|(&(text, percentage, is_chosen), index)| PollOption {
            index,
            text: FormattedText::plain(text),
            voters: if voted { percentage / 10 } else { 0 },
            percentage: if voted { percentage } else { 0 },
            is_chosen,
            is_being_chosen: false,
        })
        .collect();
    let poll = Poll {
        id,
        question: FormattedText::plain(question),
        options,
        total_voters: if voted { 10 } else { 4 },
        is_anonymous: id != 2,
        allows_multiple_answers: id == 2,
        allows_revoting: true,
        can_see_results: true,
        is_closed: id == 3,
        close_date: 0,
        kind,
    };
    Message {
        is_outgoing: from == ME,
        date: now() - ago,
        ..Message::new(
            MessageId(id),
            ChatId(MARIA),
            Sender::User(UserId(from)),
            Content::Poll(Box::new(poll)),
        )
    }
}

fn polls() -> Vec<Message> {
    let quiz = PollKind::Quiz {
        correct: vec![1],
        explanation: FormattedText::plain(
            "Canberra was built as a compromise between Sydney and Melbourne.",
        ),
    };
    let lunch = [
        ("Pizza", 0, false),
        ("Sushi", 0, false),
        ("Tacos", 0, false),
    ];
    let days = [
        ("Monday", 0, false),
        ("Wednesday", 0, false),
        ("Friday", 0, false),
    ];
    let capital = [
        ("Sydney", 40, true),
        ("Canberra", 50, false),
        ("Melbourne", 10, false),
    ];
    let movie = [("Yes", 70, true), ("No", 30, false)];
    vec![
        poll(
            1,
            MARIA,
            4 * HOUR,
            "Where should we go for lunch?",
            &lunch,
            PollKind::Regular,
        ),
        poll(
            2,
            MARIA,
            3 * HOUR,
            "Which days work for you?",
            &days,
            PollKind::Regular,
        ),
        poll(
            3,
            MARIA,
            2 * HOUR,
            "What is the capital of Australia?",
            &capital,
            quiz,
        ),
        poll(4, ME, HOUR, "Movie tonight?", &movie, PollKind::Regular),
    ]
}

fn contact(id: i64, from: i64, ago: i32, name: (&str, &str), phone: &str, user: i64) -> Message {
    let card = SharedContact {
        first_name: name.0.to_owned(),
        last_name: name.1.to_owned(),
        phone: Secret::new(phone),
        user: Some(UserId(user)).filter(|user| user.0 != 0),
        has_vcard: false,
    };
    Message {
        is_outgoing: from == ME,
        date: now() - ago,
        ..Message::new(
            MessageId(id),
            ChatId(MARIA),
            Sender::User(UserId(from)),
            Content::Contact(Box::new(card)),
        )
    }
}

fn contacts() -> Vec<Message> {
    vec![
        contact(1, MARIA, 4 * HOUR, ("Ann", "Lee"), "447700900123", 3),
        contact(2, MARIA, 3 * HOUR, ("Dmitry", "Petrov"), "79991234567", 6),
        contact(3, MARIA, 2 * HOUR, ("Plumber", ""), "15551230000", 0),
        contact(4, ME, HOUR, ("Ben", ""), "4915112345678", 4),
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
        has_spoiler: false,
        start_timestamp: 0,
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
        has_spoiler: false,
        start_timestamp: 0,
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
        has_spoiler: false,
        start_timestamp: 0,
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

/// One downloaded video in Saved Messages, the clip behind it.
fn playable(still: &Path) -> Vec<Message> {
    let message = video(
        1,
        HOUR,
        "Sunrise from the ridge",
        Some(1),
        [file(1201, Some(still)), file(1202, Some(Path::new(CLIP)))],
    );
    vec![message]
}

/// A tagged song on disk and an untagged one on the server.
fn songs() -> Vec<Message> {
    let song = |id: i64, name: &str, (title, performer): (&str, &str), duration, music| {
        saved(
            id,
            3 * HOUR - i32::try_from(id).unwrap_or(0) * HOUR,
            Content::Audio {
                name: name.to_owned(),
                caption: FormattedText::default(),
                music: Box::new(Music {
                    title: title.to_owned(),
                    performer: performer.to_owned(),
                    duration,
                    file: music,
                }),
            },
        )
    };
    let remote = File {
        size: 8_600_000,
        ..file(1222, None)
    };
    vec![
        // The round clip's 8 s sound track: still playing when the harness draws.
        song(
            1,
            "morning-tone.m4a",
            ("Morning Tone", "The Fixtures"),
            8,
            file(1221, Some(Path::new(ROUND_CLIP))),
        ),
        song(2, "field-recording.mp3", ("", ""), 225, remote),
    ]
}

/// A GIF and a round video in Saved Messages, both on disk.
fn inline_videos(still: &Path) -> Vec<Message> {
    let clip = Some(Path::new(ROUND_CLIP));
    let gif = video(
        1,
        2 * HOUR,
        "",
        None,
        [file(1211, Some(still)), file(1212, clip)],
    );
    let media = Media {
        thumbnail: minithumbnail(),
        dimensions: Dimensions::new(384, 384),
        preview: Some(file(1213, Some(still))),
        duration: 8,
        file: Some(file(1214, clip)),
        ..Media::default()
    };
    let round = saved(
        2,
        HOUR,
        Content::VideoNote {
            media: Box::new(media),
            is_viewed: true,
        },
    );
    let picture = StickerPicture {
        kind: StickerKind::Video,
        dimensions: Dimensions::new(512, 512),
        file: file(1215, Some(Path::new(ALPHA_CLIP))),
        still: None,
    };
    let sticker = saved(
        3,
        HOUR / 2,
        Content::Sticker {
            emoji: "🟥".to_owned(),
            picture: Some(Box::new(picture)),
        },
    );
    vec![gif, round, sticker]
}

fn albums(downloaded: &Path) -> Vec<Message> {
    let part = |id: i64, ago: i32, album: i64, caption: &str, dimensions: (i32, i32)| {
        let file = file(i32::try_from(990 + id).unwrap_or(990), Some(downloaded));
        Message {
            album: Some(album),
            ..photo(id, ago, caption, dimensions, file)
        }
    };
    let clip = Message {
        album: Some(2),
        ..video(
            8,
            HOUR,
            "",
            Some(42),
            [file(998, Some(downloaded)), file(999, None)],
        )
    };
    vec![
        part(1, 3 * HOUR, 1, "", (720, 1280)),
        part(2, 3 * HOUR, 1, "Three views from the ridge", (1280, 853)),
        part(3, 3 * HOUR, 1, "", (1280, 853)),
        part(5, HOUR, 2, "", (1280, 720)),
        part(6, HOUR, 2, "", (1000, 1000)),
        part(7, HOUR, 2, "", (1000, 1000)),
        clip,
    ]
}

/// A voice's 100 bars: a few words and pauses.
fn speech(seed: u32) -> Vec<u8> {
    (0..100_u32)
        .map(|index| {
            let word = (index + seed) % 23 < 17;
            let wobble = (index * 7 + seed * 3) % 11;
            u8::try_from(if word { 10 + wobble * 2 } else { wobble / 3 }).unwrap_or(0)
        })
        .collect()
}

fn voice(id: i64, from: i64, ago: i32, caption: &str, note: VoiceNote) -> Message {
    Message {
        is_outgoing: from == ME,
        date: now() - ago,
        ..Message::new(
            MessageId(id),
            ChatId(MARIA),
            Sender::User(UserId(from)),
            Content::Voice {
                caption: FormattedText::plain(caption),
                note: Box::new(note),
            },
        )
    }
}

fn voices(on_disk: &Path) -> Vec<Message> {
    let note = |duration: i32, seed: u32, file: File, is_listened: bool| VoiceNote {
        duration,
        waveform: speech(seed),
        file,
        is_listened,
    };
    let loading = File {
        size: 60_000,
        downloaded: 21_000,
        is_downloading: true,
        ..file(976, None)
    };
    vec![
        voice(1, MARIA, 4 * HOUR, "", note(7, 1, file(975, None), false)),
        voice(2, MARIA, 3 * HOUR, "", note(14, 5, loading, false)),
        voice(
            3,
            MARIA,
            2 * HOUR,
            "",
            note(42, 9, file(977, Some(on_disk)), true),
        ),
        voice(
            4,
            ME,
            HOUR,
            "On my way",
            note(3, 13, file(978, Some(on_disk)), false),
        ),
    ]
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

/// The chat list's events with `count` files in TDLib's download list,
/// the last one Maria's archive.
fn downloading(count: i32, (downloaded_size, total_size): (i64, i64)) -> Vec<Event> {
    let mut events = chats::events();
    events.push(Event::Downloads(downloads::Change::Added(Download {
        file: FileId(971),
        chat: ChatId(MARIA),
        message: MessageId(2),
        name: "saturday-photos.zip".to_owned(),
    })));
    events.push(Event::Downloads(downloads::Change::Totals(Totals {
        total_size,
        total_count: count,
        downloaded_size,
    })));
    events
}

/// Maria's three files in the downloads list, newest first.
fn listed(notes: &Path) -> Vec<downloads::Entry> {
    documents(notes)
        .into_iter()
        .rev()
        .filter_map(|message| {
            let Content::Document { media, .. } = &message.content else {
                return None;
            };
            Some(downloads::Entry {
                file: media.file.as_ref()?.id,
                added: message.date,
                message,
            })
        })
        .collect()
}

fn timestamped(still: &Path) -> Vec<Message> {
    let at = |message: Message, start| Message {
        content: match message.content {
            Content::Video(media) => Content::Video(Media {
                start_timestamp: start,
                ..media
            }),
            other => other,
        },
        ..message
    };
    let both = |id| [file(id, Some(still)), file(id + 1, Some(still))];
    vec![
        at(
            video(
                1,
                2 * HOUR,
                "From the top of the pass",
                Some(120),
                both(990),
            ),
            30,
        ),
        at(video(2, HOUR, "", Some(120), both(992)), 100),
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
