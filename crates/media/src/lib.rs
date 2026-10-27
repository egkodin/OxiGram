//! Voice messages as tdesktop records them (`Media::Capture`): the default
//! microphone, mono at 48 kHz, faded in after the first 400 ms, Opus at
//! 32 kbit/s in an OGG file, with the 5-bit waveform Telegram shows; and
//! played back on the default output.

mod capture;
mod decode;
mod encode;
mod player;
mod resample;
mod waveform;

pub use capture::{Progress, RecordError, Recorded, Recorder, Source, Stopping};
pub use decode::{DecodeError, Decoded, decode};
pub use player::{PlayError, Player};
