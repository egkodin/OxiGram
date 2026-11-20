//! The UI's copy of the model: TDLib events are applied here, and screens
//! send their actions through it.

use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant};

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
    emoji: EmojiCache,
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

const EMOJI_CAPACITY: usize = 4096;
const EMOJI_RETAIN: usize = 3072;
const EMOJI_KEEP: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
struct EmojiCache {
    entries: HashMap<EmojiKey, EmojiEntry>,
    last_trim: Option<Instant>,
}

#[derive(Debug)]
enum EmojiEntry {
    Loading,
    Ready {
        value: Option<Box<Emoji>>,
        used: Instant,
    },
}

impl EmojiCache {
    /// A miss reserves a fetch. A known entry can still be loading or empty.
    fn lookup(&mut self, key: &EmojiKey, now: Instant) -> Option<Option<Emoji>> {
        match self.entries.get_mut(key) {
            Some(EmojiEntry::Ready { value, used }) => {
                *used = now;
                Some(value.as_deref().cloned())
            }
            Some(EmojiEntry::Loading) => Some(None),
            None => {
                self.entries.insert(key.clone(), EmojiEntry::Loading);
                None
            }
        }
    }

    fn finish(&mut self, key: EmojiKey, value: Option<Emoji>, now: Instant) {
        self.entries.insert(
            key,
            EmojiEntry::Ready {
                value: value.map(Box::new),
                used: now,
            },
        );
        if self.entries.len() <= EMOJI_CAPACITY
            || self
                .last_trim
                .is_some_and(|last| now.duration_since(last) < EMOJI_KEEP)
        {
            return;
        }
        self.last_trim = Some(now);
        let mut cold: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(key, entry)| match entry {
                EmojiEntry::Ready { used, .. } if now.duration_since(*used) >= EMOJI_KEEP => {
                    Some((key.clone(), *used))
                }
                _ => None,
            })
            .collect();
        cold.sort_unstable_by_key(|(_, used)| *used);
        for (key, _) in cold.into_iter().take(self.entries.len() - EMOJI_RETAIN) {
            self.entries.remove(&key);
        }
        self.entries.shrink_to(EMOJI_CAPACITY);
    }
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
            emoji: EmojiCache::default(),
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
        if let Some(known) = self.emoji.lookup(key, Instant::now()) {
            return known;
        }
        let task = spawn(fetch_emoji(self.session.clone(), key.clone()), cx);
        let key = key.clone();
        cx.spawn(async move |this, cx| {
            let emoji = match task.await {
                Ok(emoji) => emoji,
                Err(error) => {
                    tracing::warn!(%error, "loading an emoji picture failed");
                    None
                }
            };
            let _ = this.update(cx, |this, cx| {
                let loaded = emoji.is_some();
                this.emoji.finish(key, emoji, Instant::now());
                if loaded {
                    cx.notify();
                }
            });
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
        if let Event::Chats(model::chats::Change::File(file)) = &event {
            crate::streaming::wake(self.session.client_id(), file.id);
        }
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
    #[ignore = "offline memory measurement"]
    fn offline_account_cache_workload() {
        let file = |id| File {
            id: FileId(id),
            path: Some(format!(
                "/synthetic/account1/cache/thumbnails/photo-{id:08}.webp"
            )),
            is_downloading: false,
            downloaded: 512,
            size: 512,
            uploaded: 512,
            is_uploading: false,
            prefix: Default::default(),
        };
        let mut emoji = EmojiCache::default();
        let start = Instant::now();
        for id in 0..10_000 {
            let key = EmojiKey::Custom(CustomEmojiId(i64::from(id)));
            let still = file(id + 10_000);
            let now = start + Duration::from_millis(u64::try_from(id).unwrap() * 30);
            assert!(emoji.lookup(&key, now).is_none());
            emoji.finish(
                key,
                Some(Emoji {
                    still: still.path.clone(),
                    sticker: StickerPicture {
                        kind: StickerKind::Animated,
                        dimensions: None,
                        file: file(id),
                        still: Some(still),
                        premium_effect: None,
                        skin_tone: None,
                        set: None,
                    },
                }),
                now,
            );
        }
        let mut reactions = HashMap::new();
        for id in 0..128 {
            reactions.insert(
                format!("reaction-{id}"),
                Some(ReactionAnimations {
                    appear: Some(file(id)),
                    center: Some(file(id)),
                    around: Some(file(id)),
                }),
            );
        }
        let mut userpics = HashMap::new();
        for id in 0..50_000 {
            let mut userpic = Userpic::default();
            assert!(userpic.start().is_some());
            userpics.insert(FileId(id), userpic);
        }
        let file_path_capacity = |file: &File| file.path.as_ref().map_or(0, String::capacity);
        measure(
            &emoji.entries,
            emoji
                .entries
                .values()
                .filter_map(|entry| match entry {
                    EmojiEntry::Ready { value, .. } => value.as_deref(),
                    EmojiEntry::Loading => None,
                })
                .map(|emoji| {
                    size_of::<Emoji>()
                        + emoji.still.as_ref().map_or(0, String::capacity)
                        + file_path_capacity(&emoji.sticker.file)
                        + emoji
                            .sticker
                            .still
                            .iter()
                            .map(file_path_capacity)
                            .sum::<usize>()
                })
                .sum(),
            "emoji",
        );
        assert!(emoji.entries.len() <= EMOJI_CAPACITY);
        measure(
            &reactions,
            reactions
                .iter()
                .map(|(name, animations)| {
                    let animations = animations.as_ref().expect("synthetic reaction is loaded");
                    name.capacity()
                        + [&animations.appear, &animations.center, &animations.around]
                            .into_iter()
                            .flatten()
                            .map(file_path_capacity)
                            .sum::<usize>()
                })
                .sum(),
            "reactions",
        );
        measure(&userpics, 0, "userpic download tracking");
    }

    fn measure<K, V>(map: &HashMap<K, V>, payload_bytes: usize, label: &str) {
        let slot_bytes = map.capacity() * size_of::<(K, V)>();
        eprintln!(
            "{label}: {} entries, {} bytes in usable slots and owned payloads",
            map.len(),
            slot_bytes + payload_bytes
        );
    }

    #[test]
    fn emoji_cache_keeps_recent_and_pending_entries_and_fetches_evicted_ones_again() {
        let mut emoji = EmojiCache::default();
        let start = Instant::now();
        let cold = EmojiKey::Animated("❄️".to_owned());
        let hot = EmojiKey::Animated("🔥".to_owned());
        let pending = EmojiKey::Animated("⌛".to_owned());
        for key in [&cold, &hot] {
            assert!(emoji.lookup(key, start).is_none());
            emoji.finish(key.clone(), None, start);
        }
        assert!(emoji.lookup(&pending, start).is_none());
        assert!(matches!(emoji.lookup(&pending, start), Some(None)));
        let now = start + EMOJI_KEEP;
        assert!(matches!(emoji.lookup(&hot, now), Some(None)));
        for id in 0..EMOJI_CAPACITY {
            let key = EmojiKey::Custom(CustomEmojiId(i64::try_from(id).unwrap()));
            assert!(emoji.lookup(&key, now).is_none());
            emoji.finish(key, None, now);
        }
        assert!(matches!(emoji.lookup(&hot, now), Some(None)));
        assert!(matches!(emoji.lookup(&pending, now), Some(None)));
        emoji.finish(pending, None, now);
        assert!(emoji.lookup(&cold, now).is_none());
        emoji.finish(cold.clone(), None, now);
        assert!(matches!(emoji.lookup(&cold, now), Some(None)));
    }

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
