//! The time or date at the right of a chat row (`Ui::FormatDialogsDate`).

use chrono::{DateTime, Datelike as _, TimeZone};

/// Everything in the last 20 hours shows as a time.
const RECENT_SECONDS: i64 = 20 * 3600;
const WEEK_DAYS: i64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DialogDate {
    Time(String),
    /// Monday is 1, as in `lng_weekday1`.
    Weekday(u32),
    Date(String),
}

// shortcut: 24-hour time and dd.mm.yy dates; the system locale's formats come with OS integration in M7.
pub(crate) fn format<Tz: TimeZone>(unix_time: i64, now: &DateTime<Tz>) -> DialogDate {
    let Some(then) = now.timezone().timestamp_opt(unix_time, 0).single() else {
        return DialogDate::Date(String::new());
    };
    let days = (now.date_naive() - then.date_naive()).num_days().abs();
    if days == 0 || (now.timestamp() - unix_time).abs() < RECENT_SECONDS {
        DialogDate::Time(then.naive_local().format("%H:%M").to_string())
    } else if days < WEEK_DAYS {
        DialogDate::Weekday(then.weekday().number_from_monday())
    } else {
        DialogDate::Date(then.naive_local().format("%d.%m.%y").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn at(day: u32, hour: u32) -> DateTime<FixedOffset> {
        FixedOffset::east_opt(3 * 3600)
            .expect("valid offset")
            .with_ymd_and_hms(2026, 10, day, hour, 30, 0)
            .single()
            .expect("valid time")
    }

    #[test]
    fn picks_time_weekday_or_date() {
        let now = at(26, 10); // a Monday
        assert_eq!(
            format(at(26, 8).timestamp(), &now),
            DialogDate::Time("08:30".to_owned())
        );
        // Yesterday evening is within 20 hours.
        assert_eq!(
            format(at(25, 20).timestamp(), &now),
            DialogDate::Time("20:30".to_owned())
        );
        assert_eq!(format(at(23, 9).timestamp(), &now), DialogDate::Weekday(5));
        assert_eq!(
            format(at(10, 9).timestamp(), &now),
            DialogDate::Date("10.10.26".to_owned())
        );
    }
}
