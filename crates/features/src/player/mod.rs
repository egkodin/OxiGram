//! The one sound playing in the app (`Media::Player::Instance`): a voice or
//! a song from any chat. Histories show it in its row; the bar above the
//! chat shows a song while it plays, whichever chat is open.

mod bar;
mod panel;
mod playlist;
mod sound;

use std::rc::Rc;
use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, Global, Task};
use model::profile::SharedMedia;
use model::{ChatId, MessageId};

pub(crate) use self::bar::bar;
pub(crate) use self::playlist::{Playlist, Song, Step};
pub(crate) use self::sound::Sound;
use crate::streaming::Playable;

/// How many songs around the one playing the playlist asks for
/// (`kPlaylistIdsLimit`).
const PLAYLIST_LIMIT: i32 = 32;

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
    /// The chat's shared music being fetched for the playlist.
    widening: Option<Task<()>>,
    hover: panel::Hover,
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

    /// Plays or pauses `song` from the playlist, as a click on it.
    pub(crate) fn choose(&mut self, song: Song, cx: &mut Context<Self>) {
        let Some(playing) = &self.playing else {
            return;
        };
        if playing.message == song.message {
            let chat = playing.chat;
            self.toggle(chat, song.message, cx);
        } else if let Some(playlist) = playing.playlist.clone() {
            self.fetch(playlist, song, cx);
        }
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

    /// Moves to `song`, streamed while it downloads, as tdesktop does.
    fn fetch(&mut self, playlist: Rc<Playlist>, song: Song, cx: &mut Context<Self>) {
        let session = playlist.session.clone();
        let fetch = session.file(song.file);
        self.skipping = Some(cx.spawn(async move |this, cx| {
            let file = match fetch.await {
                Ok(file) => file,
                Err(error) => {
                    tracing::warn!(%error, "looking up the next song failed");
                    return;
                }
            };
            let Some(file) = Playable::of(&session, &file) else {
                tracing::warn!("the next song has no size yet");
                return;
            };
            let sound = match file.audio(Duration::ZERO) {
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
        match next {
            Some((playlist, song)) => self.fetch(playlist, song, cx),
            None => self.reset_hover(),
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
        self.widen(cx);
        cx.notify();
    }

    /// Swaps the history's songs for the chat's shared music around the
    /// one playing (`Panel::refreshList`).
    fn widen(&mut self, cx: &mut Context<Self>) {
        let Some(playing) = &self.playing else {
            return;
        };
        let Some(playlist) = playing.playlist.clone() else {
            return;
        };
        let message = playing.message;
        let around = playlist.session.shared_media_around(
            playlist.chat,
            SharedMedia::Music,
            message,
            PLAYLIST_LIMIT,
        );
        self.widening = Some(cx.spawn(async move |this, cx| {
            let found = match around.await {
                Ok(found) => found,
                Err(error) => {
                    tracing::warn!(%error, "loading the playlist failed");
                    return;
                }
            };
            // Newest first from TDLib; the playlist is oldest first.
            let songs: Vec<Song> = found.iter().rev().filter_map(Song::of).collect();
            if !songs.iter().any(|song| song.message == message) {
                return;
            }
            this.update(cx, |this, cx| {
                let Some(playing) = this.playing.as_mut().filter(|p| p.message == message) else {
                    return;
                };
                playing.playlist = Some(Rc::new(Playlist {
                    chat: playlist.chat,
                    session: playlist.session.clone(),
                    songs,
                }));
                cx.notify();
            })
            .ok();
        }));
    }

    pub(crate) fn stop(&mut self, cx: &mut Context<Self>) {
        self.playing = None;
        self.skipping = None;
        self.widening = None;
        self.reset_hover();
        cx.notify();
    }
}
