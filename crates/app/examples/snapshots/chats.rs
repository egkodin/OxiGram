//! Preset chats for the chat list snapshot: one of each kind of row.

use model::rights::MemberStatus;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone as _};

use model::action_bar::ActionBar;
use model::chat::{Chat, ChatKind, ChatList, Draft, Folder, Position};
use model::chats::{Change, ChatAction, MuteScope, Unread};
use model::message::{Content, Media, Message, Sender};
use model::text::{Entity, EntityKind, FormattedText};
use model::user::{Badge, User, UserKind, UserStatus};
use model::{BasicGroupId, ChatId, Event, FolderId, MessageId, SupergroupId, UserId};

pub(crate) const ME: i64 = 1;
pub(crate) const HOUR: i32 = 3600;
/// An inline bot that a fixture message was sent through.
pub(crate) const GIF_BOT: i64 = 7;

/// Noon today: messages hours "ago" land on the same days whenever the
/// snapshots run, so date badges stay where the clicks expect them.
pub(crate) fn now() -> i32 {
    let noon = Local::now()
        .date_naive()
        .and_hms_opt(12, 0, 0)
        .and_then(|noon| Local.from_local_datetime(&noon).earliest())
        .map_or_else(clock, |noon| noon.timestamp());
    i32::try_from(noon).unwrap_or(i32::MAX)
}

/// The real time, for online statuses that are relative to it.
fn clock() -> i64 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    i64::try_from(seconds).unwrap_or(i64::MAX)
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
        phone: model::Secret::default(),
        kind: UserKind::Regular,
        is_contact: true,
        username: String::new(),
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
        is_outgoing,
        date: now() - ago,
        ..Message::new(MessageId(id), ChatId(chat), sender, content)
    }
}

fn text(text: &str) -> Content {
    Content::text(FormattedText::plain(text))
}

fn chat(id: i64, kind: ChatKind, title: &str, order: i64, last: Option<Message>) -> Chat {
    Chat {
        accent_color: i32::try_from(id % 7).unwrap_or(0),
        last_message: last,
        positions: vec![Position {
            list: ChatList::Main,
            order,
            is_pinned: false,
        }],
        ..Chat::new(ChatId(id), kind, title)
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
        user(GIF_BOT, "GIF", ""),
    ];
    users[1].status = UserStatus::Online {
        expires: i32::try_from(clock()).unwrap_or(i32::MAX) + HOUR,
    };
    users[1].is_premium = true;
    users[3].status = UserStatus::Offline {
        was_online: i32::try_from(clock()).unwrap_or(i32::MAX) - 25 * 60,
    };
    users[4].kind = UserKind::Bot;
    users[4].is_contact = false;
    users[5].is_contact = false;
    users[4].badge = Badge::Verified;
    users[5].badge = Badge::Scam;
    users[6].kind = UserKind::Bot;
    users[6].is_contact = false;
    users[6].username = "gif".to_owned();

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
    maria.last_read_outbox = MessageId(17);
    maria.positions[0].is_pinned = true;
    // Work holds the two unread chats its tab counts, so its menu offers Mark as read.
    maria.positions.push(Position {
        list: ChatList::Folder(FolderId(1)),
        order: 2,
        is_pinned: false,
    });

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
            Content::text(italic),
        )),
    );
    ann.last_read_outbox = MessageId(30);

    let mut group = chat(
        -100,
        ChatKind::Group {
            group: BasicGroupId(100),
        },
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
    group.last_read_inbox = MessageId(36);
    group.unread_mentions = 1;
    group.unread_reactions = 2;
    group.mute_for = i32::MAX;
    group.uses_default_mute = false;

    let photo = Content::Photo(Media {
        caption: FormattedText::plain("Sunrise over the valley"),
        thumbnail: None,
        file: None,
    });
    let channel = chat(
        -200,
        ChatKind::Supergroup {
            supergroup: SupergroupId(200),
            is_channel: true,
        },
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
    // Someone new: the history offers to add or block them.
    ben.action_bar = Some(ActionBar::ReportAddBlock {
        can_unarchive: false,
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
        ChatKind::Supergroup {
            supergroup: SupergroupId(300),
            is_channel: false,
        },
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
    team.positions.push(Position {
        list: ChatList::Folder(FolderId(1)),
        order: 1,
        is_pinned: false,
    });

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
        (
            "Old Classmates",
            ChatKind::Group {
                group: BasicGroupId(901),
            },
            3,
        ),
        (
            "Alumni News",
            ChatKind::Supergroup {
                supergroup: SupergroupId(902),
                is_channel: true,
            },
            0,
        ),
        (
            "Flat Hunting",
            ChatKind::Group {
                group: BasicGroupId(903),
            },
            0,
        ),
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
    events.extend([
        Event::Chats(Change::Group {
            group: BasicGroupId(100),
            count: 12,
            status: MemberStatus::Member,
        }),
        Event::Chats(Change::Supergroup {
            supergroup: SupergroupId(200),
            count: 48_210,
            status: MemberStatus::Member,
        }),
        Event::Chats(Change::Supergroup {
            supergroup: SupergroupId(300),
            count: 9,
            status: MemberStatus::Member,
        }),
    ]);
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
        [
            (1, "Work", "Work"),
            (2, "Family", "Home"),
            (3, "Channels", "Channels"),
        ]
        .into_iter()
        .map(|(id, name, icon)| Folder {
            id: FolderId(id),
            name: FormattedText::plain(name),
            has_my_invite_links: false,
            icon: icon.to_owned(),
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
