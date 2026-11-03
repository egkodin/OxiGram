//! Quitting: every account's TDLib client closes its database before the
//! process exits.
//!
//! rgram's own ways out (the Quit menu item, its shortcut, closing the last
//! window) hide the windows, send `close`, and quit once the update streams
//! end after `authorizationStateClosed`. When the OS ends the app instead (logout,
//! the Dock's Quit), only gpui's `on_app_quit` hook runs, within gpui's 200 ms
//! shutdown timeout. Either way the TDLib receive thread stops last: a thread
//! left inside `td_receive` crashes when `exit` destroys TDLib's statics.

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

/// A running client, by the account it serves.
struct Running {
    account: u32,
    client: tg::Client,
    closed: Closed,
}

struct Shutdown {
    clients: Vec<Running>,
    stopper: tg::Stopper,
    is_quitting: bool,
}

impl Shutdown {
    /// Asks every client to close; resolves once all of them have.
    fn close_all(&self) -> impl std::future::Future<Output = ()> + use<> {
        let closes: Vec<_> = self
            .clients
            .iter()
            .map(|running| {
                let close = running.client.request(&Close {});
                let closed = running.closed.clone();
                async move {
                    if let Err(error) = close.await {
                        tracing::warn!(%error, "closing TDLib failed");
                    }
                    closed.await.ok();
                }
            })
            .collect();
        future::join_all(closes).map(drop)
    }
}

impl Global for Shutdown {}

/// Installs the Quit action, its menu item and shortcut, and the quit hooks.
pub(crate) fn init(cx: &mut App, stopper: tg::Stopper) {
    cx.set_global(Shutdown {
        clients: Vec::new(),
        stopper,
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
        // After a graceful quit the requests fail at once with `Closed`, and
        // the clients have closed already.
        let closed = shutdown.close_all();
        let stopper = shutdown.stopper.clone();
        let executor = cx.background_executor().clone();
        async move {
            closed.await;
            tracing::info!("TDLib closed");
            // shortcut: up to a second on the receive timeout, past gpui's 200 ms; wake td_receive if the OS-quit path still crashes.
            executor.spawn(async move { stopper.stop() }).await;
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

/// Makes `client` the one quitting closes for `account`, in place of one
/// that closed before.
pub(crate) fn set_client(cx: &mut App, account: u32, client: tg::Client, closed: Closed) {
    let shutdown = cx.global_mut::<Shutdown>();
    shutdown
        .clients
        .retain(|running| running.account != account);
    shutdown.clients.push(Running {
        account,
        client,
        closed,
    });
}

/// Forgets the client of an account that is gone.
pub(crate) fn remove_client(cx: &mut App, account: u32) {
    let shutdown = cx.global_mut::<Shutdown>();
    shutdown
        .clients
        .retain(|running| running.account != account);
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
    let closed = shutdown.close_all();
    let stopper = shutdown.stopper.clone();
    for window in cx.windows() {
        if let Err(error) = window.update(cx, |_, window, _| window.remove_window()) {
            tracing::warn!("closing a window failed: {error:#}");
        }
    }
    let timeout = cx.background_executor().timer(CLOSE_TIMEOUT);
    cx.spawn(async move |cx| {
        match future::select(std::pin::pin!(closed), timeout).await {
            Either::Left(_) => tracing::info!("TDLib closed"),
            Either::Right(_) => {
                tracing::warn!(timeout = ?CLOSE_TIMEOUT, "TDLib did not close in time");
            }
        }
        cx.background_executor()
            .spawn(async move { stopper.stop() })
            .await;
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// A [`Closed`] future and the sender that resolves it.
pub(crate) fn closed_signal() -> (oneshot::Sender<()>, Closed) {
    let (sender, receiver) = oneshot::channel();
    (sender, receiver.shared())
}
