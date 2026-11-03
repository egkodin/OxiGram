//! The one sound playing in the app (`Media::Player::Instance`): a voice or
//! a song from any chat. Histories show it in its row; the bar above the
//! chat shows a song while it plays, whichever chat is open.

mod bar;
mod sound;

use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, Global, Task};
use model::{ChatId, MessageId};

pub(crate) use self::bar::bar;
pub(crate) use self::sound::Sound;

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
    _ticker: Task<()>,
}

// shortcut: one track at a time, nothing follows when it ends; the playlist comes with rows 301-302.
#[derive(Default)]
pub(crate) struct Player {
    playing: Option<Playing>,
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
                        this.playing = None;
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
            _ticker: ticker,
        });
        cx.notify();
    }

    pub(crate) fn stop(&mut self, cx: &mut Context<Self>) {
        self.playing = None;
        cx.notify();
    }
}
