//! Who saw an outgoing message, as the message menu's first line says it:
//! when the other user read it in a private chat (`WhenAction`,
//! `FormatReadDate`), who saw it in a small group (`WhoReadContextAction`).

use chrono::{DateTime, Datelike as _, Days, TimeZone};
use i18n::{Key, Strings, key};
use model::UserId;
use model::chats::Chats;
use model::message_actions::ReadDate;

use super::layout;
use super::service::{plural, user_name};

/// What TDLib told about who saw a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WhoRead {
    Date(ReadDate),
    Viewers(Vec<UserId>),
}

const SMALL_MONTHS: [Key; 12] = [
    key::lng_month1_small,
    key::lng_month2_small,
    key::lng_month3_small,
    key::lng_month4_small,
    key::lng_month5_small,
    key::lng_month6_small,
    key::lng_month7_small,
    key::lng_month8_small,
    key::lng_month9_small,
    key::lng_month10_small,
    key::lng_month11_small,
    key::lng_month12_small,
];

/// "today at 14:05", "yesterday at 9:30" or "Oct 3 at 18:00"; "read"
/// when either side hides read dates; none while unread or too old.
pub(super) fn text<Tz: TimeZone>(
    strings: &Strings,
    date: ReadDate,
    now: &DateTime<Tz>,
) -> Option<String>
where
    Tz::Offset: std::fmt::Display,
{
    let unix_time = match date {
        ReadDate::Read(unix_time) => unix_time,
        ReadDate::Hidden | ReadDate::MyHidden => {
            return Some(strings.text(key::lng_context_read_hidden).to_owned());
        }
        ReadDate::Unread | ReadDate::TooOld => return None,
    };
    let tz = now.timezone();
    let day = layout::day(unix_time, &tz)?;
    let time = layout::time(unix_time, &tz);
    let today = now.date_naive();
    let fill = |key, extra: &[(&str, &str)]| {
        let mut args = vec![("time", time.as_str())];
        args.extend_from_slice(extra);
        i18n::fill(strings.text(key), &args)
    };
    Some(if day == today {
        fill(key::lng_mediaview_today, &[])
    } else if today.checked_sub_days(Days::new(1)) == Some(day) {
        fill(key::lng_mediaview_yesterday, &[])
    } else {
        // shortcut: no year; TDLib keeps read dates for a week, so it never differs.
        let month = strings.text(SMALL_MONTHS[day.month0() as usize]);
        let date = i18n::fill(
            strings.text(key::lng_month_day),
            &[("month", month), ("day", &day.day().to_string())],
        );
        fill(key::lng_mediaview_date_time, &[("date", &date)])
    })
}

/// The one viewer's name, "3 Seen" or "Nobody Viewed" (`refreshText`).
// shortcut: no viewer userpics and no submenu listing them; with the reactions list (who reacted) in M3's reactions row.
pub(super) fn seen(strings: &Strings, chats: &Chats, viewers: &[UserId]) -> String {
    match viewers {
        [] => strings.text(key::lng_context_seen_text_none).to_owned(),
        [only] => user_name(strings, chats, *only),
        _ => plural(
            strings,
            key::lng_context_seen_text,
            i32::try_from(viewers.len()).unwrap_or(i32::MAX),
        ),
    }
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    #[test]
    fn read_dates_name_today_yesterday_or_the_day() {
        let tz = FixedOffset::east_opt(0).expect("UTC is a valid offset");
        let now = tz
            .with_ymd_and_hms(2026, 10, 27, 12, 0, 0)
            .single()
            .expect("a valid test time");
        let strings = Strings::english();
        let at = |day, hour| {
            let time = tz
                .with_ymd_and_hms(2026, 10, day, hour, 5, 0)
                .single()
                .expect("a valid test time");
            ReadDate::Read(i32::try_from(time.timestamp()).expect("fits"))
        };
        assert_eq!(
            text(&strings, at(27, 9), &now).as_deref(),
            Some("today at 09:05")
        );
        assert_eq!(
            text(&strings, at(26, 18), &now).as_deref(),
            Some("yesterday at 18:05")
        );
        assert_eq!(
            text(&strings, at(22, 7), &now).as_deref(),
            Some("Oct 22 at 07:05")
        );
        assert_eq!(
            text(&strings, ReadDate::Hidden, &now).as_deref(),
            Some("read")
        );
        assert_eq!(text(&strings, ReadDate::TooOld, &now), None);
    }

    #[test]
    fn seen_names_one_viewer_and_counts_more() {
        let strings = Strings::english();
        let mut chats = Chats::default();
        chats.apply(super::super::service::tests::user(2, "Maria"));
        assert_eq!(seen(&strings, &chats, &[]), "Nobody Viewed");
        assert_eq!(seen(&strings, &chats, &[UserId(2)]), "Maria");
        assert_eq!(seen(&strings, &chats, &[UserId(2), UserId(3)]), "2 Seen");
    }
}
