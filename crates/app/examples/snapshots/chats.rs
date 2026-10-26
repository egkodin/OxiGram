//! Preset chats for the chat list snapshot: one of each kind of row.

use std::time::{SystemTime, UNIX_EPOCH};

use model::chat::{Chat, ChatKind, ChatList, Draft, Folder, Position};
use model::chats::{Change, ChatAction, MuteScope, Unread};
use model::message::{Content, Media, Message, Sender};
use model::text::{Entity, EntityKind, FormattedText};
use model::user::{Badge, User, UserKind, UserStatus};
use model::{ChatId, Event, FolderId, MessageId, UserId};

const ME: i64 = 1;
const HOUR: i32 = 3600;

fn now() -> i32 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    i32::try_from(seconds).unwrap_or(i32::MAX)
}

fn user(id: i64, first: &str, last: &str) -> User {
    User {
        id: UserId(id),
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        status: UserStatus::Recently,
        photo: None,
        accent_color: i32::try_from(id % 7).unwrap_or(0),
        emoji_status: None,
        badge: Badge::None,
        is_premium: false,
        kind: UserKind::Regular,
        is_contact: true,
    }
}

fn message(
    chat: i64,
    id: i64,
    sender: Sender,
    is_outgoing: bool,
    ago: i32,
    content: Content,
) -> Message {
    Message {
        id: MessageId(id),
        chat: ChatId(chat),
        sender,
        is_outgoing,
        date: now() - ago,
        content,
    }
}

fn text(text: &str) -> Content {
    Content::Text(FormattedText::plain(text))
}

fn chat(id: i64, kind: ChatKind, title: &str, order: i64, last: Option<Message>) -> Chat {
    Chat {
        id: ChatId(id),
        kind,
        title: title.to_owned(),
        photo: None,
        accent_color: i32::try_from(id % 7).unwrap_or(0),
        last_message: last,
        positions: vec![Position {
            list: ChatList::Main,
            order,
            is_pinned: false,
        }],
        unread_count: 0,
        last_read_inbox: MessageId(0),
        last_read_outbox: MessageId(0),
        unread_mentions: 0,
        unread_reactions: 0,
        is_marked_as_unread: false,
        mute_for: 0,
        uses_default_mute: true,
        draft: None,
    }
}

fn private(id: i64) -> ChatKind {
    ChatKind::Private { user: UserId(id) }
}

/// The events that fill the chat list, after sign-in.
pub(crate) fn events() -> Vec<Event> {
    let mut users = vec![
        user(ME, "Alex", "Doe"),
        user(2, "Maria", "Garcia"),
        user(3, "Ann", "Lee"),
        user(4, "Ben", ""),
        user(5, "Weather", "Bot"),
        user(6, "Totally Real", "Support"),
    ];
    users[1].status = UserStatus::Online {
        expires: now() + HOUR,
    };
    users[1].is_premium = true;
    users[3].status = UserStatus::Offline {
        was_online: now() - 25 * 60,
    };
    users[4].kind = UserKind::Bot;
    users[4].is_contact = false;
    users[5].is_contact = false;
    users[4].badge = Badge::Verified;
    users[5].badge = Badge::Scam;

    let mut saved = chat(
        ME,
        private(ME),
        "Alex Doe",
        100,
        Some(message(
            ME,
            10,
            Sender::User(UserId(ME)),
            true,
            2 * HOUR,
            text("Shopping list: milk, eggs, bread"),
        )),
    );
    saved.positions[0].is_pinned = true;

    let mut maria = chat(
        2,
        private(2),
        "Maria Garcia",
        90,
        Some(message(
            2,
            20,
            Sender::User(UserId(2)),
            false,
            60,
            text("Are we still on for tonight?"),
        )),
    );
    maria.unread_count = 2;
    maria.positions[0].is_pinned = true;

    let italic = FormattedText {
        text: "on my way, see you soon".to_owned(),
        entities: vec![Entity {
            range: 10..23,
            kind: EntityKind::Italic,
        }],
    };
    let mut ann = chat(
        3,
        private(3),
        "Ann Lee",
        80,
        Some(message(
            3,
            30,
            Sender::User(UserId(ME)),
            true,
            3 * HOUR,
            Content::Text(italic),
        )),
    );
    ann.last_read_outbox = MessageId(30);

    let mut group = chat(
        -100,
        ChatKind::Group,
        "Hiking Club",
        70,
        Some(message(
            -100,
            40,
            Sender::User(UserId(3)),
            false,
            5 * HOUR,
            text("Photos from Saturday are up"),
        )),
    );
    group.unread_count = 14;
    group.unread_mentions = 1;
    group.mute_for = i32::MAX;
    group.uses_default_mute = false;

    let photo = Content::Photo(Media {
        caption: FormattedText::plain("Sunrise over the valley"),
        thumbnail: None,
    });
    let channel = chat(
        -200,
        ChatKind::Supergroup { is_channel: true },
        "Daily Landscapes",
        60,
        Some(message(
            -200,
            50,
            Sender::Chat(ChatId(-200)),
            false,
            26 * HOUR,
            photo,
        )),
    );

    let mut ben = chat(
        4,
        private(4),
        "Ben",
        50,
        Some(message(
            4,
            60,
            Sender::User(UserId(ME)),
            true,
            3 * 24 * HOUR,
            text("Sent you the file"),
        )),
    );
    ben.draft = Some(Draft {
        date: now() - 30 * 60,
        text: FormattedText::plain("I'll check it tomorrow"),
    });

    let bot = chat(
        5,
        private(5),
        "Weather Bot",
        40,
        Some(message(
            5,
            70,
            Sender::User(UserId(5)),
            false,
            4 * 24 * HOUR,
            text("Tomorrow: 18°, light rain"),
        )),
    );

    let mut team = chat(
        -300,
        ChatKind::Supergroup { is_channel: false },
        "Design Team",
        30,
        Some(message(
            -300,
            80,
            Sender::User(UserId(ME)),
            true,
            9 * 24 * HOUR,
            Content::Sticker {
                emoji: "👍".to_owned(),
            },
        )),
    );
    team.last_read_outbox = MessageId(79);
    team.is_marked_as_unread = true;

    let scam = chat(
        6,
        private(6),
        "Totally Real Support",
        20,
        Some(message(
            6,
            90,
            Sender::User(UserId(6)),
            false,
            40 * 24 * HOUR,
            Content::Voice {
                caption: FormattedText::default(),
            },
        )),
    );

    let archived = [
        ("Old Classmates", ChatKind::Group, 3),
        ("Alumni News", ChatKind::Supergroup { is_channel: true }, 0),
        ("Flat Hunting", ChatKind::Group, 0),
    ]
    .into_iter()
    .zip(1_i64..)
    .map(|((title, kind, unread), index)| {
        let id = -900 - index;
        let mut chat = chat(
            id,
            kind,
            title,
            0,
            Some(message(
                id,
                1,
                Sender::Chat(ChatId(id)),
                false,
                i32::try_from(index).unwrap_or(0) * 30 * HOUR,
                text("…"),
            )),
        );
        chat.positions = vec![Position {
            list: ChatList::Archive,
            order: 100 - index,
            is_pinned: false,
        }];
        chat.unread_count = unread;
        chat.mute_for = i32::MAX;
        chat.uses_default_mute = false;
        chat
    });

    let mut events: Vec<Event> = vec![Event::Me(UserId(ME))];
    events.extend(
        users
            .into_iter()
            .map(|user| Event::Chats(Change::User(Box::new(user)))),
    );
    events.push(Event::Chats(Change::DefaultMute {
        scope: MuteScope::Group,
        mute_for: 0,
    }));
    for chat in [saved, maria, ann, group, channel, ben, bot, team, scam]
        .into_iter()
        .chain(archived)
    {
        events.push(Event::Chats(Change::Chat(Box::new(chat))));
    }
    events.push(Event::Chats(Change::UnreadChats {
        list: ChatList::Archive,
        unread: Unread {
            chats: 1,
            unmuted_chats: 0,
        },
    }));
    events.push(Event::Chats(Change::Folders(
        [(1, "Work"), (2, "Family"), (3, "Channels")]
            .into_iter()
            .map(|(id, name)| Folder {
                id: FolderId(id),
                name: FormattedText::plain(name),
            })
            .collect(),
    )));
    for (list, chats, unmuted_chats) in [
        (ChatList::Main, 4, 3),
        (ChatList::Folder(FolderId(1)), 2, 2),
        (ChatList::Folder(FolderId(3)), 1, 0),
    ] {
        events.push(Event::Chats(Change::UnreadChats {
            list,
            unread: Unread {
                chats,
                unmuted_chats,
            },
        }));
    }
    events.push(Event::Chats(Change::Action {
        chat: ChatId(-300),
        sender: Sender::User(UserId(2)),
        action: Some(ChatAction::Typing),
    }));
    events
}
