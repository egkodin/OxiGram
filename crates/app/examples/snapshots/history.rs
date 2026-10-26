//! Preset messages for the history snapshots: a group conversation over
//! two days and a private chat, both ending in their chat's last message.

use model::message::{Content, Message, Sender};
use model::text::FormattedText;
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
            Content::Text(FormattedText::plain(text)),
        )
    }
}

/// Hiking Club: runs by one sender group, a new day gets a date badge.
fn group() -> Vec<Message> {
    let day = 24 * HOUR;
    vec![
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
        message(
            -100,
            34,
            4,
            day + 2 * HOUR - 5 * MINUTE,
            "Me too, I'll bring the map and some snacks for everyone. Does anyone need a ride from the station?",
        ),
        message(
            -100,
            35,
            3,
            6 * HOUR,
            "That was a great hike, thanks everyone",
        ),
        message(-100, 36, 3, 6 * HOUR - MINUTE, "Uploading the photos now"),
        message(
            -100,
            37,
            ME,
            5 * HOUR + 30 * MINUTE,
            "Can't wait to see them",
        ),
        message(
            -100,
            38,
            ME,
            5 * HOUR + 29 * MINUTE,
            "Same time next month?",
        ),
        message(-100, 39, 2, 5 * HOUR + 10 * MINUTE, "Absolutely"),
        message(-100, 40, 3, 5 * HOUR, "Photos from Saturday are up"),
    ]
}

/// Maria Garcia: read and unread outgoing messages and replies.
fn private() -> Vec<Message> {
    vec![
        message(2, 15, 2, 3 * HOUR, "Hi! Did you get the tickets?"),
        message(2, 16, ME, 2 * HOUR, "Yes, two seats in row 5"),
        message(2, 17, ME, 2 * HOUR - MINUTE, "The show starts at 8"),
        message(2, 18, 2, 90 * MINUTE, "Perfect"),
        message(2, 19, ME, 70 * MINUTE, "See you at the entrance"),
        message(2, 20, 2, MINUTE, "Are we still on for tonight?"),
    ]
}

pub(crate) fn histories() -> Vec<(ChatId, Vec<Message>)> {
    vec![(ChatId(-100), group()), (ChatId(2), private())]
}
