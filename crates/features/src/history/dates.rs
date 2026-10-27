//! Dates in message text in the reader's words (`FormatDateRelative`,
//! `FormatDateWithFlags`): the history shows messages with each date's
//! text replaced, so layout, selection and copy agree on one text.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use chrono::{DateTime, Datelike as _, TimeZone};
use i18n::{Key, Strings, key};
use model::message::{Content, Message};
use model::text::{DateFormat, EntityKind, FormattedText, Precision};

use super::layout;
use super::service::plural;
use crate::dialogs::weekday_key;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
/// tdesktop counts months of 30 days and years of 365.
const MONTH: i64 = 30 * DAY;
const YEAR: i64 = 365 * DAY;

const FULL_WEEKDAYS: [Key; 7] = [
    key::lng_hours_monday,
    key::lng_hours_tuesday,
    key::lng_hours_wednesday,
    key::lng_hours_thursday,
    key::lng_hours_friday,
    key::lng_hours_saturday,
    key::lng_hours_sunday,
];

/// The messages with their dates formatted; borrowed when none has one.
/// A message formatted as it was in `shown` keeps that copy, so the list
/// does not take it for an edit.
// shortcut: relative dates refresh when the history changes, not on a timer (`registerFormattedDateUpdate`); with M8 polish.
pub(super) fn with_dates<'a, Tz: TimeZone>(
    messages: &'a [Arc<Message>],
    shown: &[Arc<Message>],
    strings: &Strings,
    now: &DateTime<Tz>,
) -> Cow<'a, [Arc<Message>]>
where
    Tz::Offset: std::fmt::Display,
{
    if !messages.iter().any(|message| has_dates(message)) {
        return Cow::Borrowed(messages);
    }
    Cow::Owned(
        messages
            .iter()
            .map(|message| match formatted(message, strings, now) {
                Some(message) => shown
                    .binary_search_by_key(&message.id, |shown| shown.id)
                    .ok()
                    .map(|index| &shown[index])
                    .filter(|shown| ***shown == message)
                    .map_or_else(|| Arc::new(message), Arc::clone),
                None => Arc::clone(message),
            })
            .collect(),
    )
}

// shortcut: captions keep their dates as sent until media (M5).
fn text_of(message: &Message) -> Option<&FormattedText> {
    match &message.content {
        Content::Text { text, .. } => Some(text),
        _ => None,
    }
}

fn has_dates(message: &Message) -> bool {
    text_of(message).is_some_and(|text| {
        text.entities.iter().any(|entity| {
            matches!(
                entity.kind,
                EntityKind::DateTime {
                    format: Some(_),
                    ..
                }
            )
        })
    })
}

fn formatted<Tz: TimeZone>(
    message: &Message,
    strings: &Strings,
    now: &DateTime<Tz>,
) -> Option<Message>
where
    Tz::Offset: std::fmt::Display,
{
    let text = text_of(message).filter(|_| has_dates(message))?;
    let text = replaced(text, |unix_time, format| {
        self::format(strings, unix_time, format, now)
    });
    let mut message = message.clone();
    if let Content::Text { text: old, .. } = &mut message.content {
        *old = text;
    }
    Some(message)
}

/// The text with each formatted date's range holding `shown(date)`, and
/// every entity moved to match.
fn replaced(text: &FormattedText, shown: impl Fn(i32, DateFormat) -> String) -> FormattedText {
    let mut dates: Vec<(Range<usize>, String)> = Vec::new();
    for entity in &text.entities {
        if let EntityKind::DateTime {
            unix_time,
            format: Some(format),
        } = entity.kind
        {
            dates.push((entity.range.clone(), shown(unix_time, format)));
        }
    }
    dates.sort_by_key(|(range, _)| range.start);
    // Overlapping dates would make the offsets ambiguous; the first wins.
    dates.dedup_by(|later, earlier| later.0.start < earlier.0.end);
    let moved = |offset: usize, is_end: bool| {
        let (mut removed, mut added) = (0, 0);
        for (range, new) in &dates {
            if offset >= range.end {
                removed += range.len();
                added += new.len();
            } else if offset > range.start {
                let start = range.start - removed + added;
                return if is_end { start + new.len() } else { start };
            } else {
                break;
            }
        }
        offset - removed + added
    };
    let mut result = String::with_capacity(text.text.len());
    let mut from = 0;
    for (range, new) in &dates {
        result.push_str(&text.text[from..range.start]);
        result.push_str(new);
        from = range.end;
    }
    result.push_str(&text.text[from..]);
    let mut entities = text.entities.clone();
    for entity in &mut entities {
        entity.range = moved(entity.range.start, false)..moved(entity.range.end, true);
    }
    FormattedText {
        text: result,
        entities,
    }
}

/// A date as `format` asks, seen from `now`.
pub(super) fn format<Tz: TimeZone>(
    strings: &Strings,
    unix_time: i32,
    format: DateFormat,
    now: &DateTime<Tz>,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    match format {
        DateFormat::Relative => relative(strings, i64::from(unix_time) - now.timestamp()),
        DateFormat::Absolute {
            time,
            date,
            has_day_of_week,
        } => absolute(strings, unix_time, (time, date, has_day_of_week), now),
    }
}

/// "in 5 minutes", "2 days ago", "now".
fn relative(strings: &Strings, delta: i64) -> String {
    let distance = delta.abs();
    let is_future = delta > 0;
    let (unit, past, future) = match distance {
        0 => return strings.text(key::lng_date_relative_now).to_owned(),
        1..MINUTE => (
            1,
            key::lng_date_relative_seconds_ago,
            key::lng_date_relative_in_seconds,
        ),
        MINUTE..HOUR => (
            MINUTE,
            key::lng_date_relative_minutes_ago,
            key::lng_date_relative_in_minutes,
        ),
        HOUR..DAY => (
            HOUR,
            key::lng_date_relative_hours_ago,
            key::lng_date_relative_in_hours,
        ),
        DAY..MONTH => (
            DAY,
            key::lng_date_relative_days_ago,
            key::lng_date_relative_in_days,
        ),
        MONTH..YEAR => (
            MONTH,
            key::lng_date_relative_months_ago,
            key::lng_date_relative_in_months,
        ),
        _ => (
            YEAR,
            key::lng_date_relative_years_ago,
            key::lng_date_relative_in_years,
        ),
    };
    let count = i32::try_from(distance / unit).unwrap_or(i32::MAX);
    plural(strings, if is_future { future } else { past }, count)
}

/// "Mon Oct 27 14:05", "Monday October 27": the parts the format asks for,
/// in tdesktop's order.
fn absolute<Tz: TimeZone>(
    strings: &Strings,
    unix_time: i32,
    (time, date, has_day_of_week): (Precision, Precision, bool),
    now: &DateTime<Tz>,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let tz = now.timezone();
    let Some(moment) = tz.timestamp_opt(i64::from(unix_time), 0).single() else {
        return String::new();
    };
    let day = moment.date_naive();
    let weekday = day.weekday().number_from_monday();
    let mut parts = Vec::new();
    if has_day_of_week {
        parts.push(if date == Precision::Long {
            strings.text(FULL_WEEKDAYS[weekday as usize - 1]).to_owned()
        } else {
            strings.text(weekday_key(weekday)).to_owned()
        });
    }
    match date {
        Precision::Long => parts.push(layout::date_badge(strings, day, now)),
        Precision::Short => parts.push(layout::short_date(strings, day, now)),
        Precision::None => {}
    }
    // shortcut: 24-hour times for every locale; locale formats come with M7.
    match time {
        Precision::Long => parts.push(moment.format("%H:%M:%S").to_string()),
        Precision::Short => parts.push(layout::time(unix_time, &tz)),
        Precision::None => {}
    }
    if parts.is_empty() {
        parts.push(layout::short_date(strings, day, now));
        parts.push(layout::time(unix_time, &tz));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;
    use model::text::Entity;

    use super::*;

    fn now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(0)
            .expect("UTC is a valid offset")
            .with_ymd_and_hms(2026, 10, 27, 12, 0, 0)
            .single()
            .expect("a valid test time")
    }

    fn at(delta: i64) -> i32 {
        i32::try_from(now().timestamp() + delta).expect("fits")
    }

    #[test]
    fn relative_dates_count_the_largest_unit() {
        let strings = Strings::english();
        let shown = |delta| format(&strings, at(delta), DateFormat::Relative, &now());
        assert_eq!(shown(0), "now");
        assert_eq!(shown(-30), "30 seconds ago");
        assert_eq!(shown(5 * MINUTE + 10), "in 5 minutes");
        assert_eq!(shown(-HOUR), "1 hour ago");
        assert_eq!(shown(3 * DAY), "in 3 days");
        assert_eq!(shown(-2 * YEAR), "2 years ago");
    }

    #[test]
    fn absolute_dates_show_the_parts_asked_for() {
        let strings = Strings::english();
        let shown = |time, date, has_day_of_week| {
            let format = DateFormat::Absolute {
                time,
                date,
                has_day_of_week,
            };
            self::format(&strings, at(2 * HOUR + 5 * MINUTE), format, &now())
        };
        assert_eq!(
            shown(Precision::Short, Precision::Short, true),
            "Tue Oct 27 14:05"
        );
        assert_eq!(
            shown(Precision::None, Precision::Long, true),
            "Tuesday October 27"
        );
        assert_eq!(shown(Precision::Long, Precision::None, false), "14:05:00");
    }

    #[test]
    fn replacing_dates_moves_the_entities_after_and_around_them() {
        let text = FormattedText {
            text: "meet at DATE, bring snacks".to_owned(),
            entities: vec![
                Entity {
                    range: 8..12,
                    kind: EntityKind::DateTime {
                        unix_time: 0,
                        format: Some(DateFormat::Relative),
                    },
                },
                Entity {
                    range: 0..12,
                    kind: EntityKind::Bold,
                },
                Entity {
                    range: 14..19,
                    kind: EntityKind::Italic,
                },
            ],
        };
        let result = replaced(&text, |_, _| "in 5 minutes".to_owned());
        assert_eq!(result.text, "meet at in 5 minutes, bring snacks");
        let ranges: Vec<_> = result.entities.iter().map(|e| e.range.clone()).collect();
        assert_eq!(ranges, [8..20, 0..20, 22..27]);
        assert_eq!(&result.text[22..27], "bring");
    }
}
