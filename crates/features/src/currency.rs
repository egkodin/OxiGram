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

/// `CurrencyRulesMap`, sorted by code.
const RULES: &[(&str, Rule)] = &[
    (
        "AED",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "AFN",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "ALL",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "AMD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "ARS",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "AUD",
        Rule {
            international: "AU$",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "AZN",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "BAM",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "BDT",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "BGN",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "BHD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 3,
            strips_dot_zero: false,
        },
    ),
    (
        "BND",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "BOB",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "BRL",
        Rule {
            international: "R$",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "BYN",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "CAD",
        Rule {
            international: "CA$",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "CHF",
        Rule {
            international: "",
            thousands: '\'',
            decimal: '.',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "CLP",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 0,
            strips_dot_zero: false,
        },
    ),
    (
        "CNY",
        Rule {
            international: "CN¥",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "COP",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "CRC",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "CZK",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "DKK",
        Rule {
            international: "",
            thousands: '\x00',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "DOP",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "DZD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "EGP",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "ETB",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "EUR",
        Rule {
            international: "€",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "GBP",
        Rule {
            international: "£",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "GEL",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "GHS",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "GTQ",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "HKD",
        Rule {
            international: "HK$",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "HNL",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "HRK",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "HUF",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "IDR",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "ILS",
        Rule {
            international: "₪",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "INR",
        Rule {
            international: "₹",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "IQD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 3,
            strips_dot_zero: false,
        },
    ),
    (
        "IRR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '/',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "ISK",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 0,
            strips_dot_zero: false,
        },
    ),
    (
        "JMD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "JOD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 3,
            strips_dot_zero: false,
        },
    ),
    (
        "JPY",
        Rule {
            international: "¥",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 0,
            strips_dot_zero: false,
        },
    ),
    (
        "KES",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "KGS",
        Rule {
            international: "",
            thousands: ' ',
            decimal: '-',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "KRW",
        Rule {
            international: "₩",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 0,
            strips_dot_zero: false,
        },
    ),
    (
        "KZT",
        Rule {
            international: "",
            thousands: ' ',
            decimal: '-',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "LBP",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "LKR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MAD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MDL",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MMK",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MNT",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MOP",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MUR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MVR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MXN",
        Rule {
            international: "MX$",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MYR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "MZN",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "NGN",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "NIO",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "NOK",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "NPR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "NZD",
        Rule {
            international: "NZ$",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "PAB",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "PEN",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "PHP",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "PKR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "PLN",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "PYG",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 0,
            strips_dot_zero: false,
        },
    ),
    (
        "QAR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "RON",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "RSD",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "RUB",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "SAR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "SEK",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "SGD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "SYP",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "THB",
        Rule {
            international: "฿",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "TJS",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ';',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "TRY",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "TTD",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "TWD",
        Rule {
            international: "NT$",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "TZS",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "UAH",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "UGX",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 0,
            strips_dot_zero: false,
        },
    ),
    (
        "USD",
        Rule {
            international: "$",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: false,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "UYU",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "UZS",
        Rule {
            international: "",
            thousands: ' ',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "VEF",
        Rule {
            international: "",
            thousands: '.',
            decimal: ',',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "VND",
        Rule {
            international: "₫",
            thousands: '.',
            decimal: ',',
            is_left: false,
            has_space: true,
            exponent: 0,
            strips_dot_zero: false,
        },
    ),
    (
        "YER",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
    (
        "ZAR",
        Rule {
            international: "",
            thousands: ',',
            decimal: '.',
            is_left: true,
            has_space: true,
            exponent: 2,
            strips_dot_zero: false,
        },
    ),
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
