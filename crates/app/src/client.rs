//! The TDLib client's lifecycle: updates into the store, parameters when
//! TDLib asks, and a new client after the old one closed (log out, or QR
//! login switching to a phone number).

use std::rc::Rc;

use features::Store;
use futures::StreamExt as _;
use gpui::{App, AppContext as _, AsyncApp, Entity};
use model::{Event, Parameters, Session, auth};

use crate::shutdown;

/// What every client of this run needs.
struct Connection {
    runtime: tg::Runtime,
    parameters: Parameters,
}

/// Starts the first client and returns the store it feeds.
pub(crate) fn start(runtime: tg::Runtime, parameters: Parameters, cx: &mut App) -> Entity<Store> {
    let connection = Rc::new(Connection {
        runtime,
        parameters,
    });
    let (client, updates) = connection.runtime.create_client();
    let environment = connection.parameters.environment;
    let store = cx.new(|_| Store::new(Session::new(client.clone()), environment));
    let (closed_sender, closed) = shutdown::closed_signal();
    shutdown::init(cx, client, closed);
    pump(store.clone(), connection, updates, closed_sender, cx);
    store
}

/// Applies the client's events until its updates end, then starts the next
/// client unless the app is quitting.
fn pump(
    store: Entity<Store>,
    connection: Rc<Connection>,
    updates: tg::Updates,
    closed_sender: futures::channel::oneshot::Sender<()>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        let mut events = std::pin::pin!(model::events(updates));
        // Never log events: they carry message text.
        while let Some(event) = events.next().await {
            let is_waiting = matches!(event, Event::Auth(auth::State::WaitParameters));
            store.update(cx, |store, cx| store.apply(event, cx));
            if is_waiting {
                send_parameters(&store, &connection, cx);
            }
        }
        closed_sender.send(()).ok();
        cx.update(|cx| {
            if !shutdown::is_quitting(cx) {
                restart(store, connection, cx);
            }
        });
    })
    .detach();
}

fn restart(store: Entity<Store>, connection: Rc<Connection>, cx: &mut App) {
    tracing::info!("TDLib client closed; starting a new one");
    let (client, updates) = connection.runtime.create_client();
    store.update(cx, |store, cx| {
        store.replace_session(Session::new(client.clone()), cx)
    });
    let (closed_sender, closed) = shutdown::closed_signal();
    shutdown::set_client(cx, client, closed);
    pump(store, connection, updates, closed_sender, cx);
}

/// Answers `authorizationStateWaitTdlibParameters`, then loads the language
/// TDLib remembers.
fn send_parameters(store: &Entity<Store>, connection: &Connection, cx: &mut AsyncApp) {
    let session = store.read_with(cx, |store, _| store.session().clone());
    let task = session.set_parameters(&connection.parameters);
    cx.spawn(async move |cx| match task.await {
        Ok(()) => cx.update(|cx| features::lang::load(&session, cx)),
        Err(error) => tracing::error!(%error, "TDLib rejected its parameters"),
    })
    .detach();
}
