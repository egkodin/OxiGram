//! Voice messages in the history (`HistoryDocument` for voices): the circle
//! downloads, then plays and pauses; the waveform fills as it plays and a
//! click on it seeks; below, the length or the time played, with a dot
//! until someone listened.

use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use chrono::Local;
use gpui::{App, Context, Window};
use i18n::key;
use model::MessageId;
use model::chats::Chats;
use model::message::{Content, Message, Sender};

use super::HistoryView;
use super::documents::{CLICKED_PRIORITY, current};
use super::file_row::{FileRow, Thumb, Title};
use super::waveform::{OnSeek, Waveform};
use crate::lang::{strings, tr};
use crate::player::{Playlist, Sound, Track};
use crate::store::spawn;
use crate::streaming::Playable;

/// Who sent a voice or a round and when, for the player bar.
#[derive(Debug, Clone, Copy)]
pub(super) struct Origin {
    sender: Sender,
    is_mine: bool,
    date: i32,
}

impl Origin {
    pub(super) fn of(message: &Message) -> Self {
        Self {
            sender: message.sender,
            is_mine: message.is_outgoing && !message.is_channel_post,
            date: message.date,
        }
    }

    /// "Name" and "today at 18:33" (`FormatVoiceName`).
    // shortcut: an older date reads "05.11.26", not tdesktop's "5 November 2026".
    pub(super) fn track(self, chats: &Chats, cx: &App) -> Track {
        let strings = strings(cx);
        let name = if self.is_mine {
            strings.text(key::lng_from_you).to_owned()
        } else {
            super::service::sender_name(strings, chats, self.sender)
        };
        let date = chrono::DateTime::from_timestamp(i64::from(self.date), 0)
            .map(|date| date.with_timezone(&Local))
            .map_or_else(String::new, |sent| {
                let time = sent.format("%H:%M").to_string();
                let days = Local::now().date_naive() - sent.date_naive();
                match days.num_days() {
                    0 => i18n::fill(&tr(cx, key::lng_player_message_today), &[("time", &time)]),
                    1 => i18n::fill(
                        &tr(cx, key::lng_player_message_yesterday),
                        &[("time", &time)],
                    ),
                    _ => i18n::fill(
                        &tr(cx, key::lng_player_message_date),
                        &[
                            ("date", &sent.format("%d.%m.%y").to_string()),
                            ("time", &time),
                        ],
                    ),
                }
            });
        Track::Voice { name, date }
    }
}

impl HistoryView {
    /// A voice message's row once it is sent.
    pub(super) fn voice(&self, message: &Message, cx: &Context<Self>) -> Option<FileRow> {
        let Content::Voice { note, .. } = &message.content else {
            return None;
        };
        let file = current(self, &note.file, cx);
        let playing = self.player.read(cx).sound_of(self.chat, message.id);
        let (thumb, status) = self.sound_status(file, playing, note.duration, cx);
        let is_unheard = !message.is_outgoing && !note.is_listened;
        let shown = match playing {
            Some(player) => player.progress(),
            // An incoming voice nobody played shows in the played color.
            None if is_unheard => 1.0,
            None => 0.0,
        };
        let weak = cx.weak_entity();
        let origin = Origin::of(message);
        let on_seek = playing.map(|_| -> OnSeek {
            let weak = weak.clone();
            Rc::new(move |share, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if let Some(playing) = this.player.read(cx).current() {
                        playing.sound.seek(share);
                    }
                    cx.notify();
                });
            })
        });
        let (id, file_id) = (message.id, file.id);
        let playable = Playable::of(self.store.read(cx).session(), file);
        Some(FileRow {
            title: Title::Waveform(Waveform {
                values: note.waveform.clone(),
                progress: shown,
                on_seek,
            }),
            status,
            is_unread: !note.is_listened,
            thumb,
            on_click: Rc::new(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| match thumb {
                    Thumb::Download => this.start_download(file_id, CLICKED_PRIORITY, cx),
                    Thumb::Cancel { .. } => this.cancel_download(file_id, cx),
                    Thumb::Play | Thumb::Pause | Thumb::Document => {
                        if let Some(file) = &playable {
                            if is_unheard {
                                this.mark_listened(id, cx);
                            }
                            this.toggle_voice((id, origin), file.clone(), window, cx);
                        }
                    }
                });
            }),
        })
    }

    pub(super) fn mark_listened(&self, message: MessageId, cx: &mut Context<Self>) {
        let request = self
            .store
            .read(cx)
            .session()
            .open_message_content(self.chat, message);
        let task = spawn(request, cx);
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "marking a voice listened failed");
            }
        })
        .detach();
    }

    /// Pauses or resumes the voice playing, or starts `message`'s; one
    /// still downloading streams.
    fn toggle_voice(
        &mut self,
        (message, origin): (MessageId, Origin),
        file: Playable,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.toggle_playing(message, cx) {
            return;
        }
        self.player.update(cx, |player, cx| player.stop(cx));
        let track = origin.track(&self.store.read(cx).state().chats, cx);
        let Some(path) = file.path().map(Path::to_path_buf) else {
            if let Some(player) = self.stream(&file) {
                self.play(message, (track, None), Sound::Music(player), cx);
            }
            return;
        };
        let decoding = cx.background_executor().spawn({
            let path = path.clone();
            async move { media::decode(&path) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let decoded = decoding.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let player = decoded
                    .inspect_err(|error| tracing::debug!(%error, "not an Opus voice"))
                    .ok()
                    .and_then(|decoded| {
                        media::Player::start(&decoded)
                            .inspect_err(|error| tracing::warn!(%error, "no audio output"))
                            .ok()
                    })
                    .map(Sound::Voice)
                    // MP3 and M4A voices stream as music do.
                    .or_else(|| this.stream(&file).map(Sound::Music));
                match player {
                    Some(player) => this.play(message, (track, None), player, cx),
                    None => this.open_file(&path, window, cx),
                }
            });
        })
        .detach();
    }

    /// Pauses or resumes `message`'s sound; false when it does not play.
    pub(super) fn toggle_playing(&mut self, message: MessageId, cx: &mut Context<Self>) -> bool {
        let chat = self.chat;
        self.player
            .update(cx, |player, cx| player.toggle(chat, message, cx))
    }

    /// `file`'s sound from its start.
    pub(super) fn stream(&self, file: &Playable) -> Option<media::AudioPlayer> {
        file.audio(Duration::ZERO)
            .inspect_err(|error| tracing::warn!(%error, "cannot play the file"))
            .ok()
    }

    /// Plays `sound` in the app's player, stopping what played.
    pub(super) fn play(
        &mut self,
        message: MessageId,
        (track, playlist): (Track, Option<Rc<Playlist>>),
        sound: Sound,
        cx: &mut Context<Self>,
    ) {
        let chat = self.chat;
        self.player.update(cx, |player, cx| {
            player.play((chat, message), track, sound, playlist, cx)
        });
    }
}
