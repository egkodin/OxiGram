//! The UI's copy of the model: TDLib events are applied here, and screens
//! send their actions through it.

use std::collections::HashMap;
use std::future::Future;

use gpui::{App, Context, EventEmitter, Task};
use model::history::Page;
use model::message::Message;
use model::thread::Key;
use model::{ChatId, Environment, Event, MessageId, Session, State, auth};

/// The model state and the session that changes it.
#[derive(Debug)]
pub struct Store {
    state: State,
    session: Session,
    environment: Environment,
    /// Messages picked for forwarding, by the chat they go to, until that
    /// chat's field takes them (`History::setForwardDraft`).
    forwarding: HashMap<ChatId, Forwarding>,
}

/// Messages of one chat waiting to be forwarded, with the bar's lines,
/// made where the messages were shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forwarding {
    pub from: ChatId,
    pub messages: Vec<MessageId>,
    pub title: String,
    pub text: String,
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
            forwarding: HashMap::new(),
        }
    }

    pub fn set_forwarding(&mut self, to: ChatId, forwarding: Forwarding, cx: &mut Context<Self>) {
        self.forwarding.insert(to, forwarding);
        cx.notify();
    }

    #[must_use]
    pub fn has_forwarding(&self, to: ChatId) -> bool {
        self.forwarding.contains_key(&to)
    }

    pub fn take_forwarding(&mut self, to: ChatId) -> Option<Forwarding> {
        self.forwarding.remove(&to)
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

    /// Starts keeping `key`'s messages; TDLib hears the chat is open with
    /// its first history.
    pub fn open_chat(&mut self, key: Key, cx: &mut Context<Self>) {
        let was_open = self.state.is_chat_open(key.chat);
        self.state.open_history(key);
        cx.notify();
        if !was_open {
            log_failure(
                spawn(self.session.open_chat(key.chat), cx),
                "opening a chat",
                cx,
            );
        }
    }

    pub fn close_chat(&mut self, key: Key, cx: &mut Context<Self>) {
        self.state.close_history(key);
        cx.notify();
        if !self.state.is_chat_open(key.chat) {
            log_failure(
                spawn(self.session.close_chat(key.chat), cx),
                "closing a chat",
                cx,
            );
        }
    }

    pub fn add_history_page(
        &mut self,
        key: Key,
        page: Page,
        messages: Vec<Message>,
        cx: &mut Context<Self>,
    ) {
        self.state.add_history_page(key, page, messages);
        cx.notify();
    }

    pub fn clear_history(&mut self, key: Key, cx: &mut Context<Self>) {
        self.state.clear_history(key);
        cx.notify();
    }

    pub fn set_pinned(&mut self, key: Key, messages: Vec<Message>, cx: &mut Context<Self>) {
        self.state.set_pinned(key, messages);
        cx.notify();
    }

    pub fn add_reference(&mut self, key: Key, message: Message, cx: &mut Context<Self>) {
        self.state.add_reference(key, message);
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

fn log_failure(
    task: Task<Result<(), model::RequestError>>,
    what: &'static str,
    cx: &mut Context<Store>,
) {
    cx.spawn(async move |_, _| {
        if let Err(error) = task.await {
            tracing::warn!(%error, what, "request failed");
        }
    })
    .detach();
}

/// Runs a session request off the UI thread.
pub fn spawn<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
    cx: &App,
) -> Task<T> {
    cx.background_executor().spawn(future)
}
