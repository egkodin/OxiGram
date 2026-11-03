//! Music files in the history (`HistoryView::Document` for songs): the
//! circle downloads, then plays and pauses; "Performer – Title" above the
//! length, the transfer, or the time played.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::Context;
use i18n::key;
use model::MessageId;
use model::message::{Content, Message};

use super::HistoryView;
use super::documents::current;
use super::file_row::{FileRow, Thumb, Title, progress};
use super::video::duration_text;
use crate::file_sizes::{progress_text, size_text};
use crate::lang::{strings, tr};
use crate::player::{Sound, Track};

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
        let length = duration_text(music.duration);
        let (thumb, status) = match (&file.path, file.is_downloading, playing) {
            (Some(_), _, Some(player)) => (
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
        let weak = cx.weak_entity();
        let (chat, id, file_id) = (message.chat, message.id, file.id);
        let path = file.path.clone().map(PathBuf::from);
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
                        if let Some(path) = &path {
                            this.toggle_music(id, path, track.clone(), window, cx);
                        }
                    }
                });
            }),
        })
    }

    /// Pauses or resumes the song playing, or starts `message`'s; a file
    /// FFmpeg cannot play opens in the system player.
    fn toggle_music(
        &mut self,
        message: MessageId,
        path: &std::path::Path,
        track: Track,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.toggle_playing(message, cx) {
            return;
        }
        self.player.update(cx, |player, cx| player.stop(cx));
        match self.stream(path) {
            Some(player) => self.play(message, track, Sound::Music(player), cx),
            None => self.open_file(path, window, cx),
        }
    }
}
