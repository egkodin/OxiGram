//! TDLib client runtime (ADR 0003).
//!
//! [`Runtime`] owns the process's only `td_receive` caller: a dedicated thread
//! that routes every object TDLib returns, by the `@client_id` and `@extra`
//! fields TDLib appends to it. Each [`Client`] offers exactly two interfaces:
//! [`Client::request`] for typed requests and the [`Updates`] stream returned
//! with it.
//!
//! Responses only report the outcome of a request. State changes always arrive
//! as updates, in TDLib's order, so callers never apply a response to state.

mod error;
mod route;

use std::collections::HashMap;
use std::ffi::CString;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

use futures_channel::{mpsc, oneshot};
use td_api::Function;
use td_api::enums::{AuthorizationState, Update};
use td_api::functions::GetOption;
use td_sys::{ClientId, Receiver};

pub use error::{DecodeError, TdError};
use route::Route;

/// How long one `td_receive` call blocks; also the latency of noticing shutdown.
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(1);

/// Runs synchronous TDLib requests, such as `setLogVerbosityLevel` or
/// `getOption("version")`. Only functions the schema marks as "can be called
/// synchronously" are answered.
///
/// # Errors
///
/// Returns the TDLib error, or a decode error if the response does not match
/// `F::Response`.
pub fn execute<F: Function>(function: &F) -> Result<F::Response, TdError> {
    let request = encode(function, None)?;
    let response = td_sys::execute(&request).ok_or(TdError::NoResponse)?;
    decode::<F>(&response)
}

/// The receive thread and the routing state shared with every [`Client`].
///
/// At most one runtime runs per process, because TDLib allows a single
/// `td_receive` caller. Dropping it stops the thread within a second;
/// requests still waiting then fail with [`TdError::Closed`].
#[derive(Debug)]
pub struct Runtime {
    shared: Arc<Shared>,
}

/// Why [`Runtime::start`] failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StartError {
    /// Another runtime still owns TDLib's receiver.
    #[error("a TDLib runtime is already running in this process")]
    AlreadyRunning,
    /// The OS refused to start the receive thread.
    #[error("cannot start the TDLib receive thread")]
    Thread(#[source] std::io::Error),
}

impl Runtime {
    /// Starts the receive thread.
    ///
    /// # Errors
    ///
    /// Fails if another runtime is running or the thread cannot be spawned.
    pub fn start() -> Result<Self, StartError> {
        let mut receiver = Receiver::take().ok_or(StartError::AlreadyRunning)?;
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            next_extra: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
        });
        let thread_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("tdlib-receive".to_owned())
            .spawn(move || {
                while thread_shared.running.load(Ordering::Acquire) {
                    if let Some(object) = receiver.receive(RECEIVE_TIMEOUT) {
                        thread_shared.dispatch(&object);
                    }
                }
                thread_shared.shut_down();
            })
            .map_err(StartError::Thread)?;
        Ok(Self { shared })
    }

    /// Creates a TDLib client instance and the stream of its updates.
    ///
    /// The instance starts working after its first request.
    #[must_use]
    pub fn create_client(&self) -> (Client, Updates) {
        let id = td_sys::create_client_id();
        let (sender, receiver) = mpsc::unbounded();
        self.shared.lock_clients().insert(id, sender);
        let client = Client {
            id,
            shared: Arc::clone(&self.shared),
        };
        // TDLib starts an instance only on its first request (`td_create_client_id`),
        // so until then no `authorizationStateWaitTdlibParameters` comes; the answer
        // is not needed.
        drop(client.send(&GetOption {
            name: "version".to_owned(),
        }));
        (client, Updates(receiver))
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::Release);
    }
}

/// A handle to one TDLib client instance. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Client {
    id: ClientId,
    shared: Arc<Shared>,
}

impl Client {
    /// Sends `function` now and returns a future of its response.
    ///
    /// The request is sent when this is called, not when the future is first
    /// polled, so requests reach TDLib in call order.
    ///
    /// # Errors
    ///
    /// The future resolves to the TDLib error, [`TdError::Closed`] if the client
    /// or runtime closed first, or a decode error.
    pub fn request<F: Function>(
        &self,
        function: &F,
    ) -> impl Future<Output = Result<F::Response, TdError>> + Send + use<F> {
        let sent = self.send(function);
        async move {
            let response = sent?.await.map_err(|_| TdError::Closed)?;
            decode::<F>(&response)
        }
    }

    fn send<F: Function>(&self, function: &F) -> Result<oneshot::Receiver<String>, TdError> {
        let extra = self.shared.next_extra.fetch_add(1, Ordering::Relaxed);
        let request = encode(function, Some(extra))?;
        let (reply, response) = oneshot::channel();
        {
            // Checked under the pending lock, which closing and shutdown also hold,
            // so no request is registered for a client that is already gone.
            let mut pending = self.shared.lock_pending();
            let is_open = self.shared.running.load(Ordering::Acquire)
                && self.shared.lock_clients().contains_key(&self.id);
            if !is_open {
                return Err(TdError::Closed);
            }
            pending.insert(
                extra,
                Pending {
                    client: self.id,
                    reply,
                },
            );
        }
        td_sys::send(self.id, &request);
        Ok(response)
    }
}

/// Updates of one client, in TDLib's order. Ends after
/// `authorizationStateClosed` or when the runtime stops.
#[derive(Debug)]
pub struct Updates(mpsc::UnboundedReceiver<Update>);

impl futures_core::Stream for Updates {
    type Item = Update;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Update>> {
        Pin::new(&mut self.0).poll_next(cx)
    }
}

/// Lock order: `pending` before `clients`.
#[derive(Debug)]
struct Shared {
    running: AtomicBool,
    next_extra: AtomicU64,
    pending: Mutex<HashMap<u64, Pending>>,
    /// Open clients; a client is removed once TDLib reports it closed.
    clients: Mutex<HashMap<ClientId, mpsc::UnboundedSender<Update>>>,
}

#[derive(Debug)]
struct Pending {
    client: ClientId,
    reply: oneshot::Sender<String>,
}

impl Shared {
    fn lock_pending(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Pending>> {
        self.pending.lock().expect("pending-request lock poisoned")
    }

    fn lock_clients(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<ClientId, mpsc::UnboundedSender<Update>>> {
        self.clients.lock().expect("client lock poisoned")
    }

    /// Routes one object from `td_receive`.
    fn dispatch(&self, object: &str) {
        let Some(route) = Route::of(object) else {
            tracing::warn!(
                object = route::type_name(object),
                "dropping object without @client_id"
            );
            return;
        };
        match route.extra {
            Some(extra) => self.respond(extra, object),
            None => self.update(route.client, object),
        }
    }

    fn respond(&self, extra: u64, object: &str) {
        let pending = self.lock_pending().remove(&extra);
        match pending {
            // A send error means the caller dropped its future; nothing to do.
            Some(pending) => drop(pending.reply.send(object.to_owned())),
            None => tracing::debug!(extra, "response to an unknown request"),
        }
    }

    fn update(&self, client: ClientId, object: &str) {
        let update = match serde_json::from_str::<Update>(object) {
            Ok(update) => update,
            Err(error) => {
                tracing::warn!(
                    update = route::type_name(object),
                    error = %DecodeError::from(&error),
                    "dropping undecodable update"
                );
                return;
            }
        };
        let is_closed = matches!(
            &update,
            Update::AuthorizationState(state)
                if matches!(*state.authorization_state, AuthorizationState::Closed(_))
        );
        if is_closed {
            let mut pending = self.lock_pending();
            let mut clients = self.lock_clients();
            if let Some(sender) = clients.remove(&client) {
                drop(sender.unbounded_send(update));
            }
            pending.retain(|_, pending| pending.client != client);
        } else if let Some(sender) = self.lock_clients().get(&client) {
            // A send error means the app stopped listening; nothing to do.
            drop(sender.unbounded_send(update));
        }
    }

    /// Fails every waiting request and ends every update stream.
    fn shut_down(&self) {
        let mut pending = self.lock_pending();
        pending.clear();
        self.lock_clients().clear();
    }
}

/// Serializes `function`, adding `@extra` for requests that expect a routed reply.
fn encode<F: Function>(function: &F, extra: Option<u64>) -> Result<CString, TdError> {
    let json = serde_json::to_string(function).map_err(|_| TdError::Encode(F::NAME))?;
    let json = match (extra, json.strip_prefix('{')) {
        // Every function serializes as `{"@type":...}`, so a field always follows.
        (Some(extra), Some(fields)) => format!("{{\"@extra\":{extra},{fields}"),
        (Some(_), None) => return Err(TdError::Encode(F::NAME)),
        (None, _) => json,
    };
    // serde_json escapes NUL, so this only fails on a serializer bug.
    CString::new(json).map_err(|_| TdError::Encode(F::NAME))
}

fn decode<F: Function>(response: &str) -> Result<F::Response, TdError> {
    if route::type_name(response) == "error" {
        return Err(TdError::from_error_object(response));
    }
    serde_json::from_str(response).map_err(|error| TdError::Decode {
        function: F::NAME,
        error: DecodeError::from(&error),
    })
}

const _: fn() = || {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Runtime>();
    send_sync::<Client>();
    send_sync::<TdError>();
    fn send<T: Send>() {}
    send::<Updates>();
};
