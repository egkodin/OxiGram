//! The UI's copy of the model: TDLib events are applied here, and screens
//! send their actions through it.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use gpui::{App, Context, EventEmitter, Task};
use model::file::File;
use model::history::Page;
use model::message::Message;
use model::reaction::ReactionAnimations;
use model::sticker::{StickerKind, StickerPicture};
use model::thread::Key;
use model::{
    ChatId, CustomEmojiId, Environment, Event, FileId, MessageId, RequestError, Session, State,
    auth,
};

/// A userpic whose download stopped short is asked for this many times in
/// all, as TDLib does not retry a stopped download on its own.
const USERPIC_TRIES: u8 = 3;
/// The wait before the second try, doubled before each next one.
const USERPIC_RETRY: Duration = Duration::from_secs(2);
/// Emoji pictures download ahead of photos, behind userpics.
const EMOJI_PRIORITY: i32 = 16;

/// The model state and the session that changes it.
#[derive(Debug)]
pub struct Store {
    state: State,
    session: Session,
    environment: Environment,
    /// Messages picked for forwarding, by the chat they go to, until that
    /// chat's field takes them (`History::setForwardDraft`).
    forwarding: HashMap<ChatId, Forwarding>,
    /// Userpics asked for: how many tries, and whether one is running.
    userpics: HashMap<FileId, Userpic>,
    /// Emoji pictures asked for this run: None while they load or when
    /// Telegram has none.
    emoji: HashMap<EmojiKey, Option<Emoji>>,
    /// Emoji reactions' animations asked for this run, by emoji.
    reaction_animations: HashMap<String, Option<ReactionAnimations>>,
}

/// What an emoji is drawn from.
#[derive(Debug, Clone)]
struct Emoji {
    /// The still picture's path once on disk.
    still: Option<String>,
    sticker: StickerPicture,
}

/// An emoji drawn from Telegram's own picture.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EmojiKey {
    /// A custom emoji (`getCustomEmojiStickers`).
    Custom(CustomEmojiId),
    /// A lone emoji's big version (`getAnimatedEmoji`).
    Animated(String),
}

#[derive(Debug, Clone, Copy, Default)]
struct Userpic {
    tries: u8,
    is_loading: bool,
}

impl Userpic {
    /// Starts a try, with the pause before the one after it; None while one
    /// runs or when all were used.
    fn start(&mut self) -> Option<Duration> {
        if self.is_loading || self.tries >= USERPIC_TRIES {
            return None;
        }
        let pause = USERPIC_RETRY * (1 << self.tries);
        self.tries += 1;
        self.is_loading = true;
        Some(pause)
    }
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
            userpics: HashMap::new(),
            emoji: HashMap::new(),
            reaction_animations: HashMap::new(),
        }
    }

    /// Downloads a userpic once per run; its path reaches the state when
    /// done. One that stops short is asked again after a pause, up to
    /// `USERPIC_TRIES` times, from the next render that draws it.
    pub fn load_userpic(&mut self, file: FileId, priority: i32, cx: &mut Context<Self>) {
        let userpic = self.userpics.entry(file).or_default();
        let Some(pause) = userpic.start() else {
            return;
        };
        let task = spawn(self.session.download_whole_file(file, priority), cx);
        cx.spawn(async move |this, cx| {
            let done = match task.await {
                Ok(loaded) if loaded.path.is_some() => Some(loaded),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!(%error, "downloading a userpic failed");
                    None
                }
            };
            if let Some(loaded) = done {
                let _ = this.update(cx, |this, cx| {
                    this.apply(Event::Chats(model::chats::Change::File(loaded)), cx);
                });
                return;
            }
            cx.background_executor().timer(pause).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(userpic) = this.userpics.get_mut(&file) {
                    userpic.is_loading = false;
                }
                // Views draw again and ask for it once more.
                cx.notify();
            });
        })
        .detach();
    }

    /// The path of an emoji's picture once on disk; the first ask fetches
    /// it, and views draw again when it lands.
    // shortcut: one request per emoji and no retry; batch ids and retry when long reaction lists or a flaky network show gaps.
    pub fn emoji_picture(&mut self, key: &EmojiKey, cx: &mut Context<Self>) -> Option<String> {
        self.emoji(key, cx)?.still
    }

    /// The TGS file of an animated emoji, asked for as `emoji_picture` is.
    pub fn emoji_animation(&mut self, key: &EmojiKey, cx: &mut Context<Self>) -> Option<File> {
        let sticker = self.emoji(key, cx)?.sticker;
        (sticker.kind == StickerKind::Animated).then_some(sticker.file)
    }

    /// The emoji's sticker, asked for as `emoji_picture` is.
    pub fn emoji_sticker(
        &mut self,
        key: &EmojiKey,
        cx: &mut Context<Self>,
    ) -> Option<StickerPicture> {
        Some(self.emoji(key, cx)?.sticker)
    }

    fn emoji(&mut self, key: &EmojiKey, cx: &mut Context<Self>) -> Option<Emoji> {
        if let Some(known) = self.emoji.get(key) {
            return known.clone();
        }
        self.emoji.insert(key.clone(), None);
        let task = spawn(fetch_emoji(self.session.clone(), key.clone()), cx);
        let key = key.clone();
        cx.spawn(async move |this, cx| match task.await {
            Ok(Some(emoji)) => {
                let _ = this.update(cx, |this, cx| {
                    this.emoji.insert(key, Some(emoji));
                    cx.notify();
                });
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "loading an emoji picture failed"),
        })
        .detach();
        None
    }

    /// An emoji reaction's animations; the first ask fetches them.
    pub fn reaction_animations(
        &mut self,
        emoji: &str,
        cx: &mut Context<Self>,
    ) -> Option<ReactionAnimations> {
        if let Some(known) = self.reaction_animations.get(emoji) {
            return known.clone();
        }
        self.reaction_animations.insert(emoji.to_owned(), None);
        let task = spawn(self.session.emoji_reaction(emoji.to_owned()), cx);
        let emoji = emoji.to_owned();
        cx.spawn(async move |this, cx| match task.await {
            Ok(animations) => {
                let _ = this.update(cx, |this, cx| {
                    this.reaction_animations.insert(emoji, Some(animations));
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "loading a reaction's animations failed"),
        })
        .detach();
        None
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

    pub fn set_scheduled(&mut self, key: Key, messages: Vec<Message>, cx: &mut Context<Self>) {
        self.state.set_scheduled(key, messages);
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
/// Finds an emoji's sticker and downloads the picture gpui draws.
async fn fetch_emoji(session: Session, key: EmojiKey) -> Result<Option<Emoji>, RequestError> {
    let picture: Option<StickerPicture> = match key {
        EmojiKey::Custom(id) => session
            .custom_emoji(vec![id])
            .await?
            .into_iter()
            .next()
            .map(|(_, picture)| picture),
        EmojiKey::Animated(emoji) => session.animated_emoji(emoji).await?,
    };
    let Some(picture) = picture else {
        return Ok(None);
    };
    let still = match picture.drawable() {
        Some(file) if file.path.is_some() => file.path.clone(),
        Some(file) => {
            session
                .download_whole_file(file.id, EMOJI_PRIORITY)
                .await?
                .path
        }
        None => None,
    };
    Ok(Some(Emoji {
        still,
        sticker: picture,
    }))
}

pub fn spawn<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
    cx: &App,
) -> Task<T> {
    cx.background_executor().spawn(future)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_userpic_is_tried_three_times_with_growing_pauses() {
        let mut userpic = Userpic::default();
        assert_eq!(userpic.start(), Some(Duration::from_secs(2)));
        assert_eq!(userpic.start(), None, "one try runs at a time");
        userpic.is_loading = false;
        assert_eq!(userpic.start(), Some(Duration::from_secs(4)));
        userpic.is_loading = false;
        assert_eq!(userpic.start(), Some(Duration::from_secs(8)));
        userpic.is_loading = false;
        assert_eq!(userpic.start(), None, "three tries in all");
    }
}
