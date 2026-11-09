//! Voice messages as tdesktop records them (`Media::Capture`): the chosen
//! microphone, mono at 48 kHz, faded in after the first 400 ms, Opus at
//! 32 kbit/s in an OGG file, with the 5-bit waveform Telegram shows; and
//! played back on the chosen output. Videos decode through FFmpeg on a
//! thread of their own (`video`), and so does the sound of music and
//! videos (`audio`).

mod audio;
mod capture;
mod decode;
pub mod devices;
mod encode;
mod hw;
mod input;
pub mod lottie;
mod player;
mod resample;
pub mod stream;
mod video;
mod waveform;

pub use audio::{AudioError, AudioPlayer, MAX_SPEED, MIN_SPEED, silence_output};
pub use capture::{Progress, RecordError, Recorded, Recorder, Source, Stopping};
pub use decode::{DecodeError, Decoded, decode};
pub use player::{PlayError, Player};
pub use video::{Output, VideoDecoder, VideoError, VideoFrame, VideoInfo};
