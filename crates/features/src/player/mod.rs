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
use i18n::key;
use model::profile::SharedMedia;
use model::{ChatId, MessageId};

pub(crate) use self::bar::bar;
pub(crate) use self::playlist::{Order, Playlist, Repeat, Song, Step};
use self::playlist::{Shuffle, Traversal};
pub(crate) use self::sound::{Command, RoundLink, Sound};
use crate::streaming::Playable;

/// How many songs around the one playing the playlist asks for
/// (`kPlaylistIdsLimit`).
const PLAYLIST_LIMIT: i32 = 32;

/// How often the rows and the bar move while something plays.
const TICK: Duration = Duration::from_millis(50);

/// What plays, for the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Track {
    /// A voice or a round: the sender and when it was sent
    /// (`FormatVoiceName`).
    Voice { name: String, date: String },
    /// The performer, maybe empty, and the title.
    Song { performer: String, title: String },
}

impl Track {
    pub(crate) fn speed_kind(&self) -> crate::speed::Kind {
        match self {
            Self::Voice { .. } => crate::speed::Kind::Voice,
            Self::Song { .. } => crate::speed::Kind::Audio,
        }
    }
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
    shuffle: Option<Shuffle>,
    hover: panel::Hover,
    /// The player presets, open at a point.
    control_menu: Option<(
        gpui::Point<gpui::Pixels>,
        Entity<ui::PopupMenu>,
        gpui::Subscription,
    )>,
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
        self.shuffle = (playlist.is_some() && crate::settings::song_order(cx) == Order::Shuffle)
            .then(|| Shuffle::new(message));
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
    pub(crate) fn can_step(&self, step: Step, cx: &App) -> bool {
        self.neighbour(step, cx).is_some()
    }

    /// Moves to the previous or next song, once it is on disk.
    pub(crate) fn step(&mut self, step: Step, cx: &mut Context<Self>) {
        if let Some((playlist, song)) = self.neighbour(step, cx) {
            let (playlist, song) = (playlist.clone(), song.clone());
            self.fetch(playlist, song, cx);
        }
    }

    fn neighbour(&self, step: Step, cx: &App) -> Option<(&Rc<Playlist>, &Song)> {
        let playing = self.playing.as_ref()?;
        let playlist = playing.playlist.as_ref()?;
        let repeat = crate::settings::song_repeat(cx);
        let song = playlist.neighbour(playing.message, step, repeat, self.traversal(cx))?;
        Some((playlist, song))
    }

    fn traversal(&self, cx: &App) -> Traversal<'_> {
        match (crate::settings::song_order(cx), &self.shuffle) {
            (Order::Shuffle, Some(shuffle)) => Traversal::Shuffled(shuffle),
            (Order::Reverse, _) => Traversal::Reverse,
            _ => Traversal::Forward,
        }
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
            let volume = cx.update(|cx| crate::settings::song_volume(cx));
            let sound = match file.audio(Duration::ZERO, volume) {
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
        self.control_menu = None;
        let next = self.playing.take().and_then(|playing| {
            let playlist = playing.playlist?;
            let repeat = match (&playing.track, &playing.sound) {
                (Track::Song { .. }, Sound::Music(audio)) if audio.reached_end() => {
                    crate::settings::song_repeat(cx)
                }
                _ => Repeat::Off,
            };
            let song = playlist
                .after_end(playing.message, repeat, self.traversal(cx))?
                .clone();
            Some((playlist, song))
        });
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
        sound.set_speed(crate::speed::current(track.speed_kind(), cx));
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

    /// What plays plays at this speed.
    pub(crate) fn speed(&self, cx: &App) -> f32 {
        self.playing.as_ref().map_or(1.0, |playing| {
            crate::speed::current(playing.track.speed_kind(), cx)
        })
    }

    /// The speed presets at the pointer (`SpeedController`'s dropdown).
    pub(crate) fn show_speed_menu(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let on_choose: crate::speed::OnChoose = std::rc::Rc::new(move |speed, _, cx| {
            let _ = weak.update(cx, |player, cx| {
                player.change_speed(|kept| kept.chosen(speed), cx);
            });
        });
        let current = self.speed(cx);
        let menu = cx.new(|cx| crate::speed::menu(current, &on_choose, cx));
        self.open_control_menu(menu, window, cx);
    }

    // shortcut: five presets, with a continuous slider when the player controls support one.
    pub(crate) fn show_volume_menu(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let current = crate::settings::song_volume(cx);
        let menu = cx.new(|cx| {
            let mut menu = ui::PopupMenu::new(&style::widgets::POPUP_MENU_WITH_ICONS, cx);
            for level in [0, 25, 50, 75, 100] {
                let weak = weak.clone();
                let label = if level == current {
                    format!("✓ {level}%")
                } else {
                    format!("{level}%")
                };
                menu.option(label, None, move |_, cx| {
                    let _ = weak.update(cx, |player, cx| player.change_volume(level, cx));
                });
            }
            menu
        });
        self.open_control_menu(menu, window, cx);
    }

    pub(super) fn show_order_menu(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        use style::media_player::{
            MEDIA_PLAYER_ORDER_ICON_REVERSE, MEDIA_PLAYER_ORDER_ICON_REVERSE_ACTIVE,
            MEDIA_PLAYER_ORDER_ICON_SHUFFLE, MEDIA_PLAYER_ORDER_ICON_SHUFFLE_ACTIVE,
        };
        let weak = cx.weak_entity();
        let current = crate::settings::song_order(cx);
        let menu = cx.new(|cx| {
            let mut menu = ui::PopupMenu::new(&style::widgets::POPUP_MENU_WITH_ICONS, cx);
            for (order, text, icon, active) in [
                (
                    Order::Reverse,
                    key::lng_audio_player_reverse,
                    MEDIA_PLAYER_ORDER_ICON_REVERSE,
                    MEDIA_PLAYER_ORDER_ICON_REVERSE_ACTIVE,
                ),
                (
                    Order::Shuffle,
                    key::lng_audio_player_shuffle,
                    MEDIA_PLAYER_ORDER_ICON_SHUFFLE,
                    MEDIA_PLAYER_ORDER_ICON_SHUFFLE_ACTIVE,
                ),
            ] {
                let weak = weak.clone();
                let selected = order == current;
                let target = if selected { Order::Normal } else { order };
                menu.option(
                    crate::lang::tr(cx, text),
                    Some(if selected { active } else { icon }),
                    move |_, cx| {
                        let _ = weak.update(cx, |player, cx| player.change_order(target, cx));
                    },
                );
            }
            menu
        });
        self.open_control_menu(menu, window, cx);
    }

    fn change_order(&mut self, order: Order, cx: &mut Context<Self>) {
        self.shuffle = self
            .playing
            .as_ref()
            .filter(|playing| order == Order::Shuffle && playing.playlist.is_some())
            .map(|playing| Shuffle::new(playing.message));
        crate::settings::set_song_order(cx, order);
        cx.notify();
    }

    fn open_control_menu(
        &mut self,
        menu: Entity<ui::PopupMenu>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let subscription = cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
            this.control_menu = None;
            cx.notify();
        });
        gpui::Focusable::focus_handle(menu.read(cx), cx).focus(window, cx);
        self.control_menu = Some((window.mouse_position(), menu, subscription));
        cx.notify();
    }

    pub(crate) fn change_volume(&mut self, volume: u8, cx: &mut Context<Self>) {
        if let Some(Playing {
            track: Track::Song { .. },
            sound: Sound::Music(audio),
            ..
        }) = &self.playing
        {
            audio.set_volume(volume);
            crate::settings::set_song_volume(cx, volume);
            cx.notify();
        }
    }

    pub(crate) fn control_menu_overlay(&self) -> Option<gpui::AnyElement> {
        let (position, menu, _) = self.control_menu.as_ref()?;
        Some(crate::speed::overlay(*position, menu.clone()))
    }

    /// Saves `edit` of the speed for what plays and applies it.
    pub(crate) fn change_speed(
        &mut self,
        edit: impl FnOnce(crate::speed::Speed) -> crate::speed::Speed,
        cx: &mut Context<Self>,
    ) {
        let Some(playing) = &self.playing else {
            return;
        };
        let speed = crate::speed::change(playing.track.speed_kind(), edit, cx);
        playing.sound.set_speed(speed);
        cx.notify();
    }

    pub(crate) fn stop(&mut self, cx: &mut Context<Self>) {
        self.control_menu = None;
        self.playing = None;
        self.skipping = None;
        self.widening = None;
        self.shuffle = None;
        self.reset_hover();
        cx.notify();
    }
}
