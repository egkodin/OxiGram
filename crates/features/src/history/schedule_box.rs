//! The box that picks when a message goes (`HistoryView::ScheduleBox` over
//! `Ui::ChooseDateTimeBox`): the day, which opens the calendar, "at" and
//! the time; the bell at the top makes it silent, and the dots offer
//! "Send when online".

use std::rc::Rc;

use chrono::{Datelike as _, Local, Months, NaiveDate, NaiveTime, TimeZone as _, Timelike as _};
use gpui::{
    Anchor, App, AppContext as _, Context, DismissEvent, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Pixels, Point, Render,
    Styled as _, Subscription, WeakEntity, Window, anchored, deferred, div, px,
};
use i18n::{Key, Strings, key};
use model::message::Schedule;
use model::send::{SendOptions, Sound};
use style::boxes::SCHEDULE_DATE_FIELD;
use style::choose_date_time::{
    SCHEDULE_AT_LABEL, SCHEDULE_AT_SKIP, SCHEDULE_AT_TOP, SCHEDULE_DATE_TOP, SCHEDULE_DATE_WIDTH,
    SCHEDULE_HEIGHT, SCHEDULE_TIME_WIDTH,
};
use style::info::INFO_TOP_BAR_MENU;
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::menu_icons::MENU_ICON_WHEN_ONLINE;
use style::widgets::{IconButton as IconButtonStyle, POPUP_MENU_WITH_ICONS};
use style::{ColorId, ColorRef, Icon, IconMask, Margins, MonoIcon};
use ui::{
    BoxFrame, Corrector, FlatLabel, IconButton, InputEvent, InputField, Markup, MenuTone,
    PopupMenu, RoundButton,
};

use super::calendar::{self, Calendar};
use super::send_menu::{OnOptions, SendMenu, WHEN_ONLINE};
use crate::lang::{strings, tr};
use crate::layers::Layers;
use crate::px_of;

/// The time the box offers first, from now (`DefaultScheduleTime`).
const DEFAULT_DELAY: i64 = 600;
/// The soonest a message can be scheduled for (`kMinimalSchedule`).
const MIN_DELAY: i64 = 10;
/// Where the menu keeps from the window's edges.
const EDGE_MARGIN: f32 = 8.0;

const MONTH_DAYS: [Key; 12] = [
    key::lng_month_day1,
    key::lng_month_day2,
    key::lng_month_day3,
    key::lng_month_day4,
    key::lng_month_day5,
    key::lng_month_day6,
    key::lng_month_day7,
    key::lng_month_day8,
    key::lng_month_day9,
    key::lng_month_day10,
    key::lng_month_day11,
    key::lng_month_day12,
];

/// The bell tdesktop animates in `NotifyToggleButton`, as still icons.
// shortcut: still bells with no Lottie morph and no "Recipients will be notified" tooltip; add both with Lottie playback.
macro_rules! bell {
    ($name:literal, $fg:ident) => {
        Icon::new(&[MonoIcon::new(
            IconMask::File {
                name: $name,
                modifiers: &[],
            },
            ColorRef::Palette(ColorId::$fg),
            Margins::new(0, 0, 0, 0),
        )])
    };
}

static NOTIFY_ON: IconButtonStyle = IconButtonStyle {
    icon: bell!("menu/unmute", BoxTitleCloseFg),
    icon_over: bell!("menu/unmute", BoxTitleCloseFgOver),
    icon_position: style::Point::new(-1, -1),
    ..INFO_TOP_BAR_MENU
};

static NOTIFY_OFF: IconButtonStyle = IconButtonStyle {
    icon: bell!("menu/mute", BoxTitleCloseFg),
    icon_over: bell!("menu/mute", BoxTitleCloseFgOver),
    ..NOTIFY_ON
};

/// Digits as "H:MM", the colon added once the hours are typed
/// (`TimeInput`, which tdesktop splits into two fields).
// shortcut: the cursor goes to the end after each edit; keep it in place if mid-text edits matter.
struct TimeCorrector;

impl Corrector for TimeCorrector {
    fn correct(&mut self, was: &str, _: usize, now: &mut String, now_cursor: &mut usize) {
        *now = correct_time(was, now);
        *now_cursor = now.len();
    }
}

fn correct_time(was: &str, now: &str) -> String {
    let digits = |text: &str, max| -> String {
        text.chars()
            .filter(char::is_ascii_digit)
            .take(max)
            .collect()
    };
    if let Some((hours, minutes)) = now.split_once(':') {
        return format!("{}:{}", digits(hours, 2), digits(minutes, 2));
    }
    let typed = digits(now, 4);
    // "3" can only be an hour; "1" may become "12".
    let hours = if typed.starts_with(|digit| digit > '2') {
        1
    } else {
        2
    };
    let is_typing = now.len() > was.len();
    if typed.len() > hours || (typed.len() == hours && is_typing) {
        format!("{}:{}", &typed[..hours], &typed[hours..])
    } else {
        typed
    }
}

fn parse_time(text: &str) -> Option<NaiveTime> {
    let (hours, minutes) = text.split_once(':')?;
    NaiveTime::from_hms_opt(hours.parse().ok()?, minutes.parse().ok()?, 0)
}

fn time_text(time: NaiveTime) -> String {
    format!("{}:{:02}", time.hour(), time.minute())
}

/// "October 27", with the year when it is not this one (`DayString`).
fn day_text(strings: &Strings, date: NaiveDate, today: NaiveDate) -> String {
    let month = strings.text(MONTH_DAYS[date.month0() as usize]);
    let day = date.day().to_string();
    if date.year() == today.year() {
        i18n::fill(
            strings.text(key::lng_month_day),
            &[("month", month), ("day", &day)],
        )
    } else {
        i18n::fill(
            strings.text(key::lng_month_day_year),
            &[
                ("month", month),
                ("day", &day),
                ("year", &date.year().to_string()),
            ],
        )
    }
}

/// The earliest and latest Unix times a message can go at: from ten
/// seconds ahead to a year ahead.
fn bounds(now: i64) -> (i64, i64) {
    let latest = Local
        .timestamp_opt(now, 0)
        .single()
        .and_then(|time| time.checked_add_months(Months::new(12)))
        .map_or(now, |time| time.timestamp() - 1);
    (now + MIN_DELAY, latest)
}

fn local_date(timestamp: i64) -> Option<NaiveDate> {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| time.date_naive())
}

struct OpenMenu {
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    _subscription: Subscription,
}

struct ScheduleBox {
    kind: SendMenu,
    date: NaiveDate,
    day: Entity<InputField>,
    time: Entity<InputField>,
    sound: Sound,
    on_send: OnOptions,
    layers: WeakEntity<Layers>,
    menu: Option<OpenMenu>,
    _subscription: Subscription,
}

impl Focusable for ScheduleBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.time.focus_handle(cx)
    }
}

impl ScheduleBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    /// The chosen moment, if it is a valid time between the bounds.
    fn collect(&self, cx: &App) -> Option<i64> {
        let time = parse_time(self.time.read(cx).text())?;
        let chosen = Local
            .from_local_datetime(&self.date.and_time(time))
            .earliest()?
            .timestamp();
        let (min, max) = bounds(Local::now().timestamp());
        (min..=max).contains(&chosen).then_some(chosen)
    }

    /// Ctrl or Cmd held schedules it silently too, as tdesktop's "pro tip"
    /// has it.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.collect(cx).and_then(|at| i32::try_from(at).ok()) else {
            self.time.update(cx, |time, cx| time.show_error(cx));
            return;
        };
        let is_silent = self.sound == Sound::Silent || window.modifiers().secondary(cx);
        let options = SendOptions {
            sound: if is_silent {
                Sound::Silent
            } else {
                Sound::Normal
            },
            schedule: Some(Schedule::At(at)),
        };
        self.send(options, window, cx);
    }

    fn send(&self, options: SendOptions, window: &mut Window, cx: &mut App) {
        self.close(window, cx);
        (self.on_send)(options, window, cx);
    }

    fn set_date(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.date = date;
        let text = day_text(strings(cx), date, Local::now().date_naive());
        self.day.update(cx, |day, cx| day.set_text(&text, None, cx));
    }

    /// The calendar over the box, from today to a year ahead.
    fn show_calendar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(layers) = self.layers.upgrade() else {
            return;
        };
        let (min, max) = bounds(Local::now().timestamp());
        let weak = cx.weak_entity();
        let calendar = Calendar {
            highlighted: self.date,
            min: local_date(min).unwrap_or(self.date),
            max: local_date(max).unwrap_or(self.date),
            on_chosen: Rc::new(move |date, _, cx| {
                let _ = weak.update(cx, |this, cx| this.set_date(date, cx));
            }),
        };
        calendar::show(&layers, calendar, window, cx);
    }

    /// The dots' menu: "Send when online" (`FillSendUntilOnlineMenu`).
    fn show_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            menu.action(
                tr(cx, key::lng_scheduled_send_until_online),
                MENU_ICON_WHEN_ONLINE,
                MenuTone::Normal,
                move |window, cx| {
                    let _ = weak.update(cx, |this, cx| this.send(WHEN_ONLINE, window, cx));
                },
            );
            menu
        });
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.menu = Some(OpenMenu {
            position: window.mouse_position(),
            menu,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn top_buttons(&self, frame: BoxFrame, cx: &mut Context<Self>) -> BoxFrame {
        let frame = if self.kind == SendMenu::ScheduledToUser {
            let weak = cx.weak_entity();
            frame.top_button(
                IconButton::new("schedule-menu", &INFO_TOP_BAR_MENU).on_click(
                    move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| this.show_menu(window, cx));
                    },
                ),
            )
        } else {
            frame
        };
        let notify = match self.sound {
            Sound::Normal => &NOTIFY_ON,
            Sound::Silent => &NOTIFY_OFF,
        };
        let weak = cx.weak_entity();
        frame.top_button(
            IconButton::new("schedule-notify", notify).on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.sound = match this.sound {
                        Sound::Normal => Sound::Silent,
                        Sound::Silent => Sound::Normal,
                    };
                    cx.notify();
                });
            }),
        )
    }

    fn fields(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .h(px_of(SCHEDULE_HEIGHT))
            .pt(px_of(SCHEDULE_DATE_TOP))
            .flex()
            .justify_center()
            .gap(px_of(SCHEDULE_AT_SKIP))
            .child(
                div()
                    .w(px_of(SCHEDULE_DATE_WIDTH))
                    // shortcut: a press opens the calendar, where tdesktop opens it on focus, so Tab does not; add focus if keyboard use needs it.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.show_calendar(window, cx)),
                    )
                    .child(self.day.clone()),
            )
            .child(
                div()
                    .pt(px_of(SCHEDULE_AT_TOP) - px_of(SCHEDULE_DATE_TOP))
                    .child(FlatLabel::new(
                        "schedule-at",
                        &SCHEDULE_AT_LABEL,
                        Markup::plain(tr(cx, key::lng_schedule_at).to_string()),
                    )),
            )
            .child(div().w(px_of(SCHEDULE_TIME_WIDTH)).child(self.time.clone()))
    }
}

impl Render for ScheduleBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.kind {
            SendMenu::Reminder => key::lng_remind_title,
            SendMenu::Scheduled | SendMenu::ScheduledToUser => key::lng_schedule_title,
        };
        let (submit, cancel) = (cx.entity(), cx.entity());
        let frame = BoxFrame::new("schedule-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(tr(cx, title))
            .row(self.fields(cx))
            .button(
                RoundButton::new(
                    "schedule-submit",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_schedule_button),
                )
                .on_click(move |_, window, cx| {
                    submit.update(cx, |this, cx| this.submit(window, cx));
                }),
            )
            .button(
                RoundButton::new(
                    "schedule-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    cancel.update(cx, |this, cx| this.close(window, cx));
                }),
            );
        let menu = self.menu.as_ref().map(|open| {
            deferred(
                anchored()
                    .anchor(Anchor::TopRight)
                    .position(open.position)
                    .snap_to_window_with_margin(px(EDGE_MARGIN))
                    .child(div().child(open.menu.clone())),
            )
            .with_priority(1)
        });
        div().child(self.top_buttons(frame, cx)).children(menu)
    }
}

/// Opens the box over whatever box is open; the chosen time goes to
/// `on_send`.
pub(super) fn show(
    layers: &Entity<Layers>,
    kind: SendMenu,
    on_send: OnOptions,
    window: &mut Window,
    cx: &mut App,
) {
    let now = Local::now();
    let (min, max) = bounds(now.timestamp());
    let first = (now.timestamp() + DEFAULT_DELAY).clamp(min, max);
    let first = Local.timestamp_opt(first, 0).single().unwrap_or(now);
    let date = first.date_naive();
    let layers_weak = layers.downgrade();
    let view = cx.new(|cx| {
        let day_text = day_text(strings(cx), date, now.date_naive());
        let day = cx.new(|cx| {
            let mut day = InputField::new(&SCHEDULE_DATE_FIELD, "", window, cx);
            day.set_text(&day_text, None, cx);
            day
        });
        let time = cx.new(|cx| {
            let mut time =
                InputField::new(&SCHEDULE_DATE_FIELD, "", window, cx).with_corrector(TimeCorrector);
            time.set_text(&time_text(first.time()), None, cx);
            time
        });
        let subscription = cx.subscribe_in(
            &time,
            window,
            |this: &mut ScheduleBox, _, event, window, cx| {
                if *event == InputEvent::Submitted {
                    this.submit(window, cx);
                }
            },
        );
        ScheduleBox {
            kind,
            date,
            day,
            time,
            sound: Sound::Normal,
            on_send,
            layers: layers_weak,
            menu: None,
            _subscription: subscription,
        }
    });
    layers.update(cx, |layers, cx| layers.show_above(view, window, cx));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colon_follows_the_hours() {
        assert_eq!(correct_time("", "1"), "1");
        assert_eq!(correct_time("1", "12"), "12:");
        assert_eq!(correct_time("", "9"), "9:");
        assert_eq!(correct_time("", "930"), "9:30");
        assert_eq!(correct_time("", "1234"), "12:34");
        assert_eq!(correct_time("12:34", "12:345"), "12:34");
        assert_eq!(correct_time("", "1a2:b3"), "12:3");
    }

    #[test]
    fn deleting_the_colon_keeps_the_hours() {
        assert_eq!(correct_time("12:", "12"), "12");
        assert_eq!(correct_time("12", "1"), "1");
    }

    #[test]
    fn times_parse_only_when_valid() {
        assert_eq!(parse_time("9:05"), NaiveTime::from_hms_opt(9, 5, 0));
        assert_eq!(parse_time("23:59"), NaiveTime::from_hms_opt(23, 59, 0));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("12:"), None);
        assert_eq!(parse_time("12"), None);
    }

    #[test]
    fn days_name_the_year_only_when_it_is_another() {
        let strings = Strings::english();
        let today = NaiveDate::from_ymd_opt(2026, 10, 27).expect("a valid date");
        let next = NaiveDate::from_ymd_opt(2027, 1, 3).expect("a valid date");
        assert_eq!(day_text(&strings, today, today), "October 27");
        assert_eq!(day_text(&strings, next, today), "January 3, 2027");
    }

    #[test]
    fn the_bounds_run_from_ten_seconds_to_a_year_ahead() {
        let now = Local
            .with_ymd_and_hms(2026, 10, 27, 12, 0, 0)
            .single()
            .expect("an unambiguous time")
            .timestamp();
        let (min, max) = bounds(now);
        assert_eq!(min, now + 10);
        let year = Local
            .with_ymd_and_hms(2027, 10, 27, 12, 0, 0)
            .single()
            .expect("an unambiguous time")
            .timestamp();
        assert_eq!(max, year - 1);
    }
}
