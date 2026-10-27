//! Service message texts (`HistoryItem::setServiceMessageByAction`):
//! "Ann added Ben", "Channel photo updated".

use i18n::{Key, Strings, key};
use model::chat::{Chat, ChatKind};
use model::chats::Chats;
use model::message::{Message, Sender, Service};
use model::user::UserKind;

use super::headers::Replied;
use crate::dialogs::content_preview;

const MINUTE: i32 = 60;
const HOUR: i32 = 3600;
const DAY: i32 = 86_400;

pub(super) fn plural(strings: &Strings, key: Key, count: i32) -> String {
    i18n::fill(
        strings.plural(key, i64::from(count)),
        &[("count", &count.to_string())],
    )
}

pub(super) fn user_name(strings: &Strings, chats: &Chats, user: model::UserId) -> String {
    match chats.user(user) {
        Some(user) if user.kind == UserKind::Deleted => strings.text(key::lng_deleted).to_owned(),
        Some(user) => user.name(),
        None => String::new(),
    }
}

/// The sender's full name, the chat's title for posts on its behalf.
pub(super) fn from(strings: &Strings, chats: &Chats, message: &Message) -> String {
    sender_name(strings, chats, message.sender)
}

/// A user's full name or a chat's title.
pub(super) fn sender_name(strings: &Strings, chats: &Chats, sender: Sender) -> String {
    match sender {
        Sender::User(user) => user_name(strings, chats, user),
        Sender::Chat(chat) => chats
            .chat(chat)
            .map(|chat| chat.title.clone())
            .unwrap_or_default(),
    }
}

/// "A, B and C" (`lng_action_add_users_and_one`, `_and_last`).
fn names(strings: &Strings, names: &[String]) -> String {
    let Some((first, rest)) = names.split_first() else {
        return String::new();
    };
    rest.iter()
        .enumerate()
        .fold(first.clone(), |accumulated, (index, name)| {
            let key = if index + 1 == rest.len() {
                key::lng_action_add_users_and_last
            } else {
                key::lng_action_add_users_and_one
            };
            i18n::fill(
                strings.text(key),
                &[("accumulated", &accumulated), ("user", name)],
            )
        })
}

/// An auto-delete period in its largest unit (`Ui::FormatTTL`).
fn ttl(strings: &Strings, seconds: i32) -> String {
    let days = seconds / DAY;
    if seconds < DAY {
        plural(strings, key::lng_hours, seconds / HOUR)
    } else if days < 7 {
        plural(strings, key::lng_days, days)
    } else if days < 31 {
        let weeks = plural(strings, key::lng_weeks, days / 7);
        match days % 7 {
            0 => weeks,
            rest => format!("{weeks} {}", plural(strings, key::lng_days, rest)),
        }
    } else {
        plural(strings, key::lng_months, days / 31)
    }
}

/// A video chat's length in its largest whole unit.
fn call_length(strings: &Strings, seconds: i32) -> String {
    if seconds >= DAY {
        plural(strings, key::lng_days, seconds / DAY)
    } else if seconds >= HOUR {
        plural(strings, key::lng_hours, seconds / HOUR)
    } else if seconds >= MINUTE {
        plural(strings, key::lng_minutes, seconds / MINUTE)
    } else {
        plural(strings, key::lng_seconds, seconds)
    }
}

fn is_channel(chat: &Chat) -> bool {
    matches!(
        chat.kind,
        ChatKind::Supergroup {
            is_channel: true,
            ..
        }
    )
}

/// What a pinned message's notice names: its text, else its state.
fn pinned(strings: &Strings, from: &str, pinned: Replied<'_>) -> String {
    let fill = |key, args: &[(&str, &str)]| i18n::fill(strings.text(key), args);
    let media = match pinned {
        Replied::Loaded(pinned) => {
            let text = content_preview(strings, &pinned.content).0.text;
            return fill(
                key::lng_action_pinned_message,
                &[("from", from), ("text", &text)],
            );
        }
        Replied::Loading => strings.text(key::lng_contacts_loading),
        Replied::Deleted => strings.text(key::lng_deleted_message),
    };
    fill(
        key::lng_action_pinned_media,
        &[("from", from), ("media", media)],
    )
}

/// The text of a service message, or `None` for ones tdesktop hides.
pub(crate) fn text(
    strings: &Strings,
    chats: &Chats,
    chat: &Chat,
    message: &Message,
    service: &Service,
    pinned_message: Replied<'_>,
) -> Option<String> {
    let from = from(strings, chats, message);
    let fill = |key, args: &[(&str, &str)]| i18n::fill(strings.text(key), args);
    let with_from = |key| fill(key, &[("from", &from)]);
    let channel = is_channel(chat);
    Some(match service {
        Service::GroupCreated { title } => fill(
            key::lng_action_created_chat,
            &[("from", &from), ("title", title)],
        ),
        Service::SupergroupCreated { .. } if channel => {
            strings.text(key::lng_action_created_channel).to_owned()
        }
        Service::SupergroupCreated { title } => fill(
            key::lng_action_created_chat,
            &[("from", &from), ("title", title)],
        ),
        Service::TitleChanged { title } if channel => {
            fill(key::lng_action_changed_title_channel, &[("title", title)])
        }
        Service::TitleChanged { title } => fill(
            key::lng_action_changed_title,
            &[("from", &from), ("title", title)],
        ),
        Service::PhotoChanged if channel => strings
            .text(key::lng_action_changed_photo_channel)
            .to_owned(),
        Service::PhotoChanged => with_from(key::lng_action_changed_photo),
        Service::PhotoDeleted if channel => strings
            .text(key::lng_action_removed_photo_channel)
            .to_owned(),
        Service::PhotoDeleted => with_from(key::lng_action_removed_photo),
        Service::MembersAdded(users)
            if users.len() == 1 && message.sender == Sender::User(users[0]) =>
        {
            with_from(key::lng_action_user_joined)
        }
        Service::MembersAdded(users) => {
            let added: Vec<String> = users
                .iter()
                .map(|user| user_name(strings, chats, *user))
                .collect();
            let key = if added.len() == 1 {
                key::lng_action_add_user
            } else {
                key::lng_action_add_users_many
            };
            let list = names(strings, &added);
            fill(key, &[("from", &from), ("user", &list), ("users", &list)])
        }
        Service::JoinedByLink => with_from(key::lng_action_user_joined_by_link),
        Service::JoinedByRequest => with_from(key::lng_action_user_joined_by_request),
        Service::MemberRemoved(user) if message.sender == Sender::User(*user) => {
            with_from(key::lng_action_user_left)
        }
        Service::MemberRemoved(user) => fill(
            key::lng_action_kick_user,
            &[("from", &from), ("user", &user_name(strings, chats, *user))],
        ),
        Service::Upgraded | Service::Other => return None,
        Service::Pinned(_) => pinned(strings, &from, pinned_message),
        Service::ScreenshotTaken if message.is_outgoing => {
            strings.text(key::lng_action_you_took_screenshot).to_owned()
        }
        Service::ScreenshotTaken => with_from(key::lng_action_took_screenshot),
        Service::ContactRegistered => with_from(key::lng_action_user_registered),
        Service::AutoDelete { seconds: 0 } if channel => {
            strings.text(key::lng_action_ttl_removed_channel).to_owned()
        }
        Service::AutoDelete { seconds: 0 } if message.is_outgoing => {
            strings.text(key::lng_action_ttl_removed_you).to_owned()
        }
        Service::AutoDelete { seconds: 0 } => with_from(key::lng_action_ttl_removed),
        Service::AutoDelete { seconds } => {
            let duration = ttl(strings, *seconds);
            let key = if channel {
                key::lng_action_ttl_changed_channel
            } else if message.is_outgoing {
                key::lng_action_ttl_changed_you
            } else {
                key::lng_action_ttl_changed
            };
            fill(key, &[("from", &from), ("duration", &duration)])
        }
        Service::VideoChatStarted if channel => strings
            .text(key::lng_action_group_call_started_channel)
            .to_owned(),
        Service::VideoChatStarted => with_from(key::lng_action_group_call_started_group),
        Service::VideoChatEnded { seconds } => {
            let duration = call_length(strings, *seconds);
            let key = if channel {
                key::lng_action_group_call_finished
            } else {
                key::lng_action_group_call_finished_group
            };
            fill(key, &[("from", &from), ("duration", &duration)])
        }
        Service::Custom(text) => text.clone(),
        _ => return None,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use model::chats::Change;
    use model::message::Content;
    use model::user::{Badge, User, UserStatus};
    use model::{BasicGroupId, ChatId, MessageId, UserId};

    pub(in crate::history) fn user(id: i64, name: &str) -> Change {
        Change::User(Box::new(User {
            id: UserId(id),
            first_name: name.to_owned(),
            last_name: String::new(),
            status: UserStatus::Recently,
            photo: None,
            accent_color: 0,
            emoji_status: None,
            badge: Badge::None,
            is_premium: false,
            phone: model::Secret::default(),
            kind: UserKind::Regular,
            is_contact: false,
            username: String::new(),
        }))
    }

    fn group() -> Chat {
        Chat::new(
            ChatId(-1),
            ChatKind::Group {
                group: BasicGroupId(1),
            },
            "Club",
        )
    }

    fn service(from: i64, service: Service) -> (Message, Service) {
        let message = Message::new(
            MessageId(1),
            ChatId(-1),
            Sender::User(UserId(from)),
            Content::Service(service.clone()),
        );
        (message, service)
    }

    fn render(
        chats: &Chats,
        chat: &Chat,
        (message, service): (Message, Service),
    ) -> Option<String> {
        text(
            &Strings::english(),
            chats,
            chat,
            &message,
            &service,
            Replied::Loading,
        )
    }

    #[test]
    fn joins_adds_and_leaves_name_the_people() {
        let mut chats = Chats::default();
        for (id, name) in [(1, "Ann"), (2, "Ben"), (3, "Cy"), (4, "Di")] {
            chats.apply(user(id, name));
        }
        let chat = group();
        let added =
            |users: &[i64]| Service::MembersAdded(users.iter().map(|id| UserId(*id)).collect());
        assert_eq!(
            render(&chats, &chat, service(1, added(&[1]))).as_deref(),
            Some("Ann joined the group")
        );
        assert_eq!(
            render(&chats, &chat, service(1, added(&[2]))).as_deref(),
            Some("Ann added Ben")
        );
        assert_eq!(
            render(&chats, &chat, service(1, added(&[2, 3, 4]))).as_deref(),
            Some("Ann added Ben, Cy and Di")
        );
        assert_eq!(
            render(&chats, &chat, service(2, Service::MemberRemoved(UserId(2)))).as_deref(),
            Some("Ben left the group")
        );
        assert_eq!(render(&chats, &chat, service(1, Service::Upgraded)), None);
    }

    #[test]
    fn periods_use_their_largest_unit() {
        let strings = Strings::english();
        assert_eq!(ttl(&strings, DAY), "1 day");
        assert_eq!(ttl(&strings, 7 * DAY), "1 week");
        assert_eq!(ttl(&strings, 10 * DAY), "1 week 3 days");
        assert_eq!(ttl(&strings, 31 * DAY), "1 month");
        assert_eq!(call_length(&strings, 90), "1 minute");
        assert_eq!(call_length(&strings, 2 * HOUR + 5), "2 hours");
    }
}
