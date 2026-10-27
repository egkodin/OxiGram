//! File sizes as tdesktop writes them (`FormatSizeText`,
//! `FormatDownloadText`).

use i18n::{Strings, key};

const KB: i64 = 1024;
const MB: i64 = 1024 * 1024;

/// "3.4 MB", "12.5 KB", "300 B".
pub(crate) fn size_text(size: i64) -> String {
    if size >= MB {
        let tenths = size * 10 / MB;
        format!("{}.{} MB", tenths / 10, tenths % 10)
    } else if size >= KB {
        let tenths = size * 10 / KB;
        format!("{}.{} KB", tenths / 10, tenths % 10)
    } else {
        format!("{size} B")
    }
}

/// "1.2 / 3.4 MB", in the total's unit.
pub(crate) fn progress_text(strings: &Strings, ready: i64, total: i64) -> String {
    let (ready, total, unit) = if total >= MB {
        let tenths = |bytes: i64| {
            let tenths = bytes * 10 / MB;
            format!("{}.{}", tenths / 10, tenths % 10)
        };
        (tenths(ready), tenths(total), "MB")
    } else if total >= KB {
        ((ready / KB).to_string(), (total / KB).to_string(), "KB")
    } else {
        (ready.to_string(), total.to_string(), "B")
    };
    i18n::fill(
        strings.text(key::lng_save_downloaded),
        &[("ready", &ready), ("total", &total), ("mb", unit)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_take_tdesktops_units() {
        assert_eq!(size_text(300), "300 B");
        assert_eq!(size_text(12_800), "12.5 KB");
        assert_eq!(size_text(3_565_158), "3.3 MB");
    }

    #[test]
    fn progress_follows_the_totals_unit() {
        let strings = Strings::english();
        assert_eq!(
            progress_text(&strings, 1_258_292, 3_565_158),
            "1.2 / 3.3 MB"
        );
        // Tenths are cut, not rounded.
        assert_eq!(
            progress_text(&strings, 1_258_291, 3_565_158),
            "1.1 / 3.3 MB"
        );
        assert_eq!(progress_text(&strings, 2048, 10_240), "2 / 10 KB");
    }
}
