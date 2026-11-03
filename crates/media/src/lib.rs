//! Voice messages as tdesktop records them (`Media::Capture`): the default
//! microphone, mono at 48 kHz, faded in after the first 400 ms, Opus at
//! 32 kbit/s in an OGG file, with the 5-bit waveform Telegram shows; and
//! played back on the default output. Videos decode through FFmpeg on a
//! thread of their own (`video`), and so does the sound of music and
//! videos (`audio`).

mod audio;
mod capture;
mod decode;
mod encode;
mod hw;
mod player;
mod resample;
mod video;
mod waveform;

pub use audio::{AudioError, AudioPlayer, silence_output};
pub use capture::{Progress, RecordError, Recorded, Recorder, Source, Stopping};
pub use decode::{DecodeError, Decoded, decode};
pub use player::{PlayError, Player};
pub use video::{Output, VideoDecoder, VideoError, VideoFrame, VideoInfo};
