//! The UI's copy of the model: TDLib events are applied here, and screens
//! send their actions through it.

use std::future::Future;

use gpui::{App, Context, EventEmitter, Task};
use model::{Environment, Event, Session, State, auth};

/// The model state and the session that changes it.
#[derive(Debug)]
pub struct Store {
    state: State,
    session: Session,
    environment: Environment,
}

/// What changed in the store, for screens that react to transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreEvent {
    /// TDLib reported an authorization state, possibly the same one again
    /// (a resent code); it carries the previous one.
    AuthChanged { was: auth::State },
}

impl EventEmitter<StoreEvent> for Store {}

impl Store {
    #[must_use]
    pub fn new(session: Session, environment: Environment) -> Self {
        Self {
            state: State::default(),
            session,
            environment,
        }
    }

    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    #[must_use]
    pub fn environment(&self) -> Environment {
        self.environment
    }

    /// Switches to a new TDLib client after the old one closed (log out, or
    /// leaving QR login for a phone number).
    pub fn replace_session(&mut self, session: Session, cx: &mut Context<Self>) {
        self.session = session;
        cx.notify();
    }

    pub fn apply(&mut self, event: Event, cx: &mut Context<Self>) {
        let was = matches!(event, Event::Auth(_)).then(|| self.state.auth.clone());
        self.state.apply(event);
        if let Some(was) = was {
            cx.emit(StoreEvent::AuthChanged { was });
        }
        cx.notify();
    }

    /// Sends a sign-in action; the task reports whether TDLib accepted it.
    pub fn auth(&self, action: auth::Action, cx: &App) -> Task<Result<(), auth::Error>> {
        spawn(self.session.auth(action), cx)
    }

    /// Forgets accepted terms locally; TDLib sends no update for that.
    pub fn accept_terms(&mut self, id: &auth::TermsId, cx: &mut Context<Self>) {
        self.state.accept_terms(id);
        cx.notify();
    }
}

/// Runs a session request off the UI thread.
pub fn spawn<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
    cx: &App,
) -> Task<T> {
    cx.background_executor().spawn(future)
}
