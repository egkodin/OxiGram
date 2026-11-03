//! The songs around the one playing (`Instance::moveInPlaylist`): the chat's
//! music in message order, the next one newer; a song that ends starts the
//! next.

use model::{ChatId, FileId, MessageId, Session};

use super::Track;

/// A song the player can move to.
#[derive(Debug, Clone)]
pub(crate) struct Song {
    pub message: MessageId,
    pub file: FileId,
    pub track: Track,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Previous,
    Next,
}

// shortcut: only the songs the history had loaded when one started, not the chat's shared music (`searchChatMessages`); when the playlist panel comes (row 301).
#[derive(Debug)]
pub(crate) struct Playlist {
    pub chat: ChatId,
    pub session: Session,
    /// Oldest first.
    pub songs: Vec<Song>,
}

impl Playlist {
    /// The song a `step` away from `message`; none past either end.
    pub(crate) fn neighbour(&self, message: MessageId, step: Step) -> Option<&Song> {
        neighbour(&self.songs, message, step)
    }
}

fn neighbour(songs: &[Song], message: MessageId, step: Step) -> Option<&Song> {
    let index = songs.iter().position(|song| song.message == message)?;
    let target = match step {
        Step::Previous => index.checked_sub(1)?,
        Step::Next => index + 1,
    };
    songs.get(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn songs(ids: &[i64]) -> Vec<Song> {
        ids.iter()
            .map(|&id| Song {
                message: MessageId(id),
                file: FileId(1),
                track: Track::Voice,
            })
            .collect()
    }

    fn at(songs: &[Song], message: i64, step: Step) -> Option<i64> {
        neighbour(songs, MessageId(message), step).map(|song| song.message.0)
    }

    #[test]
    fn the_next_song_is_newer_and_the_ends_stop() {
        let songs = songs(&[10, 20, 30]);
        assert_eq!(at(&songs, 20, Step::Next), Some(30));
        assert_eq!(at(&songs, 20, Step::Previous), Some(10));
        assert_eq!(at(&songs, 30, Step::Next), None);
        assert_eq!(at(&songs, 10, Step::Previous), None);
        assert_eq!(at(&songs, 15, Step::Next), None);
    }
}
