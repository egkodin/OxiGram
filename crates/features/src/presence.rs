//! Online while the window is active and in use (`Updates::updateOnline`):
//! offline once it loses focus, after `offlineIdleTimeout` without a key
//! press, click or wheel (`Application::updateNonIdle`), and when the
//! account's view goes (switching accounts).

use std::time::{Duration, Instant};

use gpui::{
    App, Context, DispatchPhase, Div, Entity, InteractiveElement as _, MouseDownEvent,
    ParentElement as _, ScrollEvent, Styled as _, Subscription, Task, Window, canvas,
};
use model::Presence;

use crate::store::Store;

/// `Config::offlineIdleTimeout`'s default.
// shortcut: the default, not the server's `offline_idle_timeout_ms`, and app input only (tdesktop also asks the OS for its last input); read both when they matter.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// How often the idle time is checked.
const CHECK_PERIOD: Duration = Duration::from_secs(5);

pub(crate) struct Tracker {
    store: Entity<Store>,
    last_input: Instant,
    /// What Telegram was last told; None before the first request.
    sent: Option<Presence>,
    _activation: Subscription,
    _release: Subscription,
    _check: Task<()>,
}

impl Tracker {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.last_input = Instant::now();
            }
            this.refresh(window, cx);
        });
        let release = cx.on_release(|this, cx| send(&this.store, Presence::Offline, cx));
        let check = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(CHECK_PERIOD).await;
                if this
                    .update_in(cx, |this, window, cx| this.refresh(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut tracker = Self {
            store,
            last_input: Instant::now(),
            sent: None,
            _activation: activation,
            _release: release,
            _check: check,
        };
        tracker.refresh(window, cx);
        tracker
    }

    /// A key press, click or wheel in the window.
    fn note_input(&mut self, window: &Window, cx: &mut App) {
        self.last_input = Instant::now();
        self.refresh(window, cx);
    }

    fn refresh(&mut self, window: &Window, cx: &mut App) {
        let presence = presence(window.is_window_active(), self.last_input.elapsed());
        if self.sent != Some(presence) {
            self.sent = Some(presence);
            send(&self.store, presence, cx);
        }
    }
}

/// Feeds the window's key presses, clicks and wheel turns to `tracker`,
/// boxes included: they draw beside the account's view, not in it.
pub(crate) fn watch_input(root: Div, tracker: &Entity<Tracker>) -> Div {
    let keys = tracker.clone();
    let clicks = tracker.clone();
    let wheel = tracker.clone();
    // Window-wide mouse listeners: lists stop the wheel before it bubbles up.
    let mouse = canvas(
        |_, _, _| {},
        move |_, (), window, _| {
            window.on_mouse_event(move |_: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    note(&clicks, window, cx);
                }
            });
            window.on_mouse_event(move |_: &ScrollEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    note(&wheel, window, cx);
                }
            });
        },
    )
    .absolute()
    .size_full();
    root.capture_key_down(move |_, window, cx| note(&keys, window, cx))
        .child(mouse)
}

fn note(tracker: &Entity<Tracker>, window: &Window, cx: &mut App) {
    tracker.update(cx, |tracker, cx| tracker.note_input(window, cx));
}

fn presence(is_active: bool, idle: Duration) -> Presence {
    if is_active && idle < IDLE_TIMEOUT {
        Presence::Online
    } else {
        Presence::Offline
    }
}

fn send(store: &Entity<Store>, presence: Presence, cx: &mut App) {
    tracing::debug!(?presence, "presence");
    let task = store.read(cx).session().set_presence(presence);
    crate::peer_menu::send(task, "presence", cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_only_while_active_and_used() {
        let recent = Duration::from_secs(5);
        assert_eq!(presence(true, recent), Presence::Online);
        assert_eq!(presence(false, recent), Presence::Offline);
        assert_eq!(presence(true, IDLE_TIMEOUT), Presence::Offline);
    }
}
