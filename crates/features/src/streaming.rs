//! TDLib files streamed to the players (`Media::Streaming::Loader`): a
//! [`media::stream::FileSource`] that asks TDLib for the bytes a decoder
//! reaches and reads the part on disk, and the registry that wakes waiting
//! readers on `updateFile`. The bytes land in TDLib's file cache as a
//! normal download's do, so the file opens at once the next time.

use std::collections::HashMap;
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use media::stream::{FileSource, Signal, Stream};
use media::{AudioError, AudioPlayer, VideoDecoder, VideoError};
use model::FileId;
use model::Session;
use model::file::File;

/// What streams one file: the readers to wake, and the range asked for
/// last, shared so a video's picture and sound do not move TDLib's single
/// download back and forth.
#[derive(Default)]
struct Entry {
    signals: Vec<Signal>,
    window: Arc<Mutex<Option<(u64, u64)>>>,
}

type StreamKey = (model::ClientId, FileId);
type Registry = HashMap<StreamKey, Entry>;

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}

fn register(key: StreamKey, signal: Signal) -> Arc<Mutex<Option<(u64, u64)>>> {
    let mut registry = registry().lock().expect("stream registry lock poisoned");
    let entry = registry.entry(key).or_default();
    entry.signals.retain(|signal| !signal.is_closed());
    entry.signals.push(signal);
    Arc::clone(&entry.window)
}

/// A file to play: complete on disk, or streamed while it downloads.
#[derive(Debug, Clone)]
pub(crate) enum Playable {
    Complete(PathBuf),
    Streaming {
        session: Session,
        file: FileId,
        /// Positive.
        size: u64,
    },
}

impl Playable {
    /// `file` as the store last saw it; `None` while it is not on disk and
    /// its size is not known.
    pub(crate) fn of(session: &Session, file: &File) -> Option<Self> {
        if let Some(path) = &file.path {
            return Some(Self::Complete(PathBuf::from(path)));
        }
        let size = u64::try_from(file.size).ok().filter(|size| *size > 0)?;
        Some(Self::Streaming {
            session: session.clone(),
            file: file.id,
            size,
        })
    }

    /// The path once the file is complete.
    pub(crate) fn path(&self) -> Option<&Path> {
        match self {
            Self::Complete(path) => Some(path),
            Self::Streaming { .. } => None,
        }
    }

    /// Decodes the picture into frames no bigger than `size`.
    pub(crate) fn video(&self, size: (u32, u32)) -> Result<VideoDecoder, VideoError> {
        match self {
            Self::Complete(path) => VideoDecoder::open(path, size),
            Self::Streaming { .. } => VideoDecoder::open_stream(self.stream(), size),
        }
    }

    /// Plays the sound from `start`. A streamed file opens at once and is
    /// silent if it has no sound.
    pub(crate) fn audio(&self, start: Duration) -> Result<AudioPlayer, AudioError> {
        match self {
            Self::Complete(path) => AudioPlayer::open(path, start),
            Self::Streaming { .. } => AudioPlayer::open_stream(self.stream(), start),
        }
    }

    /// A new reader, woken by the file's updates.
    fn stream(&self) -> Stream {
        let Self::Streaming {
            session,
            file,
            size,
        } = self
        else {
            unreachable!("only a streaming file has a stream");
        };
        let signal = Signal::new();
        let window = register((session.client_id(), *file), signal.clone());
        let source = TdFile {
            session: session.clone(),
            file: *file,
            size: *size,
            local: None,
            window,
        };
        Stream::new(source, signal)
    }
}

/// A file update: readers waiting on `file` look again.
pub(crate) fn wake(client: model::ClientId, file: FileId) {
    let key = (client, file);
    let mut registry = registry().lock().expect("stream registry lock poisoned");
    let Some(entry) = registry.get_mut(&key) else {
        return;
    };
    entry.signals.retain(|signal| !signal.is_closed());
    if entry.signals.is_empty() {
        registry.remove(&key);
        return;
    }
    for signal in &entry.signals {
        signal.wake();
    }
}

struct TdFile {
    session: Session,
    file: FileId,
    size: u64,
    /// The downloaded part, opened once TDLib wrote some. TDLib renames it
    /// when the download completes; an open file reads on regardless.
    local: Option<std::fs::File>,
    window: Arc<Mutex<Option<(u64, u64)>>>,
}

/// TDLib's answer, waited for on the decoder's thread (TDLib replies on its
/// own thread, never the UI's).
fn wait<T>(
    request: impl std::future::Future<Output = Result<T, model::RequestError>>,
) -> io::Result<T> {
    futures::executor::block_on(request).map_err(|error| io::Error::other(error.to_string()))
}

fn td_offset(offset: u64) -> io::Result<i64> {
    i64::try_from(offset).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset"))
}

impl TdFile {
    fn local(&mut self) -> io::Result<&mut std::fs::File> {
        if self.local.is_none() {
            let partial = wait(self.session.partial_file(self.file))?;
            let path = partial
                .path
                .ok_or_else(|| io::Error::other("the download wrote nothing yet"))?;
            self.local = Some(std::fs::File::open(path)?);
        }
        Ok(self.local.as_mut().expect("opened just above"))
    }
}

impl FileSource for TdFile {
    fn size(&self) -> u64 {
        self.size
    }

    fn ready(&mut self, offset: u64) -> io::Result<u64> {
        let prefix = wait(
            self.session
                .downloaded_prefix(self.file, td_offset(offset)?),
        )?;
        Ok(u64::try_from(prefix).unwrap_or(0))
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let local = self.local()?;
        local.seek(SeekFrom::Start(offset))?;
        local.read(buf)
    }

    fn request(&mut self, offset: u64, limit: u64) -> io::Result<()> {
        let mut window = self.window.lock().expect("stream window lock poisoned");
        let half = offset + limit / 2;
        let is_covered =
            window.is_some_and(|(start, length)| start <= offset && half <= start + length);
        // A covered range counts while TDLib still downloads it.
        if is_covered && wait(self.session.partial_file(self.file))?.is_loading {
            return Ok(());
        }
        let session = self.session.clone();
        wait(session.download_range(self.file, td_offset(offset)?, td_offset(limit)?))?;
        *window = Some((offset, limit));
        Ok(())
    }

    fn is_loading(&mut self) -> io::Result<bool> {
        let partial = wait(self.session.partial_file(self.file))?;
        Ok(partial.is_loading || partial.is_complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_and_video_share_ranges_only_within_the_same_client() {
        let file = FileId(1);
        let first = (model::ClientId(-1), file);
        let second = (model::ClientId(-2), file);
        let signals = [Signal::new(), Signal::new(), Signal::new()];
        let audio = register(first, signals[0].clone());
        let video = register(first, signals[1].clone());
        let other = register(second, signals[2].clone());
        assert!(Arc::ptr_eq(&audio, &video));
        assert!(!Arc::ptr_eq(&audio, &other));
        for signal in signals {
            signal.close();
        }
        wake(first.0, file);
        wake(second.0, file);
    }
}
