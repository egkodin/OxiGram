//! Search in a chat (`HistoryView::ComposeSearch`): a field over the
//! history, and under it the place among the results with arrows that
//! jump to the next and previous one.

use std::time::Duration;

use chrono::Local;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled as _,
    Subscription, Window, div,
};
use i18n::{Strings, key};
use model::MessageId;
use style::boxes::{
    CALENDAR_NEXT, CALENDAR_NEXT_DISABLED, CALENDAR_PREVIOUS, CALENDAR_PREVIOUS_DISABLED,
};
use style::chat::SEARCH_IN_CHAT_MULTI_SELECT;
use style::chat_helpers::HISTORY_COMPOSE_BUTTON;
use style::dialogs::DIALOG_CALENDAR;
use style::info::{HISTORY_TOP_BAR_BACK, TOP_BAR_ACTION_SKIP, TOP_BAR_HEIGHT};
use style::widgets::{DEFAULT_SETTINGS_RIGHT_LABEL, IconButton as IconButtonStyle};
use style::{ColorId, ColorRef, Icon};
use ui::{IconButton, InputEvent, InputField, color, font, font_height};

use super::HistoryView;
use super::calendar::disabled_arrow;
use crate::lang::{strings, tr};
use crate::px_of;
use crate::store::spawn;

/// Typing waits this long before searching (`AutoSearchTimeout`).
const SEARCH_DELAY: Duration = Duration::from_millis(900);

/// The open search.
pub(super) struct ChatSearch {
    field: Entity<InputField>,
    /// What the last answer found, newest first; none before it came.
    found: Option<Found>,
    /// Counts queries, so only the answer to the latest one counts.
    requests: u64,
    _subscription: Subscription,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Found {
    ids: Vec<MessageId>,
    /// About how many match in all, at least as many as found.
    total: usize,
    /// The result shown, 0 for the newest.
    current: usize,
}

impl Found {
    /// The result `delta` steps older, if there is one.
    fn step(&self, delta: isize) -> Option<usize> {
        self.current
            .checked_add_signed(delta)
            .filter(|&index| index < self.ids.len())
    }

    /// "3 of 12", or "No results" (`BottomBar::updateText`).
    fn counter(&self, strings: &Strings) -> String {
        if self.ids.is_empty() {
            return strings.text(key::lng_search_messages_none).to_owned();
        }
        i18n::fill(
            strings.text(key::lng_search_messages_n_of_amount),
            &[
                ("n", &(self.current + 1).to_string()),
                ("amount", &self.total.to_string()),
            ],
        )
    }
}

impl HistoryView {
    pub(super) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| {
            InputField::new(
                &SEARCH_IN_CHAT_MULTI_SELECT.field,
                tr(cx, key::lng_dlg_filter),
                window,
                cx,
            )
        });
        let subscription = cx.subscribe(&field, |this, _, event, cx| match event {
            InputEvent::Changed => this.search_later(cx),
            InputEvent::Submitted => this.search_now(cx),
            _ => {}
        });
        field.update(cx, |field, cx| field.focus(window, cx));
        self.search = Some(ChatSearch {
            field,
            found: None,
            requests: 0,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Escape or the back arrow; `false` when no search was open.
    pub(super) fn close_search(&mut self, cx: &mut Context<Self>) -> bool {
        let was_open = self.search.take().is_some();
        if was_open {
            cx.notify();
        }
        was_open
    }

    fn search_later(&mut self, cx: &mut Context<Self>) {
        let Some(search) = &mut self.search else {
            return;
        };
        search.requests += 1;
        let request = search.requests;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                if this.search.as_ref().is_some_and(|s| s.requests == request) {
                    this.search_now(cx);
                }
            });
        })
        .detach();
    }

    fn search_now(&mut self, cx: &mut Context<Self>) {
        let Some(search) = &mut self.search else {
            return;
        };
        search.requests += 1;
        let request = search.requests;
        let query = search.field.read(cx).text().trim().to_owned();
        if query.is_empty() {
            search.found = None;
            cx.notify();
            return;
        }
        let task = spawn(
            self.store.read(cx).session().search_chat(self.chat, query),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.search.as_ref().is_none_or(|s| s.requests != request) {
                    return;
                }
                match result {
                    Ok(found) => this.show_found(found, cx),
                    Err(error) => tracing::warn!(%error, "searching the chat failed"),
                }
            });
        })
        .detach();
    }

    fn show_found(&mut self, found: model::search::FoundMessages, cx: &mut Context<Self>) {
        let ids: Vec<MessageId> = found.messages.iter().map(|message| message.id).collect();
        let total = usize::try_from(found.total_count)
            .unwrap_or(0)
            .max(ids.len());
        let first = ids.first().copied();
        if let Some(search) = &mut self.search {
            search.found = Some(Found {
                ids,
                total,
                current: 0,
            });
        }
        if let Some(first) = first {
            self.jump_to(first, cx);
        }
        cx.notify();
    }

    fn step_search(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(found) = self.search.as_mut().and_then(|s| s.found.as_mut()) else {
            return;
        };
        let Some(index) = found.step(delta) else {
            return;
        };
        found.current = index;
        let id = found.ids[index];
        self.jump_to(id, cx);
        cx.notify();
    }

    /// The field over the history, after a back arrow.
    // shortcut: no "from" filter and no Saved Messages tags; with the member list (M6) and tags (M3, later).
    pub(super) fn search_bar(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let search = self.search.as_ref()?;
        let weak = cx.weak_entity();
        let st = &SEARCH_IN_CHAT_MULTI_SELECT;
        Some(
            div()
                .flex_none()
                .flex()
                .items_center()
                .h(px_of(TOP_BAR_HEIGHT))
                .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
                .child(
                    IconButton::new("history-search-cancel", &HISTORY_TOP_BAR_BACK).on_click(
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| this.close_search(cx));
                        },
                    ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pr(px_of(st.padding.right))
                        .child(search.field.clone()),
                )
                .into_any_element(),
        )
    }

    /// The place among the results, the calendar and the arrows
    /// (`ComposeSearch::BottomBar`), once an answer came.
    // shortcut: no list of all results (`showList`) and no F3 or Ctrl+G stepping; with the results list (M3, later).
    pub(super) fn search_results_bar(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let found = self.search.as_ref()?.found.as_ref()?;
        let label = &DEFAULT_SETTINGS_RIGHT_LABEL;
        let weak = cx.weak_entity();
        let calendar = IconButton::new("history-search-calendar", &DIALOG_CALENDAR).on_click(
            move |_, window, cx| {
                let today = Local::now().date_naive();
                let _ = weak.update(cx, |this, cx| this.show_calendar(today, window, cx));
            },
        );
        Some(
            div()
                .flex_none()
                .flex()
                .items_center()
                .h(px_of(HISTORY_COMPOSE_BUTTON.height))
                .pl(px_of(TOP_BAR_ACTION_SKIP))
                .bg(color(cx, ColorRef::Palette(ColorId::DialogsBg)))
                .border_t_1()
                .border_color(color(cx, ColorRef::Palette(ColorId::ShadowFg)))
                .child(calendar)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font(font(label.style.font))
                        .text_size(px_of(label.style.font.size))
                        .line_height(font_height(label.style.font))
                        .text_color(color(cx, label.text_fg))
                        .child(found.counter(strings(cx))),
                )
                // Icons swapped as tdesktop has them: the right arrow is newer.
                .child(self.search_arrow(
                    found,
                    1,
                    (&CALENDAR_PREVIOUS, CALENDAR_PREVIOUS_DISABLED),
                    cx,
                ))
                .child(self.search_arrow(found, -1, (&CALENDAR_NEXT, CALENDAR_NEXT_DISABLED), cx))
                .into_any_element(),
        )
    }

    fn search_arrow(
        &self,
        found: &Found,
        delta: isize,
        (enabled, disabled): (&'static IconButtonStyle, Icon),
        cx: &Context<Self>,
    ) -> AnyElement {
        if found.step(delta).is_none() {
            return disabled_arrow(enabled, disabled);
        }
        let weak = cx.weak_entity();
        let id = if delta > 0 {
            "history-search-older"
        } else {
            "history-search-newer"
        };
        IconButton::new(id, enabled)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.step_search(delta, cx));
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_stay_within_the_found_results() {
        let found = Found {
            ids: vec![MessageId(30), MessageId(20), MessageId(10)],
            total: 3,
            current: 0,
        };
        assert_eq!(found.step(1), Some(1));
        assert_eq!(found.step(-1), None, "nothing newer than the newest");
        let oldest = Found {
            current: 2,
            ..found
        };
        assert_eq!(oldest.step(1), None, "nothing older than the oldest");
        assert_eq!(oldest.step(-1), Some(1));
    }

    #[test]
    fn the_counter_names_the_place_or_says_none_found() {
        let strings = Strings::english();
        let found = Found {
            ids: vec![MessageId(30), MessageId(20)],
            total: 12,
            current: 1,
        };
        assert_eq!(found.counter(&strings), "2 of 12");
        let none = Found {
            ids: Vec::new(),
            total: 0,
            current: 0,
        };
        assert_eq!(none.counter(&strings), "No results");
    }
}
