//! The calendar a date badge opens (`CalendarBox`): a month of days, and a
//! click on one jumps to its first message (`showCalendar`).

use std::rc::Rc;

use chrono::{Datelike as _, Days, Local, Months, NaiveDate, TimeZone as _};
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, Render, StatefulInteractiveElement as _, Styled, WeakEntity,
    Window, div, prelude::FluentBuilder as _,
};
use i18n::{Key, Strings, key};
use style::basic::NORMAL_FONT;
use style::boxes::{
    CALENDAR_NEXT, CALENDAR_NEXT_DISABLED, CALENDAR_PREVIOUS, CALENDAR_PREVIOUS_DISABLED,
    CALENDAR_TITLE_HEIGHT, DEFAULT_CALENDAR_SIZES,
};
use style::layers::{BOX_TITLE_FONT, DEFAULT_BOX};
use style::widgets::IconButton as IconButtonStyle;
use style::{ColorId, ColorRef, Icon};
use ui::{BoxFrame, IconButton, RoundButton, color, font, font_height, icon};

use super::HistoryView;
use crate::lang::{strings, tr};
use crate::layers::{Dismiss, Layers};
use crate::px_of;
use crate::store::spawn;

const DAYS_IN_WEEK: u32 = 7;
/// Telegram started in August 2013, so no chat is older.
const TELEGRAM_START: (i32, u32, u32) = (2013, 8, 1);

const WEEKDAYS: [Key; 7] = [
    key::lng_weekday1,
    key::lng_weekday2,
    key::lng_weekday3,
    key::lng_weekday4,
    key::lng_weekday5,
    key::lng_weekday6,
    key::lng_weekday7,
];
const MONTHS: [Key; 12] = [
    key::lng_month1,
    key::lng_month2,
    key::lng_month3,
    key::lng_month4,
    key::lng_month5,
    key::lng_month6,
    key::lng_month7,
    key::lng_month8,
    key::lng_month9,
    key::lng_month10,
    key::lng_month11,
    key::lng_month12,
];

/// What a chosen day does.
type OnChosen = Rc<dyn Fn(NaiveDate, &mut Window, &mut App)>;

/// The first day of `date`'s month.
fn month_of(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

/// `month` moved by `delta` months, if the calendar has it.
fn shifted(month: NaiveDate, delta: i32) -> Option<NaiveDate> {
    let by = Months::new(delta.unsigned_abs());
    if delta < 0 {
        month.checked_sub_months(by)
    } else {
        month.checked_add_months(by)
    }
}

/// The days the grid shows for `month`: whole weeks from Monday, the
/// neighbouring months' days at either end.
// shortcut: weeks start on Monday and one month shows at a time; tdesktop follows the locale's first weekday and scrolls through every month.
fn grid(month: NaiveDate) -> Vec<NaiveDate> {
    let first = month_of(month);
    let shift = first.weekday().num_days_from_monday();
    let next = first + Months::new(1);
    let days = u32::try_from((next - first).num_days()).unwrap_or(0);
    let cells = (shift + days).div_ceil(DAYS_IN_WEEK) * DAYS_IN_WEEK;
    let start = first - Days::new(u64::from(shift));
    (0..cells)
        .map(|cell| start + Days::new(u64::from(cell)))
        .collect()
}

/// "October 2026" (`lng_month_year`).
fn title(strings: &Strings, month: NaiveDate) -> String {
    let name = MONTHS[month.month0() as usize];
    i18n::fill(
        strings.text(key::lng_month_year),
        &[
            ("month", strings.text(name)),
            ("year", &month.year().to_string()),
        ],
    )
}

/// What the calendar shows and does.
pub(super) struct Calendar {
    pub highlighted: NaiveDate,
    pub min: NaiveDate,
    pub max: NaiveDate,
    pub on_chosen: OnChosen,
}

struct CalendarBox {
    month: NaiveDate,
    calendar: Calendar,
    layers: WeakEntity<Layers>,
    focus: FocusHandle,
}

impl Focusable for CalendarBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CalendarBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    /// The month `delta` away, unless it is past the chat's first or last day.
    fn target(&self, delta: i32) -> Option<NaiveDate> {
        let range = month_of(self.calendar.min)..=month_of(self.calendar.max);
        shifted(self.month, delta).filter(|month| range.contains(month))
    }

    fn arrow(
        &self,
        id: &'static str,
        delta: i32,
        (enabled, disabled): (&'static IconButtonStyle, Icon),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.target(delta).is_none() {
            return disabled_arrow(enabled, disabled);
        }
        let weak = cx.weak_entity();
        IconButton::new(id, enabled)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if let Some(month) = this.target(delta) {
                        this.month = month;
                        cx.notify();
                    }
                });
            })
            .into_any_element()
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let st = &DEFAULT_CALENDAR_SIZES;
        let cell = px_of(st.cell_size.width);
        let days_fg = color(cx, ColorRef::Palette(ColorId::BoxTitleAdditionalFg));
        let title_row = div()
            .h(px_of(CALENDAR_TITLE_HEIGHT))
            .flex()
            .items_center()
            .pl(px_of(st.padding.left) + (cell - px_of(st.cell_inner)) / 2.0)
            .pr(px_of(st.padding.right))
            .child(
                div()
                    .flex_1()
                    .font(font(BOX_TITLE_FONT))
                    .text_size(px_of(BOX_TITLE_FONT.size))
                    .line_height(font_height(BOX_TITLE_FONT))
                    .text_color(color(cx, ColorRef::Palette(ColorId::BoxTitleFg)))
                    .child(title(strings(cx), self.month)),
            )
            .child(self.arrow(
                "calendar-previous",
                -1,
                (&CALENDAR_PREVIOUS, CALENDAR_PREVIOUS_DISABLED),
                cx,
            ))
            .child(self.arrow(
                "calendar-next",
                1,
                (&CALENDAR_NEXT, CALENDAR_NEXT_DISABLED),
                cx,
            ));
        let days = div()
            .h(px_of(st.days_height))
            .flex()
            .pl(px_of(st.padding.left))
            .font(font(NORMAL_FONT))
            .text_size(px_of(NORMAL_FONT.size))
            .line_height(font_height(NORMAL_FONT))
            .text_color(days_fg)
            .children(WEEKDAYS.map(|day| div().w(cell).text_center().child(tr(cx, day))));
        div().child(title_row).child(days)
    }

    fn day(&self, date: NaiveDate, cx: &mut Context<Self>) -> AnyElement {
        let st = &DEFAULT_CALENDAR_SIZES;
        let calendar = &self.calendar;
        let is_grayed = month_of(date) != self.month;
        let is_enabled = (calendar.min..=calendar.max).contains(&date);
        let is_highlighted = date == calendar.highlighted;
        let palette = |id| color(cx, ColorRef::Palette(id));
        let fg = match (is_highlighted, is_enabled, is_grayed) {
            (_, _, true) => palette(ColorId::WindowSubTextFg),
            (true, _, false) => palette(ColorId::DialogsNameFgActive),
            (false, true, false) => palette(ColorId::BoxTextFg),
            (false, false, false) => palette(ColorId::WindowSubTextFg),
        };
        let inner = px_of(st.cell_inner);
        let circle = div()
            .id(("calendar-day", date.num_days_from_ce().unsigned_abs()))
            .size(inner)
            .rounded(inner / 2.0)
            .flex()
            .items_center()
            .justify_center()
            .text_color(fg)
            .when(is_highlighted, |this| {
                this.bg(palette(if is_grayed {
                    ColorId::WindowBgOver
                } else {
                    ColorId::DialogsBgActive
                }))
            })
            .when(is_enabled, |this| {
                let weak = cx.weak_entity();
                this.cursor_pointer()
                    .when(!is_highlighted, |this| {
                        this.hover(|style| style.bg(palette(ColorId::WindowBgOver)))
                    })
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.choose(date, window, cx));
                    })
            })
            .child(date.day().to_string());
        div()
            .w(px_of(st.cell_size.width))
            .h(px_of(st.cell_size.height))
            .flex()
            .items_center()
            .justify_center()
            .child(circle)
            .into_any_element()
    }

    fn choose(&mut self, date: NaiveDate, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        (self.calendar.on_chosen)(date, window, cx);
    }
}

impl Render for CalendarBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &DEFAULT_CALENDAR_SIZES;
        let days: Vec<_> = grid(self.month)
            .into_iter()
            .map(|date| self.day(date, cx))
            .collect();
        let rows = div()
            .flex()
            .flex_wrap()
            .pl(px_of(st.padding.left))
            .pr(px_of(st.padding.right))
            .font(font(NORMAL_FONT))
            .text_size(px_of(NORMAL_FONT.size))
            .line_height(font_height(NORMAL_FONT))
            .children(days);
        let entity = cx.entity();
        let close = RoundButton::new(
            "calendar-close",
            &DEFAULT_BOX.button,
            tr(cx, key::lng_close),
        )
        .on_click(move |_, window, cx| entity.update(cx, |this, cx| this.close(window, cx)));
        div().track_focus(&self.focus).child(
            BoxFrame::new("calendar-box", &DEFAULT_BOX)
                .width(px_of(st.width))
                .row(self.header(cx))
                .row(rows)
                .button(close),
        )
    }
}

/// Shows the calendar; Escape and a click outside close it.
/// An arrow that cannot step further: its dimmed icon in the button's place.
pub(super) fn disabled_arrow(enabled: &IconButtonStyle, disabled: Icon) -> AnyElement {
    div()
        .flex_none()
        .size(px_of(enabled.width))
        .flex()
        .items_center()
        .justify_center()
        .child(icon(disabled))
        .into_any_element()
}

pub(super) fn show(
    layers: &gpui::Entity<Layers>,
    calendar: Calendar,
    window: &mut Window,
    cx: &mut App,
) {
    let weak = layers.downgrade();
    let view = cx.new(|cx| CalendarBox {
        month: month_of(calendar.highlighted),
        calendar,
        layers: weak,
        focus: cx.focus_handle(),
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx);
    });
}

/// The Unix time `date` starts at here.
fn day_start(date: NaiveDate) -> Option<i64> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|time| time.timestamp())
}

impl HistoryView {
    /// Opens the calendar at `day`, from the chat's first day to today.
    pub(super) fn show_calendar(
        &mut self,
        day: NaiveDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (year, month, first_day) = TELEGRAM_START;
        let start = NaiveDate::from_ymd_opt(year, month, first_day).unwrap_or(day);
        let first_loaded = self
            .store
            .read(cx)
            .state()
            .history(self.chat)
            .filter(|history| history.older_page().is_none())
            .and_then(|history| history.messages().first())
            .and_then(|message| super::layout::day(message.date, &Local));
        let today = Local::now().date_naive();
        let weak = cx.weak_entity();
        let calendar = Calendar {
            highlighted: day,
            min: first_loaded.unwrap_or(start).min(day),
            max: today.max(day),
            on_chosen: Rc::new(move |date, _, cx| {
                let _ = weak.update(cx, |this, cx| this.open_day(date, cx));
            }),
        };
        show(&self.layers, calendar, window, cx);
    }

    /// Jumps to the first message of `date` (`resolveJumpToDate`).
    fn open_day(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        let Some(start) = day_start(date) else {
            return;
        };
        let task = spawn(
            self.store
                .read(cx)
                .session()
                .message_before(self.chat, start),
            cx,
        );
        cx.spawn(async move |this, cx| match task.await {
            Ok(before) => {
                let _ = this.update(cx, |this, cx| this.jump_after(before, cx));
            }
            Err(error) => tracing::warn!(%error, "finding the day's first message failed"),
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a valid test date")
    }

    #[test]
    fn the_grid_covers_whole_weeks_from_monday() {
        // October 2026 starts on a Thursday and ends on a Saturday.
        let days = grid(date(2026, 10, 15));
        assert_eq!(days.len(), 35);
        assert_eq!(days.first(), Some(&date(2026, 9, 28)));
        assert_eq!(days.last(), Some(&date(2026, 11, 1)));
        // February 2027 starts on a Monday: four rows exactly.
        assert_eq!(grid(date(2027, 2, 1)).len(), 28);
    }

    #[test]
    fn the_title_names_the_month_and_year() {
        assert_eq!(
            title(&Strings::english(), date(2026, 10, 3)),
            "October 2026"
        );
    }

    #[test]
    fn months_shift_across_years() {
        assert_eq!(shifted(date(2026, 1, 1), -1), Some(date(2025, 12, 1)));
        assert_eq!(shifted(date(2026, 12, 1), 1), Some(date(2027, 1, 1)));
    }
}
