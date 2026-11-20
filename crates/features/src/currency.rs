//! Prices as tdesktop shows them (`Ui::FillAmountAndCurrency`): the
//! currency's symbol, side, separators and minor units.

/// Telegram Stars (`kCreditsCurrency`).
const STARS: &str = "XTR";
const STAR: char = '\u{2B50}';
const MINUS: char = '\u{2212}';
/// No thousands separator (DKK).
const NO_SEPARATOR: char = '\0';

/// `Ui::CurrencyRule`.
#[derive(Debug, Clone, Copy)]
struct Rule {
    /// The symbol; the ISO code when empty.
    international: &'static str,
    thousands: char,
    decimal: char,
    is_left: bool,
    has_space: bool,
    exponent: u32,
    strips_dot_zero: bool,
}

const DEFAULT: Rule = Rule {
    international: "",
    thousands: ',',
    decimal: '.',
    is_left: true,
    has_space: false,
    exponent: 2,
    strips_dot_zero: false,
};

const SPACED_PREFIX: Rule = Rule {
    has_space: true,
    ..DEFAULT
};
const SPACED_SUFFIX: Rule = Rule {
    is_left: false,
    has_space: true,
    ..DEFAULT
};
const DOT_GROUPED_SUFFIX: Rule = Rule {
    thousands: '.',
    decimal: ',',
    is_left: false,
    has_space: true,
    ..DEFAULT
};
const SPACE_GROUPED_SUFFIX: Rule = Rule {
    thousands: ' ',
    decimal: ',',
    is_left: false,
    has_space: true,
    ..DEFAULT
};

/// Overrides from `CurrencyRulesMap`, sorted by code.
const RULES: &[(&str, Rule)] = &[
    ("AED", SPACED_PREFIX),
    (
        "ALL",
        Rule {
            has_space: false,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    ("AMD", SPACED_SUFFIX),
    (
        "ARS",
        Rule {
            is_left: true,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    (
        "AUD",
        Rule {
            international: "AU$",
            ..DEFAULT
        },
    ),
    ("AZN", SPACE_GROUPED_SUFFIX),
    ("BAM", DOT_GROUPED_SUFFIX),
    ("BDT", SPACED_PREFIX),
    ("BGN", SPACE_GROUPED_SUFFIX),
    (
        "BHD",
        Rule {
            exponent: 3,
            ..SPACED_PREFIX
        },
    ),
    (
        "BND",
        Rule {
            thousands: '.',
            decimal: ',',
            ..DEFAULT
        },
    ),
    (
        "BOB",
        Rule {
            is_left: true,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    (
        "BRL",
        Rule {
            international: "R$",
            is_left: true,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    ("BYN", SPACE_GROUPED_SUFFIX),
    (
        "CAD",
        Rule {
            international: "CA$",
            ..DEFAULT
        },
    ),
    (
        "CHF",
        Rule {
            thousands: '\'',
            ..SPACED_SUFFIX
        },
    ),
    (
        "CLP",
        Rule {
            is_left: true,
            exponent: 0,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    (
        "CNY",
        Rule {
            international: "CN¥",
            ..DEFAULT
        },
    ),
    (
        "COP",
        Rule {
            is_left: true,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    (
        "CRC",
        Rule {
            thousands: '.',
            decimal: ',',
            ..DEFAULT
        },
    ),
    ("CZK", SPACE_GROUPED_SUFFIX),
    (
        "DKK",
        Rule {
            thousands: '\x00',
            ..DOT_GROUPED_SUFFIX
        },
    ),
    ("DZD", SPACED_PREFIX),
    ("EGP", SPACED_PREFIX),
    (
        "EUR",
        Rule {
            international: "€",
            ..SPACE_GROUPED_SUFFIX
        },
    ),
    (
        "GBP",
        Rule {
            international: "£",
            ..DEFAULT
        },
    ),
    ("GEL", SPACE_GROUPED_SUFFIX),
    (
        "HKD",
        Rule {
            international: "HK$",
            ..DEFAULT
        },
    ),
    ("HNL", SPACED_PREFIX),
    ("HRK", DOT_GROUPED_SUFFIX),
    ("HUF", SPACE_GROUPED_SUFFIX),
    (
        "IDR",
        Rule {
            thousands: '.',
            decimal: ',',
            ..DEFAULT
        },
    ),
    (
        "ILS",
        Rule {
            international: "₪",
            ..SPACED_PREFIX
        },
    ),
    (
        "INR",
        Rule {
            international: "₹",
            ..DEFAULT
        },
    ),
    (
        "IQD",
        Rule {
            exponent: 3,
            ..SPACED_PREFIX
        },
    ),
    (
        "IRR",
        Rule {
            decimal: '/',
            ..SPACED_SUFFIX
        },
    ),
    (
        "ISK",
        Rule {
            exponent: 0,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    (
        "JOD",
        Rule {
            exponent: 3,
            ..DEFAULT
        },
    ),
    (
        "JPY",
        Rule {
            international: "¥",
            exponent: 0,
            ..DEFAULT
        },
    ),
    (
        "KGS",
        Rule {
            decimal: '-',
            ..SPACE_GROUPED_SUFFIX
        },
    ),
    (
        "KRW",
        Rule {
            international: "₩",
            exponent: 0,
            ..DEFAULT
        },
    ),
    (
        "KZT",
        Rule {
            thousands: ' ',
            decimal: '-',
            ..DEFAULT
        },
    ),
    ("LBP", SPACED_PREFIX),
    ("LKR", SPACED_PREFIX),
    ("MAD", SPACED_PREFIX),
    ("MDL", SPACED_SUFFIX),
    (
        "MNT",
        Rule {
            thousands: ' ',
            decimal: ',',
            ..DEFAULT
        },
    ),
    ("MVR", SPACED_SUFFIX),
    (
        "MXN",
        Rule {
            international: "MX$",
            ..DEFAULT
        },
    ),
    ("NIO", SPACED_PREFIX),
    (
        "NOK",
        Rule {
            is_left: true,
            ..SPACE_GROUPED_SUFFIX
        },
    ),
    (
        "NZD",
        Rule {
            international: "NZ$",
            ..DEFAULT
        },
    ),
    ("PAB", SPACED_PREFIX),
    ("PEN", SPACED_PREFIX),
    ("PLN", SPACE_GROUPED_SUFFIX),
    (
        "PYG",
        Rule {
            is_left: true,
            exponent: 0,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    ("QAR", SPACED_PREFIX),
    ("RON", DOT_GROUPED_SUFFIX),
    ("RSD", DOT_GROUPED_SUFFIX),
    ("RUB", SPACE_GROUPED_SUFFIX),
    ("SAR", SPACED_PREFIX),
    ("SEK", DOT_GROUPED_SUFFIX),
    ("SYP", SPACED_PREFIX),
    (
        "THB",
        Rule {
            international: "฿",
            ..DEFAULT
        },
    ),
    (
        "TJS",
        Rule {
            decimal: ';',
            ..SPACE_GROUPED_SUFFIX
        },
    ),
    ("TRY", DOT_GROUPED_SUFFIX),
    (
        "TWD",
        Rule {
            international: "NT$",
            ..DEFAULT
        },
    ),
    (
        "UAH",
        Rule {
            has_space: false,
            ..SPACE_GROUPED_SUFFIX
        },
    ),
    (
        "UGX",
        Rule {
            exponent: 0,
            ..DEFAULT
        },
    ),
    (
        "USD",
        Rule {
            international: "$",
            ..DEFAULT
        },
    ),
    (
        "UYU",
        Rule {
            is_left: true,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    ("UZS", SPACE_GROUPED_SUFFIX),
    (
        "VEF",
        Rule {
            is_left: true,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    (
        "VND",
        Rule {
            international: "₫",
            exponent: 0,
            ..DOT_GROUPED_SUFFIX
        },
    ),
    ("YER", SPACED_PREFIX),
    ("ZAR", SPACED_PREFIX),
];

fn rule(currency: &str) -> Rule {
    RULES
        .binary_search_by(|(code, _)| (*code).cmp(currency))
        .map_or(DEFAULT, |index| RULES[index].1)
}

/// `amount` in the currency's minor units, as "$4.99" or "4,99 €".
pub(crate) fn format(amount: i64, currency: &str) -> String {
    let magnitude = amount.unsigned_abs();
    if currency == STARS {
        return format!(
            "{STAR}{}",
            crate::lang::group_unsigned(magnitude, Some(','))
        );
    }
    let rule = rule(currency);
    let scale = 10_u64.pow(rule.exponent);
    let fraction = magnitude % scale;
    let separator = (rule.thousands != NO_SEPARATOR).then_some(rule.thousands);
    let mut number = crate::lang::group_unsigned(magnitude / scale, separator);
    if rule.exponent != 0 && (!rule.strips_dot_zero || fraction != 0) {
        number.push(rule.decimal);
        number.push_str(&format!(
            "{fraction:0width$}",
            width = rule.exponent as usize
        ));
    }
    let name = if rule.international.is_empty() {
        currency
    } else {
        rule.international
    };
    let space = if rule.has_space { " " } else { "" };
    let sign = if amount < 0 {
        MINUS.to_string()
    } else {
        String::new()
    };
    if rule.is_left {
        format!("{sign}{name}{space}{number}")
    } else {
        format!("{sign}{number}{space}{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_prices_preserve_minor_units() {
        assert_eq!(format(i64::MAX, "USD"), "$92,233,720,368,547,758.07");
        assert_eq!(format(i64::MIN, "USD"), "−$92,233,720,368,547,758.08");
        assert_eq!(format(i64::MIN, "XTR"), "⭐9,223,372,036,854,775,808");
    }

    #[test]
    fn prices_follow_tdesktops_rules() {
        assert!(RULES.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert_eq!(format(499, "USD"), "$4.99");
        assert_eq!(format(123_456_789, "USD"), "$1,234,567.89");
        assert_eq!(format(399, "EUR"), "3,99 €");
        assert_eq!(format(-1500, "JPY"), "\u{2212}¥1,500");
        assert_eq!(format(123_456, "DKK"), "1234,56 DKK");
        assert_eq!(format(250, "ZZZ"), "ZZZ2.50");
        assert_eq!(format(1500, "XTR"), "\u{2B50}1,500");
    }
}
