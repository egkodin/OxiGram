//! Preset messages for the history snapshots: a group conversation over
//! two days and a private chat, both ending in their chat's last message.

use model::Event;
use model::history::Change;
use model::message::{Content, Forward, LinkPreview, Message, Origin, ReplyTo, Sender, Service};
use model::text::{Entity, EntityKind, FormattedText};
use model::{ChatId, MessageId, UserId};

use crate::chats::{HOUR, ME, now};

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
        message(-100, 40, 3, 5 * HOUR, "Photos from Saturday are up"),
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

/// Ann Lee: link previews below and above the text, and a message TDLib
/// cannot show.
fn ann() -> Vec<Message> {
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
    ]
}

pub(crate) fn histories() -> Vec<(ChatId, Vec<Message>)> {
    vec![
        (ChatId(-100), group()),
        (ChatId(2), private()),
        (ChatId(3), ann()),
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
