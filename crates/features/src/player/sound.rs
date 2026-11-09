//! What plays in a history's one audio slot: a voice decoded whole,
//! music streamed from its file, or a round video playing in its history.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

pub(crate) enum Sound {
    Voice(media::Player),
    Music(media::AudioPlayer),
    /// tdesktop plays rounds as voices (`AudioMsgId::Type::Voice`).
    Round(Rc<RoundLink>),
}

/// What the player asks of a round video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Command {
    Pause,
    Resume,
    /// To this share of the way in.
    Seek(f32),
    Speed(f32),
}

/// A round video playing in its history: the history's playback reports
/// where it is and takes the player's commands at its next frame. Once the
/// player lets go of it, the round stops.
#[derive(Debug, Default)]
pub(crate) struct RoundLink {
    position: Cell<Duration>,
    duration: Cell<Duration>,
    is_paused: Cell<bool>,
    is_finished: Cell<bool>,
    commands: RefCell<Vec<Command>>,
}

impl RoundLink {
    /// The playback's state, each frame: where, how long, and whether
    /// paused and ended.
    pub(crate) fn report(
        &self,
        position: Duration,
        duration: Duration,
        (is_paused, is_finished): (bool, bool),
    ) {
        self.position.set(position);
        self.duration.set(duration);
        self.is_paused.set(is_paused);
        self.is_finished.set(is_finished);
    }

    /// The round's playback is gone.
    pub(crate) fn end(&self) {
        self.is_finished.set(true);
    }

    /// What the player asked since the last frame.
    pub(crate) fn take_commands(&self) -> Vec<Command> {
        std::mem::take(&mut *self.commands.borrow_mut())
    }

    fn ask(&self, command: Command) {
        self.commands.borrow_mut().push(command);
    }
}

impl Sound {
    pub(crate) fn is_paused(&self) -> bool {
        match self {
            Self::Voice(player) => player.is_paused(),
            Self::Music(player) => player.is_paused(),
            Self::Round(link) => link.is_paused.get(),
        }
    }

    pub(crate) fn pause(&self) {
        match self {
            Self::Voice(player) => player.pause(),
            Self::Music(player) => player.pause(),
            Self::Round(link) => {
                link.is_paused.set(true);
                link.ask(Command::Pause);
            }
        }
    }

    pub(crate) fn resume(&self) {
        match self {
            Self::Voice(player) => player.resume(),
            Self::Music(player) => player.resume(),
            Self::Round(link) => {
                link.is_paused.set(false);
                link.ask(Command::Resume);
            }
        }
    }

    pub(crate) fn position(&self) -> Duration {
        match self {
            Self::Voice(player) => player.position(),
            Self::Music(player) => player.position(),
            Self::Round(link) => link.position.get(),
        }
    }

    pub(crate) fn duration(&self) -> Duration {
        match self {
            Self::Voice(player) => player.duration(),
            Self::Music(player) => player.duration(),
            Self::Round(link) => link.duration.get(),
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        match self {
            Self::Voice(player) => player.is_finished(),
            Self::Music(player) => player.is_finished(),
            Self::Round(link) => link.is_finished.get(),
        }
    }

    /// How much is played, 0 to 1.
    pub(crate) fn progress(&self) -> f32 {
        match self {
            Self::Voice(player) => player.progress(),
            Self::Music(player) => share(player.position(), player.duration()),
            Self::Round(link) => share(link.position.get(), link.duration.get()),
        }
    }

    /// Plays on from `share` of the way in.
    pub(crate) fn seek(&self, share: f32) {
        match self {
            Self::Voice(player) => player.seek(share),
            Self::Music(player) => player.seek(player.duration().mul_f32(share.clamp(0.0, 1.0))),
            Self::Round(link) => link.ask(Command::Seek(share)),
        }
    }

    pub(crate) fn set_speed(&self, speed: f32) {
        match self {
            Self::Voice(player) => player.set_speed(speed),
            Self::Music(player) => player.set_speed(speed),
            Self::Round(link) => link.ask(Command::Speed(speed)),
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
    fn a_round_takes_the_commands_once() {
        let link = Rc::new(RoundLink::default());
        let sound = Sound::Round(Rc::clone(&link));
        sound.pause();
        sound.set_speed(2.0);
        assert!(sound.is_paused());
        assert_eq!(link.take_commands(), [Command::Pause, Command::Speed(2.0)]);
        assert!(link.take_commands().is_empty());
        link.report(
            Duration::from_secs(1),
            Duration::from_secs(4),
            (false, false),
        );
        assert!((sound.progress() - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn a_share_stays_within_the_track() {
        let second = Duration::from_secs(1);
        assert!((share(second, 4 * second) - 0.25).abs() < f32::EPSILON);
        assert!((share(8 * second, 4 * second) - 1.0).abs() < f32::EPSILON);
        assert!(share(second, Duration::ZERO).abs() < f32::EPSILON);
    }
}
