//! Where FFmpeg reads a file from: its path once it is complete, or a
//! [`Stream`] through custom I/O while it downloads. Both decoders open
//! their input here, so the rest of their code is the same for both.

use std::path::PathBuf;
use std::time::Duration;

use ffmpeg::format::context::{Input, StreamIo};
use ffmpeg_next as ffmpeg;

use crate::stream::{Signal, Stream};

/// Seconds of the file the download keeps ahead of the reader
/// (tdesktop's streaming loader keeps a few parts past the playhead).
const READ_AHEAD_TIME: Duration = Duration::from_secs(5);
/// Bounds on that read-ahead, for files whose rate is odd or unknown.
const MIN_READ_AHEAD: u64 = 256 << 10;
const MAX_READ_AHEAD: u64 = 16 << 20;
/// FFmpeg's custom I/O buffer: the bytes held in memory between reads.
const IO_BUFFER: usize = 64 << 10;

#[derive(Debug)]
pub(crate) enum Origin {
    Path(PathBuf),
    Stream(Stream),
}

impl Origin {
    /// What wakes and closes the stream; none for a path.
    pub(crate) fn signal(&self) -> Option<Signal> {
        match self {
            Self::Path(_) => None,
            Self::Stream(stream) => Some(stream.signal().clone()),
        }
    }

    /// Opens the input and reads its streams; a stream then reads ahead by
    /// a few seconds of the file's rate.
    pub(crate) fn open(self) -> Result<Input, ffmpeg::Error> {
        match self {
            Self::Path(path) => {
                // ffmpeg-next's path adapter unwraps both UTF-8 and CString conversion.
                let path = path
                    .to_str()
                    .filter(|s| !s.as_bytes().contains(&0))
                    .ok_or(ffmpeg::Error::InvalidData)?;
                ffmpeg::format::input(path)
            }
            Self::Stream(stream) => {
                let (signal, size) = (stream.signal().clone(), stream.size());
                let io = StreamIo::from_read_seek_with_capacity(stream, IO_BUFFER)?;
                let input = ffmpeg::format::input_from_stream(io, None, None)?;
                let duration =
                    u64::try_from(input.duration()).map_or(Duration::ZERO, Duration::from_micros);
                signal.set_read_ahead(read_ahead(size, duration));
                Ok(input)
            }
        }
    }
}

/// Whether the decoder produced a frame on this pass.
pub(crate) fn frame_ready(result: Result<(), ffmpeg::Error>) -> Result<bool, ffmpeg::Error> {
    match result {
        Ok(()) => Ok(true),
        Err(
            ffmpeg::Error::Eof
            | ffmpeg::Error::Other {
                errno: ffmpeg::error::EAGAIN,
            },
        ) => Ok(false),
        Err(error) => Err(error),
    }
}

/// `READ_AHEAD_TIME` of a file of `size` bytes lasting `duration`.
fn read_ahead(size: u64, duration: Duration) -> u64 {
    let seconds = duration.as_secs_f64();
    if seconds <= 0.0 {
        return MIN_READ_AHEAD;
    }
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a byte count well inside f64 and u64"
    )]
    let bytes = (size as f64 / seconds * READ_AHEAD_TIME.as_secs_f64()) as u64;
    bytes.clamp(MIN_READ_AHEAD, MAX_READ_AHEAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_that_cannot_be_passed_to_ffmpeg_returns_an_error() {
        assert!(
            Origin::Path(PathBuf::from("test\0clip.mp4"))
                .open()
                .is_err()
        );
    }

    #[test]
    fn decoder_errors_are_distinct_from_a_drained_queue() {
        assert_eq!(frame_ready(Ok(())), Ok(true));
        assert_eq!(frame_ready(Err(ffmpeg::Error::Eof)), Ok(false));
        assert_eq!(
            frame_ready(Err(ffmpeg::Error::Other {
                errno: ffmpeg::error::EAGAIN
            })),
            Ok(false)
        );
        assert_eq!(
            frame_ready(Err(ffmpeg::Error::InvalidData)),
            Err(ffmpeg::Error::InvalidData)
        );
    }

    #[test]
    fn read_ahead_is_a_few_seconds_within_bounds() {
        let minute = Duration::from_secs(60);
        assert_eq!(read_ahead(60 << 20, minute), 5 << 20, "1 MiB/s for 5 s");
        assert_eq!(read_ahead(1 << 10, minute), MIN_READ_AHEAD);
        assert_eq!(read_ahead(u64::MAX >> 8, minute), MAX_READ_AHEAD);
        assert_eq!(read_ahead(60 << 20, Duration::ZERO), MIN_READ_AHEAD);
    }
}
