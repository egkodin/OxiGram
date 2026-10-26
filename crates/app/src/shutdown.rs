//! Quitting: TDLib closes its database before the process exits.
//!
//! rgram's own ways out (the Quit menu item, its shortcut, closing the last
//! window) hide the windows, send `close`, and quit once the update stream ends
//! after `authorizationStateClosed`. When the OS ends the app instead (logout,
//! the Dock's Quit), only gpui's `on_app_quit` hook runs, within gpui's 200 ms
//! shutdown timeout.

use std::time::Duration;

use futures::FutureExt as _;
use futures::channel::oneshot;
use futures::future::{self, Either, Shared};
use gpui::{App, Global, KeyBinding, Menu, MenuItem, actions};
use td_api::functions::Close;

actions!(rgram, [Quit]);

const APP_NAME: &str = "rgram";

/// How long quitting waits for TDLib to report that it closed.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// tdesktop binds `ctrl+q` to quit on every OS (`core/shortcuts.cpp`); on macOS
/// the menu's Cmd-Q takes that role.
#[cfg(target_os = "macos")]
const QUIT_SHORTCUT: &str = "cmd-q";
#[cfg(not(target_os = "macos"))]
const QUIT_SHORTCUT: &str = "ctrl-q";

/// Resolves once the update stream of the client has ended.
pub(crate) type Closed = Shared<oneshot::Receiver<()>>;

struct Shutdown {
    client: tg::Client,
    closed: Closed,
    is_quitting: bool,
}

impl Global for Shutdown {}

/// Installs the Quit action, its menu item and shortcut, and the quit hooks.
pub(crate) fn init(cx: &mut App, client: tg::Client, closed: Closed) {
    cx.set_global(Shutdown {
        client,
        closed,
        is_quitting: false,
    });
    cx.on_action(|_: &Quit, cx| quit(cx));
    cx.bind_keys([KeyBinding::new(QUIT_SHORTCUT, Quit, None)]);
    set_menus(cx);
    cx.observe_global::<features::Lang>(set_menus).detach();
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            quit(cx);
        }
    })
    .detach();
    cx.on_app_quit(|cx| {
        let shutdown = cx.global::<Shutdown>();
        // After a graceful quit this fails at once with `Closed`, and `closed`
        // has already resolved.
        let close = shutdown.client.request(&Close {});
        let closed = shutdown.closed.clone();
        async move {
            close.await.ok();
            if closed.await.is_ok() {
                tracing::info!("TDLib closed");
            }
        }
    })
    .detach();
}

fn set_menus(cx: &mut App) {
    let quit = features::lang::tr_with(
        cx,
        i18n::key::lng_mac_menu_quit_telegram,
        &[("telegram", APP_NAME)],
    );
    cx.set_menus(vec![Menu {
        name: APP_NAME.into(),
        items: vec![MenuItem::action(quit, Quit)],
        disabled: false,
    }]);
}

/// Makes `client` the one quitting closes, after the previous one closed.
pub(crate) fn set_client(cx: &mut App, client: tg::Client, closed: Closed) {
    let shutdown = cx.global_mut::<Shutdown>();
    shutdown.client = client;
    shutdown.closed = closed;
}

#[must_use]
pub(crate) fn is_quitting(cx: &App) -> bool {
    cx.global::<Shutdown>().is_quitting
}

/// Hides every window, closes TDLib, and quits once it has closed.
pub(crate) fn quit(cx: &mut App) {
    let shutdown = cx.global_mut::<Shutdown>();
    if std::mem::replace(&mut shutdown.is_quitting, true) {
        return;
    }
    let close = shutdown.client.request(&Close {});
    let closed = shutdown.closed.clone();
    for window in cx.windows() {
        if let Err(error) = window.update(cx, |_, window, _| window.remove_window()) {
            tracing::warn!("closing a window failed: {error:#}");
        }
    }
    let timeout = cx.background_executor().timer(CLOSE_TIMEOUT);
    cx.spawn(async move |cx| {
        if let Err(error) = close.await {
            tracing::warn!(%error, "closing TDLib failed");
        }
        match future::select(closed, timeout).await {
            Either::Left(_) => tracing::info!("TDLib closed"),
            Either::Right(_) => {
                tracing::warn!(timeout = ?CLOSE_TIMEOUT, "TDLib did not close in time");
            }
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// A [`Closed`] future and the sender that resolves it.
pub(crate) fn closed_signal() -> (oneshot::Sender<()>, Closed) {
    let (sender, receiver) = oneshot::channel();
    (sender, receiver.shared())
}
