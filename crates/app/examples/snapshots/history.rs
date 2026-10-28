//! Preset messages for the history snapshots: a group conversation over
//! two days and a private chat, both ending in their chat's last message.

use model::Event;
use model::file::File;
use model::history::Change;
use model::message::{
    Content, External, Forward, LinkPreview, Media, Message, Origin, Replies, ReplyTo, Sender,
    Sending, Service,
};
use model::reaction::{Reaction, ReactionKind, Reactions};
use model::text::{DateFormat, Entity, EntityKind, FormattedText, Precision};
use model::{ChatId, FileId, MessageId, UserId};

use crate::chats::{GIF_BOT, HOUR, ME, now};

const MINUTE: i32 = 60;

fn message(chat: i64, id: i64, from: i64, ago: i32, text: &str) -> Message {
    Message {
        is_outgoing: from == ME,
        date: now() - ago,
        ..Message::new(
            MessageId(id),
            ChatId(chat),
            Sender::User(UserId(from)),
            Content::text(FormattedText::plain(text)),
        )
    }
}

/// Marks `part` of the message's text with `kind`.
fn entity(mut message: Message, part: &str, kind: EntityKind) -> Message {
    if let Content::Text { text, .. } = &mut message.content {
        let start = text
            .text
            .find(part)
            .expect("the part is in the fixture text");
        text.entities.push(Entity {
            range: start..start + part.len(),
            kind,
        });
    }
    message
}

fn reply(message: Message, to: i64) -> Message {
    Message {
        reply_to: Some(ReplyTo {
            chat: message.chat,
            message: MessageId(to),
            quote: None,
            external: None,
        }),
        ..message
    }
}

/// Hiking Club: runs by one sender group, a new day gets a date badge.
fn group() -> Vec<Message> {
    let day = 24 * HOUR;
    let joined = Message {
        date: now() - day - 4 * HOUR,
        ..Message::new(
            MessageId(30),
            ChatId(-100),
            Sender::User(UserId(4)),
            Content::Service(Service::MembersAdded(vec![UserId(4)])),
        )
    };
    let forwarded = Message {
        forward: Some(Forward {
            origin: Origin::HiddenUser("Trail News".to_owned()),
            date: now() - 2 * day,
        }),
        ..message(
            -100,
            39,
            2,
            5 * HOUR + 10 * MINUTE,
            "North loop is open again, see https://example.org/trails",
        )
    };
    vec![
        joined,
        message(
            -100,
            31,
            2,
            day + 3 * HOUR,
            "Who's in for Saturday? The weather looks great.",
        ),
        message(
            -100,
            32,
            2,
            day + 3 * HOUR - MINUTE,
            "Meeting at the north trailhead at 7.",
        ),
        message(-100, 33, ME, day + 2 * HOUR, "Count me in!"),
        entity(
            entity(
                message(
                    -100,
                    34,
                    4,
                    day + 2 * HOUR - 5 * MINUTE,
                    "Me too, I'll bring the map and some snacks for everyone. Does anyone need a ride from the station?",
                ),
                "the map",
                EntityKind::Bold,
            ),
            "Does anyone need a ride",
            EntityKind::Italic,
        ),
        message(
            -100,
            35,
            3,
            6 * HOUR,
            "That was a great hike, thanks everyone",
        ),
        message(-100, 36, 3, 6 * HOUR - MINUTE, "Uploading the photos now"),
        reply(
            message(
                -100,
                37,
                ME,
                5 * HOUR + 30 * MINUTE,
                "Can't wait to see them",
            ),
            36,
        ),
        message(
            -100,
            38,
            ME,
            5 * HOUR + 29 * MINUTE,
            "Same time next month?",
        ),
        entity(forwarded, "https://example.org/trails", EntityKind::Url),
        Message {
            via_bot: Some(UserId(GIF_BOT)),
            ..message(-100, 40, 3, 5 * HOUR, "Photos from Saturday are up")
        },
    ]
}

/// Maria Garcia: read and unread outgoing messages and replies.
fn private() -> Vec<Message> {
    vec![
        message(2, 15, 2, 3 * HOUR, "Hi! Did you get the tickets?"),
        message(2, 16, ME, 2 * HOUR, "Yes, two seats in row 5"),
        message(2, 17, ME, 2 * HOUR - MINUTE, "The show starts at 8"),
        reply(message(2, 18, 2, 90 * MINUTE, "Perfect"), 17),
        entity(
            entity(
                message(2, 19, ME, 70 * MINUTE, "See you at gate B, seats 12 and 13"),
                "gate B",
                EntityKind::Code,
            ),
            "12 and 13",
            EntityKind::Spoiler,
        ),
        entity(
            message(
                2,
                20,
                2,
                MINUTE,
                "Doors open at 7:30\nAre we still on for tonight?",
            ),
            "Doors open at 7:30",
            EntityKind::BlockQuote(model::text::Quote::Plain),
        ),
    ]
}

/// Gives a text message a link preview.
fn with_preview(mut message: Message, preview: LinkPreview) -> Message {
    if let Content::Text { preview: slot, .. } = &mut message.content {
        *slot = Some(Box::new(preview));
    }
    message
}

/// A reply to something in another chat or to a story, as TDLib tells it.
fn external(message: Message, chat: i64, to: i64, external: External) -> Message {
    Message {
        reply_to: Some(ReplyTo {
            chat: ChatId(chat),
            message: MessageId(to),
            quote: None,
            external: Some(Box::new(external)),
        }),
        ..message
    }
}

/// Ann Lee: a reply to her story, link previews below and above the text,
/// a message TDLib cannot show and a reply to a channel post.
fn ann() -> Vec<Message> {
    let story = external(
        message(3, 25, ME, 4 * HOUR + 5 * MINUTE, "Great view!"),
        3,
        0,
        External::Story { poster: ChatId(3) },
    );
    let channel_post = external(
        message(3, 29, 3, 3 * HOUR, "Like this one from Daily Landscapes"),
        -200,
        50,
        External::Message {
            origin: Origin::Channel {
                chat: ChatId(-200),
                message: MessageId(50),
                signature: String::new(),
            },
            media: Some(Box::new(Content::Photo(model::message::Media {
                caption: FormattedText::plain("Sunrise over the valley"),
                thumbnail: None,
                dimensions: None,
                preview: None,
                duration: 0,
                file: None,
                has_spoiler: false,
                start_timestamp: 0,
            }))),
        },
    );
    let guide = entity(
        message(
            3,
            26,
            3,
            4 * HOUR,
            "Have you seen the new guide? https://example.org/guide",
        ),
        "https://example.org/guide",
        EntityKind::Url,
    );
    let unsupported = Message {
        content: Content::Unsupported,
        ..message(3, 27, 3, 4 * HOUR - MINUTE, "")
    };
    vec![
        story,
        with_preview(
            guide,
            LinkPreview {
                url: "https://example.org/guide".to_owned(),
                site_name: "Example Trails".to_owned(),
                title: "Trail guide 2026".to_owned(),
                description: FormattedText::plain(
                    "Maps, distances and difficulty for every route in the park, updated for the new season.",
                ),
                is_above_text: false,
                is_url_in_text: true,
            },
        ),
        unsupported,
        with_preview(
            message(3, 28, ME, 3 * HOUR + MINUTE, "Looks good for Saturday"),
            LinkPreview {
                url: "https://example.org/weather".to_owned(),
                site_name: "Weather".to_owned(),
                title: "Weekend forecast".to_owned(),
                description: FormattedText::plain("Sunny, 18 degrees"),
                is_above_text: true,
                is_url_in_text: false,
            },
        ),
        channel_post,
    ]
}

/// Daily Landscapes: a post nobody commented on and one with comments
/// the user has not read.
fn landscapes() -> Vec<Message> {
    let post = |id, ago, text| Message {
        sender: Sender::Chat(ChatId(-200)),
        is_channel_post: true,
        views: 1520,
        ..message(-200, id, 0, ago, text)
    };
    let reaction = |emoji: &str, count, is_chosen, recent: &[i64]| Reaction {
        kind: ReactionKind::Emoji(emoji.to_owned()),
        count,
        is_chosen,
        recent: recent
            .iter()
            .map(|&user| Sender::User(UserId(user)))
            .collect(),
    };
    let reactions = |list| {
        Some(Reactions {
            list,
            ..Reactions::default()
        })
    };
    let commented = Message {
        reactions: reactions(vec![
            reaction("👍", 12, false, &[]),
            reaction("🔥", 48, true, &[]),
            reaction("😍", 3, false, &[]),
        ]),
        replies: Some(Replies {
            count: 12,
            recent: [3, 2, 4].map(|user| Sender::User(UserId(user))).to_vec(),
            last_read: MessageId(60),
            last: MessageId(70),
        }),
        ..post(
            49,
            27 * HOUR,
            "Sunrise over the valley, taken from the ridge trail.",
        )
    };
    let quiet = Message {
        reactions: reactions(vec![
            reaction("❤", 1, false, &[3]),
            reaction("👍", 1, false, &[2]),
        ]),
        replies: Some(Replies::default()),
        ..post(48, 28 * HOUR, "Morning mist over the lake.")
    };
    let time_only = DateFormat::Absolute {
        time: Precision::Short,
        date: Precision::None,
        has_day_of_week: false,
    };
    let walk = post(
        50,
        2 * HOUR,
        "Sunset walk today at 6:30 pm. Seats: +1 555 0100, card 4242 4242 4242 4242.",
    );
    let walk = entity(
        walk,
        "6:30 pm",
        EntityKind::DateTime {
            unix_time: now() + 6 * HOUR + 30 * MINUTE,
            format: Some(time_only),
        },
    );
    let walk = entity(walk, "+1 555 0100", EntityKind::PhoneNumber);
    let walk = entity(walk, "4242 4242 4242 4242", EntityKind::BankCardNumber);
    let code = entity(
        post(51, HOUR, "Route as code:\nlet km = 12.4;\nlet climb = 820;"),
        "let km = 12.4;\nlet climb = 820;",
        EntityKind::Pre {
            language: Some("rust".to_owned()),
        },
    );
    vec![quiet, commented, walk, code]
}

/// Saved Messages with a file a third of the way up.
pub(crate) fn uploads() -> Vec<(ChatId, Vec<Message>)> {
    let file = File {
        id: FileId(900),
        size: 3_565_158,
        uploaded: 1_258_291,
        is_uploading: true,
        path: None,
        is_downloading: false,
        downloaded: 0,
    };
    let sent = message(ME, 1, ME, 2 * HOUR, "Report for the trip");
    let uploading = Message {
        sending: Some(Sending::Pending),
        content: Content::Document {
            name: "trail-report.pdf".to_owned(),
            media: Media {
                caption: FormattedText::plain("Numbers for the Q3 hike"),
                thumbnail: None,
                dimensions: None,
                preview: None,
                duration: 0,
                file: Some(file),
                has_spoiler: false,
                start_timestamp: 0,
            },
        },
        ..message(ME, 2, ME, MINUTE, "")
    };
    vec![(ChatId(ME), vec![sent, uploading])]
}

/// Two reminders in Saved Messages, tomorrow and the day after, as TDLib
/// reports them once set, and the chat's flag that it has some.
pub(crate) fn reminders() -> Vec<Event> {
    let day = 24 * HOUR;
    let reminder = |id, at: i32, text| {
        let mut message = message(ME, id, ME, 0, text);
        message.date = now() + at;
        message.schedule = Some(model::message::Schedule::At(now() + at));
        Event::Messages(Change::New(Box::new(message)))
    };
    vec![
        reminder(20, day + 2 * HOUR, "Call the dentist"),
        reminder(21, 2 * day, "Pack the tent"),
        Event::Chats(model::chats::Change::HasScheduled {
            chat: ChatId(ME),
            has: true,
        }),
    ]
}

pub(crate) fn histories() -> Vec<(ChatId, Vec<Message>)> {
    vec![
        (ChatId(-100), group()),
        (ChatId(2), private()),
        (ChatId(3), ann()),
        (ChatId(-200), landscapes()),
    ]
}

/// Hiking Club's pins: the meeting point and the trail news.
pub fn pins() -> Vec<Event> {
    [32, 39]
        .map(|id| {
            Event::Messages(Change::Pinned {
                chat: ChatId(-100),
                message: MessageId(id),
                is_pinned: true,
            })
        })
        .into()
}
