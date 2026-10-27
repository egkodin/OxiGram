//! The day that floats over the history's top while it scrolls
//! (`HistoryInner::scrollDateCheck`).

use std::time::Duration;

use chrono::{Local, NaiveDate};
use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, ElementId, IntoElement, ParentElement as _,
    Pixels, Styled, div,
};
use model::MessageId;

use super::{HistoryView, bubble, layout};

/// Shown this long after the last scroll up (`kScrollDateHideTimeout`).
const HIDE_TIMEOUT: Duration = Duration::from_millis(1000);
/// Shown this long after scrolling down into another day.
const DAY_CROSSING_TIMEOUT: Duration = Duration::from_millis(3000);
/// `historyDateFadeDuration`.
const FADE: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Visibility {
    #[default]
    Hidden,
    Shown,
    Hiding,
}

#[derive(Debug, Default)]
pub(super) struct ScrollDate {
    /// The top message and how far into it the list is scrolled.
    last: Option<(MessageId, Pixels)>,
    last_day: Option<NaiveDate>,
    visibility: Visibility,
    /// Scrolling down keeps the date shown after it crossed into a day.
    is_after_day_crossing: bool,
    /// Counts shows, so each one fades in once.
    shows: u64,
    /// Counts timers, so only the latest one hides the date.
    timers: u64,
}

impl HistoryView {
    /// Scrolling up shows the top message's day; scrolling down shows it
    /// only on entering another day.
    pub(super) fn on_scroll_date(&mut self, cx: &mut Context<Self>) {
        let top = self.list.logical_scroll_top();
        let Some(message) = self.shown.get(top.item_ix) else {
            self.hide_date(cx);
            return;
        };
        let current = (message.id, top.offset_in_item);
        let day = layout::day(message.date, &Local);
        let state = &mut self.scroll_date;
        let previous_day = std::mem::replace(&mut state.last_day, day);
        let previous = state.last.replace(current);
        if previous == Some(current) {
            return;
        }
        // Chats open at the bottom, so the first scroll goes up.
        let is_up = previous.is_none_or(|previous| current < previous);
        if is_up {
            state.is_after_day_crossing = false;
            self.show_date(HIDE_TIMEOUT, cx);
        } else if previous_day.is_some() && day.is_some() && previous_day != day {
            state.is_after_day_crossing = true;
            self.show_date(DAY_CROSSING_TIMEOUT, cx);
        } else if !state.is_after_day_crossing {
            self.hide_date(cx);
        }
    }

    fn show_date(&mut self, timeout: Duration, cx: &mut Context<Self>) {
        let state = &mut self.scroll_date;
        if state.visibility != Visibility::Shown {
            state.visibility = Visibility::Shown;
            state.shows += 1;
            cx.notify();
        }
        state.timers += 1;
        let timer = state.timers;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(timeout).await;
            let _ = this.update(cx, |this, cx| {
                if this.scroll_date.timers == timer {
                    this.scroll_date.is_after_day_crossing = false;
                    this.hide_date(cx);
                }
            });
        })
        .detach();
    }

    fn hide_date(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.scroll_date;
        if state.visibility != Visibility::Shown {
            return;
        }
        state.visibility = Visibility::Hiding;
        state.timers += 1;
        let timer = state.timers;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FADE).await;
            let _ = this.update(cx, |this, cx| {
                if this.scroll_date.timers == timer {
                    this.scroll_date.visibility = Visibility::Hidden;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The floating day, unless the top message's own date badge is there.
    // shortcut: the next day's badge slides under the floating one instead of pushing it up; push with the history rewrite to a custom list in M8.
    pub(super) fn floating_date(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = &self.scroll_date;
        let is_shown = match state.visibility {
            Visibility::Hidden => return None,
            Visibility::Shown => true,
            Visibility::Hiding => false,
        };
        let top = self.list.logical_scroll_top();
        let message = self.shown.get(top.item_ix)?;
        let day = layout::day(message.date, &Local)?;
        let previous_day = top
            .item_ix
            .checked_sub(1)
            .and_then(|index| self.shown.get(index))
            .and_then(|previous| layout::day(previous.date, &Local));
        if previous_day != Some(day) && top.offset_in_item < bubble::date_row_height() {
            return None;
        }
        let badge = self.date_badge(message.id, day, &Local::now(), cx);
        let id = ElementId::NamedInteger(
            if is_shown { "date-in" } else { "date-out" }.into(),
            state.shows,
        );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .child(bubble::date_pill(badge, cx))
                .with_animation(id, Animation::new(FADE), move |this, delta| {
                    this.opacity(if is_shown { delta } else { 1.0 - delta })
                })
                .into_any_element(),
        )
    }
}
