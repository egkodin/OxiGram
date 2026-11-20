//! The Premium page's subscription periods (`Ui::Premium::AddGiftOptions`
//! with `SubscriptionOptionsForRows`): a radio, the period with its
//! discount, the yearly total and the monthly price.
// shortcut: the radios and badges take one flat colour instead of the gift gradient; with M8 polish.

use gpui::{
    App, InteractiveElement as _, IntoElement, ParentElement, StatefulInteractiveElement as _,
    Styled, div, px,
};
use i18n::key;
use model::premium::SubscriptionOption;
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::layers::DEFAULT_BOX_CHECKBOX;
use style::premium::PREMIUM_SUBSCRIPTION_OPTION;
use style::widgets::DEFAULT_RADIO;
use style::{ColorId, ColorRef, Font, FontFlags};
use ui::{Checkbox, Checked, color, font, font_height};

use crate::currency;
use crate::lang::{tr, tr_plural, tr_with};
use crate::px_of;

/// `windowFiltersButton.badgeStyle.font`.
const BADGE_FONT: Font = Font::new(10, FontFlags::BOLD, "");
const BADGE_BG: ColorRef = ColorRef::Palette(ColorId::PremiumButtonBg2);
const BADGE_FG: ColorRef = ColorRef::Palette(ColorId::PremiumButtonFg);
const TITLE_FG: ColorRef = ColorRef::Palette(ColorId::BoxTextFg);
const PRICE_FG: ColorRef = ColorRef::Palette(ColorId::WindowSubTextFg);
/// U+2212, as `CreateSubscriptionOption` writes the discount.
const MINUS: char = '\u{2212}';

/// What a row shows (`PremiumSubscriptionOption` after
/// `SubscriptionOptionsForRows`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OptionTexts {
    pub title: String,
    /// "−20%"; empty without a discount.
    pub discount: String,
    /// The undiscounted total, struck out; empty without a discount.
    pub full_price: String,
    /// "$39.99 per year"; empty for the monthly option.
    pub per_year: String,
    /// "$3.33 / month".
    pub per_month_label: String,
    /// "$3.33", for the button.
    pub per_month: String,
}

/// `PremiumSubscriptionOptionsFromTL`: the discount is measured against
/// the currency's cheapest total over its months.
pub(super) fn texts(options: &[SubscriptionOption], cx: &App) -> Vec<OptionTexts> {
    options
        .iter()
        .map(|option| {
            let base = options
                .iter()
                .filter(|other| other.currency == option.currency && other.months > 0)
                .min_by_key(|other| other.amount)
                .unwrap_or(option);
            option_texts(option, base, cx)
        })
        .collect()
}

/// The money a row shows, before the words go around it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Prices {
    /// Rounded percent saved against `base`.
    discount: i64,
    full: String,
    per_year: String,
    per_month: String,
}

fn prices(option: &SubscriptionOption, base: &SubscriptionOption) -> Prices {
    let months = i128::from(option.months.max(1));
    let base_months = i128::from(base.months.max(1));
    let amount = i128::from(option.amount);
    let baseline = i128::from(base.amount) * months;
    let money = |n, d| currency::format(rounded_ratio(n, d), &option.currency);
    Prices {
        discount: if baseline > 0 {
            rounded_ratio(100 * (baseline - amount * base_months), baseline)
        } else {
            0
        },
        full: money(baseline, base_months),
        per_year: money(amount * 12, months),
        per_month: money(amount, months),
    }
}

/// Rounds a price ratio with a positive denominator to the nearest minor unit.
fn rounded_ratio(numerator: i128, denominator: i128) -> i64 {
    let rounded = (numerator + numerator.signum() * (denominator / 2)) / denominator;
    i64::try_from(rounded).unwrap_or(if rounded < 0 { i64::MIN } else { i64::MAX })
}

fn option_texts(option: &SubscriptionOption, base: &SubscriptionOption, cx: &App) -> OptionTexts {
    let prices = prices(option, base);
    let is_monthly = option.months == 1;
    let has_discount = prices.discount > 0;
    OptionTexts {
        title: period(option.months, cx),
        discount: if has_discount {
            format!("{MINUS}{}%", prices.discount)
        } else {
            String::new()
        },
        full_price: if has_discount && !is_monthly {
            prices.full
        } else {
            String::new()
        },
        per_year: if is_monthly {
            String::new()
        } else {
            tr_with(
                cx,
                key::lng_premium_subscribe_total,
                &[("cost", &prices.per_year)],
            )
        },
        per_month_label: tr_with(
            cx,
            key::lng_premium_gift_per,
            &[("cost", &prices.per_month)],
        ),
        per_month: prices.per_month,
    }
}

fn period(months: i32, cx: &App) -> String {
    let named = match months {
        1 => key::lng_premium_subscribe_months_1,
        6 => key::lng_premium_subscribe_months_6,
        12 => key::lng_premium_subscribe_months_12,
        24 => key::lng_premium_subscribe_months_24,
        _ => return tr_plural(cx, key::lng_months, i64::from(months), &[]),
    };
    tr(cx, named).to_string()
}

fn text(value: String, st: Font, fg: ColorRef, cx: &App) -> gpui::Div {
    div()
        .font(font(st))
        .text_size(px_of(st.size))
        .line_height(font_height(st))
        .text_color(color(cx, fg))
        .whitespace_nowrap()
        .child(value)
}

/// One period's row; `on_choose` picks it.
pub(super) fn row(
    index: usize,
    texts: &OptionTexts,
    is_chosen: bool,
    on_choose: impl Fn(&mut App) + Clone + 'static,
    cx: &App,
) -> impl IntoElement {
    let st = &PREMIUM_SUBSCRIPTION_OPTION;
    let checked = if is_chosen { Checked::Yes } else { Checked::No };
    let badge = (!texts.discount.is_empty()).then(|| {
        div()
            .ml(px_of(st.badge_shift.x))
            .px(px_of(st.badge_margins.left))
            .h(px_of(st.badge_height))
            .flex()
            .items_center()
            .rounded(px_of(st.badge_radius))
            .bg(color(cx, BADGE_BG))
            .child(text(texts.discount.clone(), BADGE_FONT, BADGE_FG, cx))
    });
    let full_price = (!texts.full_price.is_empty())
        .then(|| text(texts.full_price.clone(), NORMAL_FONT, PRICE_FG, cx).line_through());
    let per_year = (!texts.per_year.is_empty()).then(|| {
        div().flex().gap(px(4.0)).children(full_price).child(text(
            texts.per_year.clone(),
            NORMAL_FONT,
            PRICE_FG,
            cx,
        ))
    });
    let radio_choose = on_choose.clone();
    div()
        .id(("premium-option", index))
        .flex()
        .items_center()
        .h(px_of(st.row_height))
        .ml(px_of(st.row_padding.left))
        .mr(px_of(st.row_padding.right))
        .mt(px_of(st.row_padding.top))
        .mb(px_of(st.row_padding.bottom))
        .cursor_pointer()
        .on_click(move |_, _, cx| on_choose(cx))
        .child(
            div()
                .flex_none()
                .w(px_of(st.text_left))
                .pl(px_of(st.row_margins.left))
                .child(
                    Checkbox::radio(
                        ("premium-option-radio", index),
                        &DEFAULT_BOX_CHECKBOX,
                        &DEFAULT_RADIO,
                        "",
                        checked,
                    )
                    .on_toggle(move |_, cx| radio_choose(cx)),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(text(texts.title.clone(), SEMIBOLD_FONT, TITLE_FG, cx))
                        .children(badge),
                )
                .children(per_year),
        )
        .child(div().pr(px_of(st.row_margins.right)).child(text(
            texts.per_month_label.clone(),
            NORMAL_FONT,
            PRICE_FG,
            cx,
        )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::premium::PaymentLink;

    fn option(months: i32, amount: i64) -> SubscriptionOption {
        SubscriptionOption {
            currency: "USD".to_owned(),
            amount,
            months,
            link: PaymentLink::Other,
        }
    }

    #[test]
    fn prices_retain_minor_units_and_handle_a_zero_baseline() {
        let monthly = option(1, i64::MAX);
        let result = prices(&monthly, &option(1, 0));
        assert_eq!(result.per_month, "$92,233,720,368,547,758.07");
        assert_eq!(result.discount, 0);
        assert_eq!(prices(&option(2, 5), &monthly).per_month, "$0.03");
        assert_eq!(rounded_ratio(-5, 2), -3);
    }

    #[test]
    fn yearly_price_is_measured_against_the_monthly_one() {
        let monthly = option(1, 499);
        let yearly = prices(&option(12, 3999), &monthly);
        assert_eq!(yearly.discount, 33);
        assert_eq!(yearly.full, "$59.88");
        assert_eq!(yearly.per_year, "$39.99");
        assert_eq!(yearly.per_month, "$3.33");
        let same = prices(&monthly, &monthly);
        assert_eq!((same.discount, same.per_month.as_str()), (0, "$4.99"));
    }
}
