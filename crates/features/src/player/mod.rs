//! The one sound playing in the app (`Media::Player::Instance`): a voice or
//! a song from any chat. Histories show it in its row; the bar above the
//! chat shows a song while it plays, whichever chat is open.

mod bar;
mod playlist;
mod sound;

use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, Global, Task};
use model::{ChatId, MessageId};

pub(crate) use self::bar::bar;
pub(crate) use self::playlist::{Playlist, Song, Step};
pub(crate) use self::sound::Sound;

/// The download priority of the song moved to, as a click's.
const SKIP_PRIORITY: i32 = 32;

/// How often the rows and the bar move while something plays.
const TICK: Duration = Duration::from_millis(50);

/// What plays, for the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Track {
    Voice,
    /// The performer, maybe empty, and the title.
    Song {
        performer: String,
        title: String,
    },
}

pub(crate) struct Playing {
    pub chat: ChatId,
    pub message: MessageId,
    pub track: Track,
    pub sound: Sound,
    /// The songs around it; none for a voice.
    playlist: Option<Rc<Playlist>>,
    _ticker: Task<()>,
}

#[derive(Default)]
pub(crate) struct Player {
    playing: Option<Playing>,
    /// The song being fetched to move to.
    skipping: Option<Task<()>>,
}

struct Shared(Entity<Player>);

impl Global for Shared {}

impl Player {
    /// The app's player, made on first use.
    pub(crate) fn global(cx: &mut App) -> Entity<Self> {
        if let Some(shared) = cx.try_global::<Shared>() {
            return shared.0.clone();
        }
        let player = cx.new(|_| Self::default());
        cx.set_global(Shared(player.clone()));
        player
    }

    pub(crate) fn current(&self) -> Option<&Playing> {
        self.playing.as_ref()
    }

    /// `message`'s sound, while it plays or is paused.
    pub(crate) fn sound_of(&self, chat: ChatId, message: MessageId) -> Option<&Sound> {
        self.playing
            .as_ref()
            .filter(|playing| playing.chat == chat && playing.message == message)
            .map(|playing| &playing.sound)
    }

    /// Pauses or resumes `message`'s sound; false when it does not play.
    pub(crate) fn toggle(
        &mut self,
        chat: ChatId,
        message: MessageId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(sound) = self.sound_of(chat, message) else {
            return false;
        };
        if sound.is_paused() {
            sound.resume();
        } else {
            sound.pause();
        }
        cx.notify();
        true
    }

    /// Plays `sound`, stopping what played.
    pub(crate) fn play(
        &mut self,
        (chat, message): (ChatId, MessageId),
        track: Track,
        sound: Sound,
        playlist: Option<Rc<Playlist>>,
        cx: &mut Context<Self>,
    ) {
        self.skipping = None;
        self.begin((chat, message), track, sound, playlist, cx);
    }

    /// Whether the playlist goes on a `step` from what plays.
    pub(crate) fn can_step(&self, step: Step) -> bool {
        self.neighbour(step).is_some()
    }

    /// Moves to the previous or next song, once it is on disk.
    pub(crate) fn step(&mut self, step: Step, cx: &mut Context<Self>) {
        if let Some((playlist, song)) = self.neighbour(step) {
            self.fetch(playlist, song, cx);
        }
    }

    fn neighbour(&self, step: Step) -> Option<(Rc<Playlist>, Song)> {
        let playing = self.playing.as_ref()?;
        let playlist = playing.playlist.clone()?;
        let song = playlist.neighbour(playing.message, step)?.clone();
        Some((playlist, song))
    }

    // shortcut: the bar keeps the old song (or hides after the last one ended) until the next is on disk; tdesktop streams it at once (row 259).
    fn fetch(&mut self, playlist: Rc<Playlist>, song: Song, cx: &mut Context<Self>) {
        let fetch = playlist.session.fetch_file(song.file, SKIP_PRIORITY);
        self.skipping = Some(cx.spawn(async move |this, cx| {
            let path = match fetch.await {
                Ok(Some(path)) => path,
                Ok(None) => return,
                Err(error) => {
                    tracing::warn!(%error, "downloading the next song failed");
                    return;
                }
            };
            let sound = match media::AudioPlayer::open(Path::new(&path), Duration::ZERO) {
                Ok(sound) => sound,
                Err(error) => {
                    tracing::warn!(%error, "cannot play the next song");
                    return;
                }
            };
            let key = (playlist.chat, song.message);
            this.update(cx, |this, cx| {
                this.begin(key, song.track, Sound::Music(sound), Some(playlist), cx);
            })
            .ok();
        }));
    }

    /// Ends what played; the next song follows it (`moveInPlaylist(1, true)`).
    fn finish(&mut self, cx: &mut Context<Self>) {
        let next = self.neighbour(Step::Next);
        self.playing = None;
        if let Some((playlist, song)) = next {
            self.fetch(playlist, song, cx);
        }
    }

    fn begin(
        &mut self,
        (chat, message): (ChatId, MessageId),
        track: Track,
        sound: Sound,
        playlist: Option<Rc<Playlist>>,
        cx: &mut Context<Self>,
    ) {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let is_over = this.update(cx, |this, cx| {
                    let is_over = this
                        .playing
                        .as_ref()
                        .is_none_or(|playing| playing.sound.is_finished());
                    if is_over {
                        this.finish(cx);
                    }
                    cx.notify();
                    is_over
                });
                if is_over.unwrap_or(true) {
                    break;
                }
            }
        });
        self.playing = Some(Playing {
            chat,
            message,
            track,
            sound,
            playlist,
            _ticker: ticker,
        });
        cx.notify();
    }

    pub(crate) fn stop(&mut self, cx: &mut Context<Self>) {
        self.playing = None;
        self.skipping = None;
        cx.notify();
    }
}
