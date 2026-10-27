//! A user's status line ("online", "last seen 5 minutes ago") and the
//! order it sorts in (`Data::OnlineText`, `Data::SortByOnlineValue`).

use chrono::{DateTime, Days, TimeZone};
use gpui::App;
use i18n::{Strings, key};
use model::chat::{Chat, ChatKind};
use model::user::{User, UserKind, UserStatus};

use crate::lang::tr_plural_decimal;

const MINUTE: i64 = 60;
const HOUR: i64 = 3600;
const DAY: i64 = 86_400;
/// Past this, "last seen" names the time of day instead of hours ago.
const HOURS_AGO_LIMIT: i64 = 12;

/// The status text and whether it is "online" (drawn in the active color).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Status {
    pub text: String,
    pub is_online: bool,
}

// shortcut: bots show "bot" only; the active-users count and username suffixes come with bot profiles in M10.
pub(crate) fn status<Tz: TimeZone>(strings: &Strings, user: &User, now: &DateTime<Tz>) -> Status {
    let offline = |key| Status {
        text: strings.text(key).to_owned(),
        is_online: false,
    };
    match user.kind {
        UserKind::Bot => return offline(key::lng_status_bot),
        UserKind::Deleted | UserKind::Unknown => return offline(key::lng_status_offline),
        UserKind::Regular => {}
    }
    let now_unix = now.timestamp();
    let was_online = match user.status {
        UserStatus::Online { expires } if i64::from(expires) > now_unix => {
            return Status {
                text: strings.text(key::lng_status_online).to_owned(),
                is_online: true,
            };
        }
        UserStatus::Online { expires } => expires,
        UserStatus::Offline { was_online } => was_online,
        UserStatus::Empty => return offline(key::lng_status_offline),
        UserStatus::Recently => return offline(key::lng_status_recently),
        UserStatus::LastWeek => return offline(key::lng_status_last_week),
        UserStatus::LastMonth => return offline(key::lng_status_last_month),
    };
    Status {
        text: last_seen(strings, i64::from(was_online), now),
        is_online: false,
    }
}

fn last_seen<Tz: TimeZone>(strings: &Strings, till: i64, now: &DateTime<Tz>) -> String {
    let ago = now.timestamp() - till;
    let count = |key, count: i64| {
        strings
            .plural(key, count)
            .replace("{count}", &count.to_string())
    };
    if ago < MINUTE {
        return strings.text(key::lng_status_lastseen_now).to_owned();
    } else if ago < HOUR {
        return count(key::lng_status_lastseen_minutes, ago / MINUTE);
    } else if ago / HOUR < HOURS_AGO_LIMIT {
        return count(key::lng_status_lastseen_hours, ago / HOUR);
    }
    let Some(then) = now.timezone().timestamp_opt(till, 0).single() else {
        return strings.text(key::lng_status_offline).to_owned();
    };
    // shortcut: 24-hour time and dd.mm.yy dates, as in the chat list, until the locale formats of M7.
    let time = then.naive_local().format("%H:%M").to_string();
    let (then_day, today) = (then.date_naive(), now.date_naive());
    if then_day == today {
        i18n::fill(
            strings.text(key::lng_status_lastseen_today),
            &[("time", &time)],
        )
    } else if then_day.checked_add_days(Days::new(1)) == Some(today) {
        i18n::fill(
            strings.text(key::lng_status_lastseen_yesterday),
            &[("time", &time)],
        )
    } else {
        let date = then.naive_local().format("%d.%m.%y").to_string();
        i18n::fill(
            strings.text(key::lng_status_lastseen_date),
            &[("date", &date)],
        )
    }
}

/// Larger sorts first: online users by when they go offline, then by how
/// recently they were seen; bots and unknown last.
pub(crate) fn sort_value(user: &User, now: i64) -> i64 {
    if user.kind != UserKind::Regular {
        return -1;
    }
    match user.status {
        UserStatus::Online { expires } => i64::from(expires),
        UserStatus::Offline { was_online } => i64::from(was_online),
        UserStatus::Recently => now - 3 * DAY,
        UserStatus::LastWeek => now - 7 * DAY,
        UserStatus::LastMonth => now - 30 * DAY,
        UserStatus::Empty => 0,
    }
}

/// "5 members" or "5 subscribers" once the count is known.
pub(crate) fn members_status(chat: &Chat, count: Option<i32>, cx: &App) -> Option<String> {
    let key = match chat.kind {
        ChatKind::Supergroup {
            is_channel: true, ..
        } => key::lng_chat_status_subscribers,
        _ => key::lng_chat_status_members,
    };
    count.map(|count| tr_plural_decimal(cx, key, i64::from(count)))
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;
    use model::UserId;
    use model::user::Badge;

    use super::*;

    fn user(status: UserStatus) -> User {
        User {
            id: UserId(1),
            first_name: "Ann".to_owned(),
            last_name: String::new(),
            status,
            photo: None,
            accent_color: 0,
            emoji_status: None,
            badge: Badge::None,
            is_premium: false,
            kind: UserKind::Regular,
            is_contact: true,
            username: String::new(),
        }
    }

    fn noon() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(0)
            .and_then(|zone| zone.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).single())
            .expect("a valid fixed date")
    }

    #[test]
    fn status_follows_tdesktop_phrases() {
        let strings = Strings::default();
        let now = noon();
        let at = |seconds_ago: i64| {
            status(
                &strings,
                &user(UserStatus::Offline {
                    was_online: i32::try_from(now.timestamp() - seconds_ago).expect("fits"),
                }),
                &now,
            )
            .text
        };
        let online = status(
            &strings,
            &user(UserStatus::Online {
                expires: i32::try_from(now.timestamp() + 60).expect("fits"),
            }),
            &now,
        );
        assert_eq!(
            online,
            Status {
                text: "online".to_owned(),
                is_online: true
            }
        );
        assert_eq!(at(10), "last seen just now");
        assert_eq!(at(5 * MINUTE), "last seen 5 minutes ago");
        assert_eq!(at(HOUR), "last seen 1 hour ago");
        assert_eq!(at(HOURS_AGO_LIMIT * HOUR), "last seen today at 00:00");
        assert_eq!(at(DAY), "last seen yesterday at 12:00");
        assert_eq!(at(10 * DAY), "last seen 28.02.26");
        assert_eq!(
            status(&strings, &user(UserStatus::Recently), &now).text,
            "last seen recently"
        );
    }

    #[test]
    fn online_sorts_before_recent_before_long_ago() {
        let now = noon().timestamp();
        let online = sort_value(&user(UserStatus::Online { expires: i32::MAX }), now);
        let recent = sort_value(&user(UserStatus::Recently), now);
        let long_ago = sort_value(&user(UserStatus::Empty), now);
        assert!(online > recent && recent > long_ago);
    }
}
