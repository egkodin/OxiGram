//! Voice messages in the history (`HistoryDocument` for voices): the circle
//! downloads, then plays and pauses; the waveform fills as it plays and a
//! click on it seeks; below, the length or the time played, with a dot
//! until someone listened.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Context, Task, Window};
use i18n::key;
use model::MessageId;
use model::message::{Content, Message};

use super::HistoryView;
use super::documents::{CLICKED_PRIORITY, current};
use super::file_row::{FileRow, Thumb, Title, progress};
use super::video::duration_text;
use super::waveform::{OnSeek, Waveform};
use crate::file_sizes::{progress_text, size_text};
use crate::lang::{strings, tr};
use crate::store::spawn;

/// How often the waveform and the time move while playing.
const TICK: Duration = Duration::from_millis(50);

/// The voice playing in this history.
// shortcut: playback stops with the history and the next voice does not follow; with the player bar (rows 301, 304).
pub(super) struct Playing {
    message: MessageId,
    player: media::Player,
    _ticker: Task<()>,
}

fn seconds(duration: Duration) -> i32 {
    i32::try_from(duration.as_secs()).unwrap_or(i32::MAX)
}

impl HistoryView {
    /// A voice message's row once it is sent.
    pub(super) fn voice(&self, message: &Message, cx: &Context<Self>) -> Option<FileRow> {
        let Content::Voice { note, .. } = &message.content else {
            return None;
        };
        let file = current(self, &note.file, cx);
        let playing = self
            .playing
            .as_ref()
            .filter(|playing| playing.message == message.id);
        let length = duration_text(note.duration);
        let (thumb, status) = match (&file.path, file.is_downloading, playing) {
            (Some(_), _, Some(Playing { player, .. })) => (
                if player.is_paused() {
                    Thumb::Play
                } else {
                    Thumb::Pause
                },
                i18n::fill(
                    &tr(cx, key::lng_duration_played),
                    &[
                        ("played", &duration_text(seconds(player.position()))),
                        ("duration", &duration_text(seconds(player.duration()))),
                    ],
                ),
            ),
            (Some(_), _, None) => (Thumb::Play, length),
            (None, true, _) => (
                Thumb::Cancel {
                    progress: progress(file.downloaded, file.size),
                },
                progress_text(strings(cx), file.downloaded, file.size),
            ),
            (None, false, _) => (
                Thumb::Download,
                i18n::fill(
                    &tr(cx, key::lng_duration_and_size),
                    &[("duration", &length), ("size", &size_text(file.size))],
                ),
            ),
        };
        let is_unheard = !message.is_outgoing && !note.is_listened;
        let shown = match playing {
            Some(playing) => playing.player.progress(),
            // An incoming voice nobody played shows in the played color.
            None if is_unheard => 1.0,
            None => 0.0,
        };
        let weak = cx.weak_entity();
        let on_seek = playing.map(|_| -> OnSeek {
            let weak = weak.clone();
            Rc::new(move |share, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if let Some(playing) = &this.playing {
                        playing.player.seek(share);
                    }
                    cx.notify();
                });
            })
        });
        let (id, file_id) = (message.id, file.id);
        let path = file.path.clone().map(PathBuf::from);
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
                        if let Some(path) = &path {
                            if is_unheard {
                                this.mark_listened(id, cx);
                            }
                            this.toggle_voice(id, path.clone(), window, cx);
                        }
                    }
                });
            }),
        })
    }

    fn mark_listened(&self, message: MessageId, cx: &mut Context<Self>) {
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

    /// Pauses or resumes the voice playing, or starts `message`'s.
    fn toggle_voice(
        &mut self,
        message: MessageId,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(playing) = self.playing.as_ref().filter(|p| p.message == message) {
            if playing.player.is_paused() {
                playing.player.resume();
            } else {
                playing.player.pause();
            }
            cx.notify();
            return;
        }
        self.playing = None;
        let decoding = cx.background_executor().spawn({
            let path = path.clone();
            async move { media::decode(&path) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let decoded = decoding.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let player = decoded
                    .inspect_err(|error| tracing::warn!(%error, "decoding a voice failed"))
                    .ok()
                    .and_then(|decoded| {
                        media::Player::start(&decoded)
                            .inspect_err(|error| tracing::warn!(%error, "no audio output"))
                            .ok()
                    });
                match player {
                    Some(player) => this.play(message, player, cx),
                    // MP3 and M4A voices, or no output: the system player takes them.
                    None => this.open_file(&path, window, cx),
                }
            });
        })
        .detach();
    }

    fn play(&mut self, message: MessageId, player: media::Player, cx: &mut Context<Self>) {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let is_over = this.update(cx, |this, cx| {
                    let is_over = this
                        .playing
                        .as_ref()
                        .is_none_or(|playing| playing.player.is_finished());
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
            message,
            player,
            _ticker: ticker,
        });
        cx.notify();
    }
}
