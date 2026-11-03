//! How neighbors shape a message in the history: grouping into one bubble
//! run (`Element::computeIsAttachToPrevious`) and the date badges between
//! days.

use chrono::{DateTime, Datelike as _, NaiveDate, TimeZone};
use i18n::{Key, Strings, key};
use model::message::{Content, Message};

/// Messages closer than this join the previous one's run
/// (`kAttachMessageToPreviousSecondsDelta`).
const ATTACH_SECONDS: i32 = 900;

/// Whether `message` continues the run of `previous`: the same sender,
/// within 15 minutes, on the same day, and neither is a service message.
pub(crate) fn is_attached<Tz: TimeZone>(previous: &Message, message: &Message, tz: &Tz) -> bool {
    !is_service(previous)
        && !is_service(message)
        && previous.sender == message.sender
        && (message.date - previous.date).abs() < ATTACH_SECONDS
        && day(previous.date, tz) == day(message.date, tz)
}

pub(crate) fn is_service(message: &Message) -> bool {
    matches!(message.content, Content::Service(_))
}

/// The local day of a unix time.
pub(crate) fn day<Tz: TimeZone>(unix_time: i32, tz: &Tz) -> Option<NaiveDate> {
    tz.timestamp_opt(i64::from(unix_time), 0)
        .single()
        .map(|time| time.date_naive())
}

/// A message's time under its text.
// shortcut: 24-hour time as the chat list shows it; the system locale's format comes with OS integration in M7.
pub(crate) fn time<Tz: TimeZone>(unix_time: i32, tz: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    tz.timestamp_opt(i64::from(unix_time), 0)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_default()
}

/// The badge text for `date` (`langDayOfMonthFull`): "October 26", with
/// the year once it is far from today.
pub(crate) fn date_badge<Tz: TimeZone>(
    strings: &Strings,
    date: NaiveDate,
    now: &DateTime<Tz>,
) -> String {
    month_day(strings, date, now, month_day_key(date.month()))
}

/// "Oct 26", with the year once it is far from today (`langDayOfMonth`).
pub(crate) fn short_date<Tz: TimeZone>(
    strings: &Strings,
    date: NaiveDate,
    now: &DateTime<Tz>,
) -> String {
    month_day(strings, date, now, SMALL_MONTHS[date.month0() as usize])
}

fn month_day<Tz: TimeZone>(
    strings: &Strings,
    date: NaiveDate,
    now: &DateTime<Tz>,
    month: Key,
) -> String {
    let today = now.date_naive();
    let month = strings.text(month);
    let day = date.day().to_string();
    if is_year_shown(date, today) {
        i18n::fill(
            strings.text(key::lng_month_day_year),
            &[
                ("month", month),
                ("day", &day),
                ("year", &date.year().to_string()),
            ],
        )
    } else {
        i18n::fill(
            strings.text(key::lng_month_day),
            &[("month", month), ("day", &day)],
        )
    }
}

/// `langDateMaybeWithYear`: other years show unless they are the next or
/// previous one within three months.
fn is_year_shown(date: NaiveDate, today: NaiveDate) -> bool {
    let months = |date: NaiveDate| i64::from(date.year()) * 12 + i64::from(date.month0());
    let (year, current) = (date.year(), today.year());
    year != current && ((year - current).abs() > 1 || (months(date) - months(today)).abs() > 3)
}

pub(crate) const SMALL_MONTHS: [Key; 12] = [
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

fn month_day_key(month: u32) -> Key {
    match month {
        1 => key::lng_month_day1,
        2 => key::lng_month_day2,
        3 => key::lng_month_day3,
        4 => key::lng_month_day4,
        5 => key::lng_month_day5,
        6 => key::lng_month_day6,
        7 => key::lng_month_day7,
        8 => key::lng_month_day8,
        9 => key::lng_month_day9,
        10 => key::lng_month_day10,
        11 => key::lng_month_day11,
        _ => key::lng_month_day12,
    }
}

/// A count shortened past 10,000: "12.3K", "4M" (`FormatCountToShort`).
pub(crate) fn short_count(count: i32) -> String {
    let count = i64::from(count);
    let (divider, suffix) = match count.abs() {
        abs if abs >= 1_000_000_000 => (1_000_000_000, 'B'),
        abs if abs >= 1_000_000 => (1_000_000, 'M'),
        abs if abs >= 10_000 => (1_000, 'K'),
        _ => return count.to_string(),
    };
    let tenths = count / (divider / 10);
    match tenths % 10 {
        0 => format!("{}{suffix}", tenths / 10),
        rest => format!("{}.{}{suffix}", tenths / 10, rest.abs()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};
    use model::message::Sender;
    use model::text::FormattedText;
    use model::{ChatId, MessageId, UserId};

    fn message(sender: i64, date: i32) -> Message {
        Message {
            date,
            ..Message::new(
                MessageId(1),
                ChatId(1),
                Sender::User(UserId(sender)),
                Content::text(FormattedText::plain("hi")),
            )
        }
    }

    #[test]
    fn same_sender_within_fifteen_minutes_on_one_day_attaches() {
        let utc = Utc;
        let noon = 12 * 3600;
        assert!(is_attached(
            &message(1, noon),
            &message(1, noon + 899),
            &utc
        ));
        assert!(!is_attached(
            &message(1, noon),
            &message(1, noon + 900),
            &utc
        ));
        assert!(!is_attached(&message(1, noon), &message(2, noon + 1), &utc));
        // Midnight splits a run, as the date badge sits between.
        let midnight = 24 * 3600;
        assert!(!is_attached(
            &message(1, midnight - 10),
            &message(1, midnight + 10),
            &utc
        ));
        let mut service = message(1, noon + 1);
        service.content = Content::Service(model::message::Service::ScreenshotTaken);
        assert!(!is_attached(&message(1, noon), &service, &utc));
    }

    #[test]
    fn badges_show_the_year_only_far_from_today() {
        let strings = Strings::english();
        let now = FixedOffset::east_opt(0)
            .and_then(|tz| tz.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).single())
            .expect("a valid fixed date");
        let date = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).expect("a valid date");
        assert_eq!(date_badge(&strings, date(2026, 1, 3), &now), "January 3");
        assert_eq!(
            date_badge(&strings, date(2025, 12, 31), &now),
            "December 31"
        );
        assert_eq!(date_badge(&strings, date(2025, 6, 1), &now), "June 1, 2025");
        assert_eq!(
            date_badge(&strings, date(2023, 1, 3), &now),
            "January 3, 2023"
        );
    }

    #[test]
    fn counts_shorten_past_ten_thousand() {
        assert_eq!(short_count(9_999), "9999");
        assert_eq!(short_count(12_345), "12.3K");
        assert_eq!(short_count(20_000), "20K");
        assert_eq!(short_count(4_000_000), "4M");
    }
}
