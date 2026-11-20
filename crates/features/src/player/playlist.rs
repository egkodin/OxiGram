//! The songs around the one playing (`Instance::moveInPlaylist`): the chat's
//! music in message order, the next one newer; a song that ends starts the
//! next.

use model::message::{Content, Message};
use model::{ChatId, FileId, MessageId, Session};

use super::Track;

/// A song the player can move to.
#[derive(Debug, Clone)]
pub(crate) struct Song {
    pub message: MessageId,
    pub file: FileId,
    pub track: Track,
    /// In seconds.
    pub duration: i32,
}

impl Song {
    /// `message`'s song; none when it is not music.
    pub(crate) fn of(message: &Message) -> Option<Self> {
        let Content::Audio { name, music, .. } = &message.content else {
            return None;
        };
        let (performer, title) = music.parts(name);
        Some(Self {
            message: message.id,
            file: music.file.id,
            track: Track::Song { performer, title },
            duration: music.duration,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Previous,
    Next,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Repeat {
    #[default]
    Off,
    One,
    All,
}

impl Repeat {
    pub(crate) fn cycle(self) -> Self {
        match self {
            Self::Off => Self::One,
            Self::One => Self::All,
            Self::All => Self::Off,
        }
    }
}

/// The history's loaded songs when one starts, then the chat's shared
/// music around it once that arrives.
#[derive(Debug)]
pub(crate) struct Playlist {
    pub chat: ChatId,
    pub session: Session,
    /// Oldest first.
    pub songs: Vec<Song>,
}

impl Playlist {
    /// The song a `step` away from `message`; none past either end.
    pub(crate) fn neighbour(
        &self,
        message: MessageId,
        step: Step,
        repeat: Repeat,
    ) -> Option<&Song> {
        neighbour(&self.songs, message, step, repeat)
    }

    pub(super) fn after_end(&self, message: MessageId, repeat: Repeat) -> Option<&Song> {
        after_end(&self.songs, message, repeat)
    }
}

fn after_end(songs: &[Song], message: MessageId, repeat: Repeat) -> Option<&Song> {
    if repeat == Repeat::One {
        return songs.iter().find(|song| song.message == message);
    }
    neighbour(songs, message, Step::Next, repeat)
}

fn neighbour(songs: &[Song], message: MessageId, step: Step, repeat: Repeat) -> Option<&Song> {
    let index = songs.iter().position(|song| song.message == message)?;
    let song = match step {
        Step::Previous => index.checked_sub(1).and_then(|index| songs.get(index)),
        Step::Next => songs.get(index + 1),
    };
    song.or_else(|| match (repeat, step) {
        (Repeat::All, Step::Previous) => songs.last(),
        (Repeat::All, Step::Next) => songs.first(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn songs(ids: &[i64]) -> Vec<Song> {
        ids.iter()
            .map(|&id| Song {
                message: MessageId(id),
                file: FileId(1),
                track: Track::Voice {
                    name: String::new(),
                    date: String::new(),
                },
                duration: 0,
            })
            .collect()
    }

    fn at(songs: &[Song], message: i64, step: Step) -> Option<i64> {
        neighbour(songs, MessageId(message), step, Repeat::Off).map(|song| song.message.0)
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

    #[test]
    fn repeat_all_wraps_a_known_playlist_in_both_directions() {
        let songs = songs(&[10, 20, 30]);
        let at = |id, step| {
            neighbour(&songs, MessageId(id), step, Repeat::All).map(|song| song.message.0)
        };
        assert_eq!(at(30, Step::Next), Some(10));
        assert_eq!(at(10, Step::Previous), Some(30));
        assert_eq!(at(20, Step::Next), Some(30));
        assert_eq!(at(15, Step::Next), None);
        assert_eq!(
            after_end(&songs, MessageId(30), Repeat::All).map(|song| song.message),
            Some(MessageId(10))
        );
        assert!(after_end(&songs, MessageId(30), Repeat::Off).is_none());
    }

    #[test]
    fn repeat_one_applies_to_natural_completion_only() {
        let songs = songs(&[10, 20, 30]);
        assert_eq!(
            after_end(&songs, MessageId(20), Repeat::One)
                .unwrap()
                .message,
            MessageId(20)
        );
        assert_eq!(
            neighbour(&songs, MessageId(20), Step::Next, Repeat::One)
                .unwrap()
                .message,
            MessageId(30)
        );
        assert_eq!(Repeat::Off.cycle(), Repeat::One);
        assert_eq!(Repeat::One.cycle(), Repeat::All);
        assert_eq!(Repeat::All.cycle(), Repeat::Off);
    }
}
