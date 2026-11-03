//! The TDLib clients' lifecycle, one per account: updates into the
//! account's store, parameters when TDLib asks, and a new client after the
//! old one closed (log out of the only account, or QR login switching to a
//! phone number). An account that logs out while others stay is removed.

use std::path::PathBuf;
use std::rc::Rc;

use features::accounts::Start;
use features::{AccountId, Accounts, Store};
use futures::StreamExt as _;
use gpui::{App, AppContext as _, AsyncApp, Entity, WeakEntity};
use model::{Event, Parameters, Session, auth};

use crate::{paths, shutdown};

/// What every client of this run needs.
struct Connection {
    runtime: tg::Runtime,
    /// The first account's; the others differ in their directories.
    parameters: Parameters,
    data: PathBuf,
}

impl Connection {
    /// `id`'s own database, files and language packs.
    fn parameters(&self, id: AccountId) -> Parameters {
        let dir = paths::account_dir(&self.data, id.0);
        if let Err(error) = std::fs::create_dir_all(&dir) {
            tracing::error!(%error, account = id.0, "cannot create an account's directory");
        }
        let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
        Parameters {
            database_directory: path(paths::DATABASE_DIR),
            files_directory: path(paths::FILES_DIR),
            language_pack_database: path(paths::LANGUAGE_PACKS),
            ..self.parameters.clone()
        }
    }
}

/// One account's client: what its updates feed and what it was started with.
struct Client {
    id: AccountId,
    store: Entity<Store>,
    parameters: Rc<Parameters>,
    accounts: WeakEntity<Accounts>,
    connection: Rc<Connection>,
}

/// Starts a client for each remembered account and returns them, the
/// remembered one shown.
pub(crate) fn start(
    runtime: tg::Runtime,
    parameters: Parameters,
    data: PathBuf,
    cx: &mut App,
) -> Entity<Accounts> {
    shutdown::init(cx, runtime.stopper());
    let connection = Rc::new(Connection {
        runtime,
        parameters,
        data,
    });
    let start: Start = {
        let connection = connection.clone();
        Rc::new(move |id, accounts, cx| start_account(&connection, id, accounts, cx))
    };
    let (list, active) = features::settings::accounts(cx);
    let accounts = cx.new(|_| Accounts::new(Some(start)));
    for id in list.into_iter().map(AccountId) {
        let store = start_account(&connection, id, accounts.downgrade(), cx);
        accounts.update(cx, |accounts, cx| accounts.insert(id, store, cx));
    }
    accounts.update(cx, |accounts, cx| accounts.activate(AccountId(active), cx));
    accounts
}

fn start_account(
    connection: &Rc<Connection>,
    id: AccountId,
    accounts: WeakEntity<Accounts>,
    cx: &mut App,
) -> Entity<Store> {
    let parameters = Rc::new(connection.parameters(id));
    let (client, updates) = connection.runtime.create_client();
    let environment = parameters.environment;
    let store = cx.new(|_| Store::new(session(&parameters, client.clone()), environment));
    let (closed_sender, closed) = shutdown::closed_signal();
    shutdown::set_client(cx, id.0, client, closed);
    let client = Client {
        id,
        store: store.clone(),
        parameters,
        accounts,
        connection: connection.clone(),
    };
    pump(Rc::new(client), updates, closed_sender, cx);
    store
}

/// Applies the client's events until its updates end, then starts the next
/// client, unless the app is quitting or the account is gone.
fn pump(
    client: Rc<Client>,
    updates: tg::Updates,
    closed_sender: futures::channel::oneshot::Sender<()>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        let mut events = std::pin::pin!(model::events(updates));
        let mut was_signed_in = false;
        // Never log events: they carry message text.
        while let Some(event) = events.next().await {
            let is_waiting = matches!(event, Event::Auth(auth::State::WaitParameters));
            was_signed_in |= matches!(event, Event::Auth(auth::State::Ready));
            client.store.update(cx, |store, cx| store.apply(event, cx));
            if is_waiting {
                send_parameters(&client, cx);
            }
        }
        closed_sender.send(()).ok();
        cx.update(|cx| closed(&client, was_signed_in, cx));
    })
    .detach();
}

/// After a client's updates ended: an account given up before it signed in
/// is gone, one that logged out leaves the others, the only one restarts.
fn closed(client: &Rc<Client>, was_signed_in: bool, cx: &mut App) {
    if shutdown::is_quitting(cx) {
        return;
    }
    let Some(accounts) = client.accounts.upgrade() else {
        return;
    };
    let (is_listed, count) = {
        let accounts = accounts.read(cx);
        (accounts.contains(client.id), accounts.list().len())
    };
    if !is_listed || (was_signed_in && count > 1) {
        tracing::info!(
            account = client.id.0,
            "TDLib client closed; account removed"
        );
        shutdown::remove_client(cx, client.id.0);
        accounts.update(cx, |accounts, cx| accounts.remove(client.id, cx));
        return;
    }
    restart(client, cx);
}

fn session(parameters: &Parameters, client: tg::Client) -> Session {
    Session::new(client).with_views(parameters.views)
}

fn restart(client: &Rc<Client>, cx: &mut App) {
    tracing::info!(
        account = client.id.0,
        "TDLib client closed; starting a new one"
    );
    let (tdlib, updates) = client.connection.runtime.create_client();
    client.store.update(cx, |store, cx| {
        store.replace_session(session(&client.parameters, tdlib.clone()), cx)
    });
    let (closed_sender, closed) = shutdown::closed_signal();
    shutdown::set_client(cx, client.id.0, tdlib, closed);
    pump(client.clone(), updates, closed_sender, cx);
}

/// Answers `authorizationStateWaitTdlibParameters`, then loads the language
/// TDLib remembers.
fn send_parameters(client: &Client, cx: &mut AsyncApp) {
    let session = client
        .store
        .read_with(cx, |store, _| store.session().clone());
    let task = session.set_parameters(&client.parameters);
    cx.spawn(async move |cx| match task.await {
        Ok(()) => cx.update(|cx| features::lang::load(&session, cx)),
        Err(error) => tracing::error!(%error, "TDLib rejected its parameters"),
    })
    .detach();
}
