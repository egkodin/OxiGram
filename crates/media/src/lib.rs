//! Voice messages as tdesktop records them (`Media::Capture`): the default
//! microphone, mono at 48 kHz, faded in after the first 400 ms, Opus at
//! 32 kbit/s in an OGG file, with the 5-bit waveform Telegram shows.

mod capture;
mod encode;
mod resample;
mod waveform;

pub use capture::{Progress, RecordError, Recorded, Recorder, Source, Stopping};
