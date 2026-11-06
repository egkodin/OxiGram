//! Lottie animations on screen (`Lottie::MultiPlayer` and its frame cache):
//! one player per file and pixel size, shared by every place that shows
//! it, frames rendered on the renderer thread at the size they show at.
//! Players move only while drawn, in a visible window, and stop and free
//! themselves soon after nothing draws them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, Context, Entity, Global, RenderImage, Subscription, Task, Window,
};
use media::lottie::{Info, Renderer, SkinTone};

/// A player not drawn for this long stops moving (`kNotSeenTimeout`).
const SEEN_GRACE: Duration = Duration::from_millis(250);
/// And this long after, it is closed and its frames freed.
const EVICT_AFTER: Duration = Duration::from_secs(3);
/// At most this many players off screen stay open; a fast scroll through
/// a chat of animated emoji would keep hundreds otherwise.
const MAX_HIDDEN: usize = 32;
/// Cached frames of all players stay under this many bytes...
const CACHE_BUDGET: usize = 32 << 20;
/// ...and one animation is cached only when all its frames fit in this.
const PLAYER_CACHE_LIMIT: usize = 4 << 20;
/// A pause longer than this (a hidden window, a busy thread) does not skip
/// frames; the animation resumes where it stopped.
const MAX_STEP: Duration = Duration::from_millis(100);

/// How a player moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Play {
    Loop,
    /// To the last frame, then stays (animated emoji, effects).
    Once,
    /// The first frame only: what Power Usage leaves of an animation.
    Still,
}

/// One animation at one size; `instance` keeps apart players of the same
/// file that run on their own clocks, such as one emoji per message.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    path: PathBuf,
    width: u32,
    height: u32,
    tone: SkinTone,
    instance: u64,
}

impl Key {
    /// `size` in points, drawn at the window's `scale`.
    pub(crate) fn new(path: &Path, size: (f32, f32), scale: f32, instance: u64) -> Self {
        Self {
            path: path.to_owned(),
            width: device_pixels(size.0, scale),
            height: device_pixels(size.1, scale),
            tone: SkinTone::default(),
            instance,
        }
    }

    pub(crate) fn with_tone(self, tone: SkinTone) -> Self {
        Self { tone, ..self }
    }

    fn bytes_per_frame(&self) -> usize {
        usize::try_from(self.width * self.height * 4).unwrap_or(usize::MAX)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a small positive size"
)]
fn device_pixels(points: f32, scale: f32) -> u32 {
    (points * scale.max(1.0)).round().max(1.0) as u32
}

enum Opened {
    Opening,
    Ready(Info),
    Failed,
}

struct Player {
    opened: Opened,
    play: Play,
    /// Where it is, in frames.
    position: f32,
    shown: Option<u32>,
    image: Option<Arc<RenderImage>>,
    is_rendering: bool,
    is_done: bool,
    seen: Instant,
    /// Every frame once rendered, when the animation fits the budget.
    cache: Option<Vec<Option<Arc<[u8]>>>>,
}

impl Player {
    fn new(play: Play, now: Instant) -> Self {
        Self {
            opened: Opened::Opening,
            play,
            position: 0.0,
            shown: None,
            image: None,
            is_rendering: false,
            is_done: false,
            seen: now,
            cache: None,
        }
    }

    fn info(&self) -> Option<Info> {
        match self.opened {
            Opened::Ready(info) => Some(info),
            Opened::Opening | Opened::Failed => None,
        }
    }

    fn is_moving(&self, now: Instant) -> bool {
        self.info().is_some()
            && self.play != Play::Still
            && !self.is_done
            && now.saturating_duration_since(self.seen) < SEEN_GRACE
    }

    /// The frame it should show now.
    fn due(&self) -> Option<u32> {
        let info = self.info()?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a frame number"
        )]
        let index = self.position.max(0.0).floor() as u32;
        Some(index % info.frames.max(1))
    }

    /// Moves on by `step`; a once player stops on its last frame.
    fn advance(&mut self, step: Duration) {
        let Some(info) = self.info() else {
            return;
        };
        self.position += step.as_secs_f32() * info.frame_rate;
        #[expect(clippy::cast_precision_loss, reason = "frame counts are small")]
        let last = info.frames.saturating_sub(1) as f32;
        if self.play == Play::Once && self.position >= last {
            self.position = last;
            self.is_done = true;
        }
    }
}

/// Every player, with the renderer thread behind them.
pub(crate) struct Players {
    renderer: Option<Renderer>,
    players: HashMap<Key, Player>,
    cached_bytes: usize,
    is_window_visible: bool,
    last_tick: Instant,
    tick: Option<Task<()>>,
    retired: Vec<Arc<RenderImage>>,
    frames_shown: u64,
}

/// What the players hold, for the soak in the snapshots.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub players: usize,
    pub cached_bytes: usize,
    pub frames_shown: u64,
}

#[doc(hidden)]
#[must_use]
pub fn stats(cx: &App) -> Stats {
    cx.try_global::<Shared>()
        .map_or_else(Stats::default, |shared| {
            let players = shared.0.read(cx);
            Stats {
                players: players.players.len(),
                cached_bytes: players.cached_bytes,
                frames_shown: players.frames_shown,
            }
        })
}

struct Shared(Entity<Players>);

impl Global for Shared {}

/// The players every view shares, made on first use.
pub(crate) fn players(cx: &mut App) -> Entity<Players> {
    if let Some(shared) = cx.try_global::<Shared>() {
        return shared.0.clone();
    }
    let renderer = Renderer::spawn()
        .inspect_err(|error| tracing::warn!(%error, "the Lottie thread did not start"))
        .ok();
    let entity = cx.new(|_| Players {
        renderer,
        players: HashMap::new(),
        cached_bytes: 0,
        is_window_visible: true,
        last_tick: Instant::now(),
        tick: None,
        retired: Vec::new(),
        frames_shown: 0,
    });
    cx.set_global(Shared(entity.clone()));
    entity
}

/// Redraws the view whenever a frame it may show changes.
pub(crate) fn observe<V: 'static>(cx: &mut Context<V>) -> Subscription {
    let players = players(cx);
    cx.observe(&players, |_, _, cx| cx.notify())
}

/// The frame of `key` to draw now, None until the first one is rendered;
/// drawing it keeps it moving.
pub(crate) fn frame(key: &Key, play: Play, cx: &mut App) -> Option<Arc<RenderImage>> {
    players(cx).update(cx, |players, cx| players.frame(key, play, cx))
}

/// Plays a once player from its start again (a click on an emoji).
pub(crate) fn replay(key: &Key, cx: &mut App) {
    players(cx).update(cx, |players, cx| {
        if let Some(player) = players.players.get_mut(key) {
            player.position = 0.0;
            player.is_done = false;
            players.schedule(Duration::ZERO, cx);
        }
    });
}

/// Whether the once player of `key` reached its end.
pub(crate) fn is_done(key: &Key, cx: &App) -> bool {
    cx.try_global::<Shared>().is_some_and(|shared| {
        shared
            .0
            .read(cx)
            .players
            .get(key)
            .is_some_and(|player| player.is_done)
    })
}

/// Pauses every player while the window shows nothing.
pub(crate) fn set_window_visible(is_visible: bool, cx: &mut App) {
    players(cx).update(cx, |players, cx| {
        players.is_window_visible = is_visible;
        players.last_tick = Instant::now();
        if is_visible {
            cx.notify();
        }
    });
}

/// Frees the frames players no longer show from the window's atlas.
pub(crate) fn drop_retired(window: &mut Window, cx: &mut App) {
    let Some(shared) = cx.try_global::<Shared>() else {
        return;
    };
    let entity = shared.0.clone();
    let retired = entity.update(cx, |players, _| std::mem::take(&mut players.retired));
    for image in retired {
        if let Err(error) = window.drop_image(image) {
            tracing::warn!(%error, "cannot drop an animation frame");
        }
    }
}

impl Players {
    fn frame(&mut self, key: &Key, play: Play, cx: &mut Context<Self>) -> Option<Arc<RenderImage>> {
        let now = Instant::now();
        let is_new = !self.players.contains_key(key);
        let player = self
            .players
            .entry(key.clone())
            .or_insert_with(|| Player::new(play, now));
        let was_idle = !player.is_moving(now);
        player.seen = now;
        if player.play != play {
            player.play = play;
            player.is_done = false;
        }
        let image = player.image.clone();
        if is_new {
            self.open(key.clone(), cx);
        } else if was_idle {
            self.schedule(Duration::ZERO, cx);
        }
        image
    }

    fn open(&mut self, key: Key, cx: &mut Context<Self>) {
        let Some(renderer) = self.renderer.clone() else {
            self.fail(&key);
            return;
        };
        let answer = renderer.open(key.path.clone(), key.tone);
        cx.spawn(async move |this, cx| {
            let opened = answer.await;
            let _ = this.update(cx, |this, cx| match opened {
                Ok(info) => this.opened(&key, info, cx),
                Err(error) => {
                    tracing::warn!(%error, "opening an animation failed");
                    this.fail(&key);
                }
            });
        })
        .detach();
    }

    fn fail(&mut self, key: &Key) {
        if let Some(player) = self.players.get_mut(key) {
            player.opened = Opened::Failed;
        }
    }

    fn opened(&mut self, key: &Key, info: Info, cx: &mut Context<Self>) {
        let Some(player) = self.players.get_mut(key) else {
            // Evicted while opening.
            if let Some(renderer) = &self.renderer {
                renderer.close(info.id);
            }
            return;
        };
        let frames = usize::try_from(info.frames).unwrap_or(usize::MAX);
        let bytes = frames.saturating_mul(key.bytes_per_frame());
        if bytes <= PLAYER_CACHE_LIMIT && self.cached_bytes + bytes <= CACHE_BUDGET {
            self.cached_bytes += bytes;
            player.cache = Some(vec![None; frames]);
        }
        player.opened = Opened::Ready(info);
        self.schedule(Duration::ZERO, cx);
    }

    /// Runs the next step after `wait`, unless one is already due.
    fn schedule(&mut self, wait: Duration, cx: &mut Context<Self>) {
        if self.tick.is_some() || !self.is_window_visible {
            return;
        }
        self.tick = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| {
                this.tick = None;
                this.step(cx);
            });
        }));
    }

    /// Moves what is drawn, shows or renders the frames now due, frees what
    /// is gone, and waits for the next frame.
    fn step(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let step = now.saturating_duration_since(self.last_tick).min(MAX_STEP);
        self.last_tick = now;
        self.evict(now);
        if !self.is_window_visible {
            return;
        }
        let mut is_changed = false;
        let mut next: Option<Duration> = None;
        let keys: Vec<Key> = self.players.keys().cloned().collect();
        for key in keys {
            let Some(player) = self.players.get_mut(&key) else {
                continue;
            };
            if player.is_moving(now) {
                player.advance(step);
                if let Some(info) = player.info().filter(|_| !player.is_done) {
                    let frame = Duration::from_secs_f32(1.0 / info.frame_rate);
                    next = Some(next.map_or(frame, |next| next.min(frame)));
                }
            }
            is_changed |= self.show_due(&key, cx);
        }
        if is_changed {
            cx.notify();
        }
        match next {
            Some(wait) => self.schedule(wait, cx),
            // Still open but still: close them once they are due, even if
            // nothing draws again (the chat closed).
            None if !self.players.is_empty() => self.schedule(EVICT_AFTER, cx),
            None => {}
        }
    }

    /// Shows the due frame from the cache, or asks the thread for it; true
    /// when the shown image changed.
    fn show_due(&mut self, key: &Key, cx: &mut Context<Self>) -> bool {
        let Some(player) = self.players.get_mut(key) else {
            return false;
        };
        let (Some(due), Some(info)) = (player.due(), player.info()) else {
            return false;
        };
        if player.shown == Some(due) || player.is_rendering {
            return false;
        }
        let cached = player
            .cache
            .as_ref()
            .and_then(|cache| cache.get(usize::try_from(due).ok()?)?.clone());
        if let Some(pixels) = cached {
            self.show(key, due, &pixels);
            return true;
        }
        let Some(renderer) = self.renderer.clone() else {
            return false;
        };
        player.is_rendering = true;
        let answer = renderer.render(info.id, due, key.width, key.height);
        let key = key.clone();
        cx.spawn(async move |this, cx| {
            let rendered = answer.await;
            let _ = this.update(cx, |this, cx| this.rendered(&key, due, rendered, cx));
        })
        .detach();
        false
    }

    fn rendered(
        &mut self,
        key: &Key,
        index: u32,
        rendered: Result<Vec<u8>, media::lottie::LottieError>,
        cx: &mut Context<Self>,
    ) {
        let Some(player) = self.players.get_mut(key) else {
            return;
        };
        player.is_rendering = false;
        let pixels: Arc<[u8]> = match rendered {
            Ok(pixels) => pixels.into(),
            Err(error) => {
                tracing::warn!(%error, "rendering an animation frame failed");
                player.opened = Opened::Failed;
                return;
            }
        };
        if let Some(slot) = player
            .cache
            .as_mut()
            .and_then(|cache| cache.get_mut(usize::try_from(index).ok()?))
        {
            *slot = Some(pixels.clone());
        }
        self.show(key, index, &pixels);
        cx.notify();
        // The frame may be late: what is due now could already be another.
        self.schedule(Duration::ZERO, cx);
    }

    /// Puts frame `index` up in place of the one shown.
    fn show(&mut self, key: &Key, index: u32, pixels: &[u8]) {
        let image = to_image(key, pixels);
        let old = player_image_swap(self.players.get_mut(key), index, image);
        self.retire(old);
        self.frames_shown += 1;
    }

    /// Closes players nothing drew for a while.
    fn evict(&mut self, now: Instant) {
        let seen = self.players.iter().map(|(key, player)| (key, player.seen));
        for key in to_evict(seen, now) {
            let Some(player) = self.players.remove(&key) else {
                continue;
            };
            if let (Some(info), Some(renderer)) = (player.info(), &self.renderer) {
                renderer.close(info.id);
            }
            if let Some(cache) = &player.cache {
                self.cached_bytes = self
                    .cached_bytes
                    .saturating_sub(cache.len() * key.bytes_per_frame());
            }
            self.retired.extend(player.image);
        }
    }

    fn retire(&mut self, old: Option<Arc<RenderImage>>) {
        self.retired.extend(old);
    }
}

/// The players not drawn for `EVICT_AFTER`, and the oldest of those off
/// screen beyond `MAX_HIDDEN`.
fn to_evict<'a>(seen: impl Iterator<Item = (&'a Key, Instant)>, now: Instant) -> Vec<Key> {
    let mut hidden: Vec<(&Key, Instant)> = seen
        .filter(|(_, seen)| now.saturating_duration_since(*seen) >= SEEN_GRACE)
        .collect();
    // Most recently seen first.
    hidden.sort_by_key(|(_, seen)| std::cmp::Reverse(*seen));
    hidden
        .iter()
        .enumerate()
        .filter(|(index, (_, seen))| {
            *index >= MAX_HIDDEN || now.saturating_duration_since(*seen) > EVICT_AFTER
        })
        .map(|(_, (key, _))| (*key).clone())
        .collect()
}

/// Puts `image` up as `index`, handing back the one it replaces.
fn player_image_swap(
    player: Option<&mut Player>,
    index: u32,
    image: Option<Arc<RenderImage>>,
) -> Option<Arc<RenderImage>> {
    let player = player?;
    player.shown = Some(index);
    std::mem::replace(&mut player.image, image)
}

fn to_image(key: &Key, pixels: &[u8]) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(key.width, key.height, pixels.to_vec())?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_off_screen_close_when_due_or_beyond_the_cap() {
        let now = Instant::now();
        let keys: Vec<Key> = (0..40)
            .map(|instance| Key::new(Path::new("a.tgs"), (10.0, 10.0), 1.0, instance))
            .collect();
        let visible = (&keys[0], now);
        let stale = (&keys[1], now.checked_sub(EVICT_AFTER * 2).expect("clock"));
        let hidden = keys[2..].iter().zip(1_u64..).map(|(key, age)| {
            let at = now.checked_sub(SEEN_GRACE + Duration::from_millis(age));
            (key, at.expect("clock"))
        });
        let gone = to_evict([visible, stale].into_iter().chain(hidden), now);
        assert!(!gone.contains(&keys[0]), "what shows stays");
        assert!(gone.contains(&keys[1]), "what was not drawn for long goes");
        // 38 recently hidden: the 32 seen last stay, the oldest 6 go.
        assert_eq!(gone.len(), 1 + 6);
        assert!(gone.contains(&keys[39]) && !gone.contains(&keys[2]));
    }

    fn ready(play: Play, frames: u32) -> Player {
        let mut player = Player::new(play, Instant::now());
        player.opened = Opened::Ready(media::lottie::Info {
            id: test_id(),
            frames,
            frame_rate: 60.0,
        });
        player
    }

    fn test_id() -> media::lottie::AnimationId {
        // Any id: these players never reach the thread.
        media::lottie::AnimationId::for_tests(1)
    }

    #[test]
    fn a_once_player_stops_on_its_last_frame_and_a_loop_wraps() {
        let mut once = ready(Play::Once, 60);
        once.advance(Duration::from_secs(2));
        assert!(once.is_done);
        assert_eq!(once.due(), Some(59));
        let mut looped = ready(Play::Loop, 60);
        looped.advance(Duration::from_millis(1100));
        assert!(!looped.is_done);
        assert_eq!(looped.due(), Some(6));
    }

    #[test]
    fn a_player_not_drawn_lately_stops_and_a_still_never_moves() {
        let mut player = ready(Play::Loop, 60);
        assert!(player.is_moving(Instant::now()));
        player.seen = Instant::now() - SEEN_GRACE * 2;
        assert!(!player.is_moving(Instant::now()));
        assert!(!ready(Play::Still, 60).is_moving(Instant::now()));
    }

    #[test]
    fn keys_count_device_pixels() {
        let key = Key::new(Path::new("a.tgs"), (112.0, 56.0), 2.0, 0);
        assert_eq!((key.width, key.height), (224, 112));
        assert_eq!(key.bytes_per_frame(), 224 * 112 * 4);
    }
}
