//! Who saw or reacted to a message, as the message menu's first line says
//! it: when the other user read it in a private chat (`WhenAction`,
//! `FormatReadDate`), who saw it or reacted in a group
//! (`WhoReactedContextAction`).

use chrono::{DateTime, Datelike as _, Days, TimeZone};
use i18n::{Key, Strings, key};
use model::UserId;
use model::message_actions::ReadDate;

use super::layout;
use super::service::plural;

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

/// Who saw or reacted to a message, as the menu line counts them
/// (`WhoReadContent`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Seen {
    /// Who reacted, then who only saw it; `true` for those who reacted.
    pub participants: Vec<(String, bool)>,
    /// All reactions, also those of people TDLib did not list.
    pub reactions: i32,
    /// How many saw it.
    pub read: i32,
    pub kind: SeenKind,
}

/// What seeing a message means: "Seen", "Listened", or "Reacted" for
/// messages nobody can tell who read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SeenKind {
    Seen,
    Watched,
    Listened,
    Reacted,
}

/// "Maria", "2/5 Reacted", "7 Reactions", "3 Seen" or "Nobody Viewed"
/// (`Action::refreshText`).
// shortcut: no userpics after the text and no submenu listing everyone; the reactions list shows them.
pub(super) fn seen(strings: &Strings, seen: &Seen) -> String {
    let users = i32::try_from(seen.participants.len()).unwrap_or(i32::MAX);
    let only_seen = seen.participants.iter().filter(|(_, r)| !r).count();
    let count = seen.reactions.max(users);
    let short = |key, count: i32, text: String| {
        i18n::fill(strings.plural(key, i64::from(count)), &[("count", &text)])
    };
    if let [(name, _)] = seen.participants.as_slice() {
        return name.clone();
    }
    if seen.reactions > 0 && seen.reactions <= seen.read {
        let text = format!("{}/{}", layout::short_count(seen.reactions), seen.read);
        return short(key::lng_context_seen_reacted, seen.reactions, text);
    }
    if seen.kind == SeenKind::Reacted || (count > 0 && (seen.reactions > users || only_seen == 0)) {
        return if count == 0 {
            strings.text(key::lng_context_seen_reacted_none).to_owned()
        } else if seen.reactions > seen.read {
            let text = layout::short_count(seen.reactions);
            short(key::lng_context_seen_reactions_count, seen.reactions, text)
        } else {
            short(
                key::lng_context_seen_reacted,
                count,
                layout::short_count(count),
            )
        };
    }
    let (some, none) = match seen.kind {
        SeenKind::Watched => (
            key::lng_context_seen_watched,
            key::lng_context_seen_watched_none,
        ),
        SeenKind::Listened => (
            key::lng_context_seen_listened,
            key::lng_context_seen_listened_none,
        ),
        SeenKind::Seen | SeenKind::Reacted => {
            (key::lng_context_seen_text, key::lng_context_seen_text_none)
        }
    };
    if count == 0 {
        strings.text(none).to_owned()
    } else {
        plural(strings, some, count)
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

    fn names(reacted: &[&str], only_seen: &[&str]) -> Vec<(String, bool)> {
        let reacted = reacted.iter().map(|name| ((*name).to_owned(), true));
        let seen = only_seen.iter().map(|name| ((*name).to_owned(), false));
        reacted.chain(seen).collect()
    }

    #[test]
    fn the_menu_line_names_one_person_and_counts_more() {
        let strings = Strings::english();
        let text = |participants, reactions, read, kind| {
            seen(
                &strings,
                &Seen {
                    participants,
                    reactions,
                    read,
                    kind,
                },
            )
        };
        assert_eq!(text(names(&[], &[]), 0, 0, SeenKind::Seen), "Nobody Viewed");
        assert_eq!(text(names(&[], &["Maria"]), 0, 1, SeenKind::Seen), "Maria");
        assert_eq!(
            text(names(&[], &["A", "B"]), 0, 2, SeenKind::Seen),
            "2 Seen"
        );
        assert_eq!(
            text(names(&[], &["A", "B"]), 0, 2, SeenKind::Listened),
            "2 Listened"
        );
        assert_eq!(
            text(names(&["A"], &["B", "C"]), 1, 3, SeenKind::Seen),
            "1/3 Reacted"
        );
        assert_eq!(
            text(names(&["A", "B"], &[]), 7, 0, SeenKind::Reacted),
            "7 Reactions"
        );
        assert_eq!(
            text(names(&["A", "B"], &[]), 2, 0, SeenKind::Reacted),
            "2 Reactions",
            "more reactions than readers counts reactions"
        );
        assert_eq!(
            text(names(&[], &[]), 0, 0, SeenKind::Reacted),
            "Nobody Reacted"
        );
        assert_eq!(
            text(names(&["A", "B"], &[]), 2, 0, SeenKind::Seen),
            "2 Reactions",
            "everyone who saw it reacted"
        );
        assert_eq!(
            text(names(&["A"], &["B", "C"]), 2, 1, SeenKind::Seen),
            "3 Seen"
        );
    }
}
