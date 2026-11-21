//! Music files in the history (`HistoryView::Document` for songs): the
//! circle downloads, then plays and pauses; "Performer – Title" above the
//! length, the transfer, or the time played.

use std::rc::Rc;

use gpui::{App, Context};
use i18n::key;
use model::MessageId;
use model::file::File;
use model::message::{Content, Message};

use super::HistoryView;
use super::documents::current;
use super::file_row::{FileRow, Thumb, Title, progress};
use super::video::duration_text;
use crate::file_sizes::{progress_text, size_text};
use crate::lang::{strings, tr};
use crate::player::{Playlist, Song, Sound, Track};
use crate::streaming::Playable;

fn seconds(duration: std::time::Duration) -> i32 {
    i32::try_from(duration.as_secs()).unwrap_or(i32::MAX)
}

impl HistoryView {
    /// A music message's row.
    // shortcut: no seek bar under the name while it plays, and the next song does not follow; with the player bar (rows 301, 302).
    pub(super) fn music(&self, message: &Message, cx: &Context<Self>) -> Option<FileRow> {
        let Content::Audio { name, music, .. } = &message.content else {
            return None;
        };
        let file = current(self, &music.file, cx);
        let playing = self.player.read(cx).sound_of(self.chat, message.id);
        let (thumb, status) = self.sound_status(file, playing, music.duration, cx);
        let weak = cx.weak_entity();
        let (chat, id, file_id) = (message.chat, message.id, file.id);
        let playable = Playable::of(self.store.read(cx).session(), file);
        let (performer, title) = music.parts(name);
        let track = Track::Song { performer, title };
        Some(FileRow {
            title: Title::Name(music.name(name)),
            status,
            is_unread: false,
            thumb,
            on_click: Rc::new(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| match thumb {
                    Thumb::Download => this.add_to_downloads(file_id, (chat, id), cx),
                    Thumb::Cancel { .. } => this.cancel_download(file_id, cx),
                    Thumb::Play | Thumb::Pause | Thumb::Document => {
                        if let Some(file) = &playable {
                            this.toggle_music(id, file, track.clone(), window, cx);
                        }
                    }
                });
            }),
        })
    }

    /// A sound's circle and the line under its name: the time played while
    /// it plays, its length once loaded, the transfer, or what a click loads.
    pub(super) fn sound_status(
        &self,
        file: &File,
        playing: Option<&Sound>,
        duration: i32,
        cx: &App,
    ) -> (Thumb, String) {
        let length = duration_text(duration);
        let can_stream = Playable::of(self.store.read(cx).session(), file).is_some();
        match (&file.path, file.is_downloading, playing) {
            (_, _, Some(player)) => (
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
            (None, false, None) => (
                if can_stream {
                    Thumb::Play
                } else {
                    Thumb::Download
                },
                i18n::fill(
                    &tr(cx, key::lng_duration_and_size),
                    &[("duration", &length), ("size", &size_text(file.size))],
                ),
            ),
        }
    }

    /// The loaded songs of this chat, oldest first.
    fn playlist(&self, cx: &Context<Self>) -> Rc<Playlist> {
        let store = self.store.read(cx);
        let songs = store
            .state()
            .history(self.key())
            .map(|history| {
                history
                    .messages()
                    .iter()
                    .filter_map(|message| Song::of(message))
                    .collect()
            })
            .unwrap_or_default();
        Rc::new(Playlist {
            chat: self.chat,
            session: store.session().clone(),
            songs,
        })
    }

    /// Pauses or resumes the song playing, or starts `message`'s, streamed
    /// while it downloads; a file on disk FFmpeg cannot play opens in the
    /// system player.
    fn toggle_music(
        &mut self,
        message: MessageId,
        file: &Playable,
        track: Track,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.toggle_playing(message, cx) {
            return;
        }
        self.player.update(cx, |player, cx| player.stop(cx));
        let volume = crate::settings::song_volume(cx);
        match (self.stream(file, volume), file.path()) {
            (Some(player), _) => {
                let playlist = self.playlist(cx);
                self.play(message, (track, Some(playlist)), Sound::Music(player), cx);
            }
            (None, Some(path)) => self.open_file(path, window, cx),
            (None, None) => {}
        }
    }
}
