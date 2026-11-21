//! The songs around the one playing (`Instance::moveInPlaylist`): the chat's
//! music in message order, the next one newer; a song that ends starts the
//! next.

use std::hash::{BuildHasher as _, RandomState};

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Order {
    #[default]
    Normal,
    Reverse,
    Shuffle,
}

#[derive(Clone, Copy)]
pub(super) enum Traversal<'a> {
    Forward,
    Reverse,
    Shuffled(&'a Shuffle),
}

pub(super) struct Shuffle {
    hasher: RandomState,
    first: (u64, MessageId),
}

impl Shuffle {
    pub(super) fn new(message: MessageId) -> Self {
        let hasher = RandomState::new();
        let first = (hasher.hash_one(message), message);
        Self { hasher, first }
    }

    // Rotate the ranking around the song that started this shuffled pass.
    fn rank(&self, message: MessageId) -> (bool, (u64, MessageId)) {
        let key = (self.hasher.hash_one(message), message);
        (key < self.first, key)
    }

    fn neighbour<'a>(
        &self,
        songs: &'a [Song],
        message: MessageId,
        step: Step,
        repeat: Repeat,
    ) -> Option<&'a Song> {
        if !songs.iter().any(|song| song.message == message) {
            return None;
        }
        let current = self.rank(message);
        let ranked = songs.iter().map(|song| (self.rank(song.message), song));
        let song = match step {
            Step::Previous => ranked
                .clone()
                .filter(|(rank, _)| *rank < current)
                .max_by_key(|(rank, _)| *rank),
            Step::Next => ranked
                .clone()
                .filter(|(rank, _)| *rank > current)
                .min_by_key(|(rank, _)| *rank),
        };
        song.or_else(|| match (repeat, step) {
            (Repeat::All, Step::Previous) => ranked.max_by_key(|(rank, _)| *rank),
            (Repeat::All, Step::Next) => ranked.min_by_key(|(rank, _)| *rank),
            _ => None,
        })
        .map(|(_, song)| song)
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
    pub(super) fn neighbour(
        &self,
        message: MessageId,
        step: Step,
        repeat: Repeat,
        order: Traversal<'_>,
    ) -> Option<&Song> {
        ordered_neighbour(&self.songs, message, step, repeat, order)
    }

    pub(super) fn after_end(
        &self,
        message: MessageId,
        repeat: Repeat,
        order: Traversal<'_>,
    ) -> Option<&Song> {
        after_end(&self.songs, message, repeat, order)
    }
}

fn after_end<'a>(
    songs: &'a [Song],
    message: MessageId,
    repeat: Repeat,
    order: Traversal<'_>,
) -> Option<&'a Song> {
    if repeat == Repeat::One {
        return songs.iter().find(|song| song.message == message);
    }
    ordered_neighbour(songs, message, Step::Next, repeat, order)
}

fn ordered_neighbour<'a>(
    songs: &'a [Song],
    message: MessageId,
    step: Step,
    repeat: Repeat,
    order: Traversal<'_>,
) -> Option<&'a Song> {
    match (order, step) {
        (Traversal::Shuffled(shuffle), step) => shuffle.neighbour(songs, message, step, repeat),
        (Traversal::Reverse, Step::Previous) => {
            chronological_neighbour(songs, message, Step::Next, repeat)
        }
        (Traversal::Reverse, Step::Next) => {
            chronological_neighbour(songs, message, Step::Previous, repeat)
        }
        (Traversal::Forward, step) => chronological_neighbour(songs, message, step, repeat),
    }
}

fn chronological_neighbour(
    songs: &[Song],
    message: MessageId,
    step: Step,
    repeat: Repeat,
) -> Option<&Song> {
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
        ordered_neighbour(
            songs,
            MessageId(message),
            step,
            Repeat::Off,
            Traversal::Forward,
        )
        .map(|song| song.message.0)
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
            ordered_neighbour(&songs, MessageId(id), step, Repeat::All, Traversal::Forward)
                .map(|song| song.message.0)
        };
        assert_eq!(at(30, Step::Next), Some(10));
        assert_eq!(at(10, Step::Previous), Some(30));
        assert_eq!(at(20, Step::Next), Some(30));
        assert_eq!(at(15, Step::Next), None);
        assert_eq!(
            after_end(&songs, MessageId(30), Repeat::All, Traversal::Forward)
                .map(|song| song.message),
            Some(MessageId(10))
        );
        assert!(after_end(&songs, MessageId(30), Repeat::Off, Traversal::Forward).is_none());
    }

    #[test]
    fn repeat_one_applies_to_natural_completion_only() {
        let songs = songs(&[10, 20, 30]);
        assert_eq!(
            after_end(&songs, MessageId(20), Repeat::One, Traversal::Forward)
                .unwrap()
                .message,
            MessageId(20)
        );
        assert_eq!(
            ordered_neighbour(
                &songs,
                MessageId(20),
                Step::Next,
                Repeat::One,
                Traversal::Forward
            )
            .unwrap()
            .message,
            MessageId(30)
        );
        assert_eq!(Repeat::Off.cycle(), Repeat::One);
        assert_eq!(Repeat::One.cycle(), Repeat::All);
        assert_eq!(Repeat::All.cycle(), Repeat::Off);
    }

    #[test]
    fn reverse_plays_earlier_songs_and_wraps_to_the_newest() {
        let songs = songs(&[10, 20, 30]);
        let reverse = |id, step, repeat| {
            ordered_neighbour(&songs, MessageId(id), step, repeat, Traversal::Reverse)
                .map(|song| song.message.0)
        };
        assert_eq!(reverse(20, Step::Next, Repeat::Off), Some(10));
        assert_eq!(reverse(20, Step::Previous, Repeat::Off), Some(30));
        assert_eq!(reverse(10, Step::Next, Repeat::Off), None);
        assert_eq!(reverse(10, Step::Next, Repeat::All), Some(30));
        assert_eq!(reverse(30, Step::Previous, Repeat::All), Some(10));
    }
    #[test]
    fn shuffle_starts_at_the_selected_song_and_walks_each_loaded_song_once() {
        let songs = songs(&[10, 20, 30, 40, 50]);
        let first = MessageId(30);
        let shuffle = Shuffle::new(first);
        let order = Traversal::Shuffled(&shuffle);
        let mut played = vec![first];
        for _ in 1..songs.len() {
            let previous = *played.last().unwrap();
            let next = after_end(&songs, previous, Repeat::Off, order)
                .unwrap()
                .message;
            assert!(!played.contains(&next));
            assert_eq!(
                ordered_neighbour(&songs, next, Step::Previous, Repeat::Off, order)
                    .unwrap()
                    .message,
                previous,
            );
            played.push(next);
        }
        let last = *played.last().unwrap();
        assert!(after_end(&songs, last, Repeat::Off, order).is_none());
        assert_eq!(
            after_end(&songs, last, Repeat::All, order).unwrap().message,
            first
        );
        assert_eq!(
            after_end(&songs, last, Repeat::One, order).unwrap().message,
            last
        );
        assert!(after_end(&songs, MessageId(-1), Repeat::All, order).is_none());
    }
}
