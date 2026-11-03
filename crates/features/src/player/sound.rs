//! What plays in a history's one audio slot: a voice decoded whole, or
//! music streamed from its file.

use std::time::Duration;

pub(crate) enum Sound {
    Voice(media::Player),
    Music(media::AudioPlayer),
}

impl Sound {
    pub(crate) fn is_paused(&self) -> bool {
        match self {
            Self::Voice(player) => player.is_paused(),
            Self::Music(player) => player.is_paused(),
        }
    }

    pub(crate) fn pause(&self) {
        match self {
            Self::Voice(player) => player.pause(),
            Self::Music(player) => player.pause(),
        }
    }

    pub(crate) fn resume(&self) {
        match self {
            Self::Voice(player) => player.resume(),
            Self::Music(player) => player.resume(),
        }
    }

    pub(crate) fn position(&self) -> Duration {
        match self {
            Self::Voice(player) => player.position(),
            Self::Music(player) => player.position(),
        }
    }

    pub(crate) fn duration(&self) -> Duration {
        match self {
            Self::Voice(player) => player.duration(),
            Self::Music(player) => player.duration(),
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        match self {
            Self::Voice(player) => player.is_finished(),
            Self::Music(player) => player.is_finished(),
        }
    }

    /// How much is played, 0 to 1.
    pub(crate) fn progress(&self) -> f32 {
        match self {
            Self::Voice(player) => player.progress(),
            Self::Music(player) => share(player.position(), player.duration()),
        }
    }

    /// Plays on from `share` of the way in.
    pub(crate) fn seek(&self, share: f32) {
        match self {
            Self::Voice(player) => player.seek(share),
            Self::Music(player) => player.seek(player.duration().mul_f32(share.clamp(0.0, 1.0))),
        }
    }
}

fn share(position: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 0.0;
    }
    (position.as_secs_f32() / duration.as_secs_f32()).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_stays_within_the_track() {
        let second = Duration::from_secs(1);
        assert!((share(second, 4 * second) - 0.25).abs() < f32::EPSILON);
        assert!((share(8 * second, 4 * second) - 1.0).abs() < f32::EPSILON);
        assert!(share(second, Duration::ZERO).abs() < f32::EPSILON);
    }
}
