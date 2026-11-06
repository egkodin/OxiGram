//! Reading a file while it downloads (`Media::Streaming::Reader`): bytes
//! that are on disk come at once; a read that reaches a missing range asks
//! for it and waits on the decoder's thread until it lands, the download
//! is gone, or the player closes. The download follows the reader: a few
//! seconds past it, and wherever a seek jumps.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// How long a waiting read sleeps before it looks again by itself, should
/// a wake-up be missed.
const POLL: Duration = Duration::from_millis(250);
/// Looks that find the download stopped short before a read fails.
const MAX_STALLS: u32 = 3;
/// What the reader keeps downloading past itself until the player tells it
/// the video's rate (`set_read_ahead`).
pub const DEFAULT_READ_AHEAD: u64 = 1 << 20;

/// A file that downloads in ranges.
pub trait FileSource: Send + 'static {
    /// The whole file's size in bytes.
    fn size(&self) -> u64;
    /// The bytes on disk from `offset` on, without waiting.
    ///
    /// # Errors
    /// When the file cannot be looked at.
    fn ready(&mut self, offset: u64) -> io::Result<u64>;
    /// Reads bytes at `offset` that `ready` reported.
    ///
    /// # Errors
    /// When the local file cannot be read.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize>;
    /// Asks for `limit` bytes from `offset` to download.
    ///
    /// # Errors
    /// When the download cannot start.
    fn request(&mut self, offset: u64, limit: u64) -> io::Result<()>;
    /// Whether a download is running.
    ///
    /// # Errors
    /// When the file cannot be looked at.
    fn is_loading(&mut self) -> io::Result<bool>;
}

/// The link between a reader and the rest of the app: file updates wake
/// it, the player closes it, and the UI sees it waiting for data.
#[derive(Debug, Clone, Default)]
pub struct Signal(Arc<Shared>);

#[derive(Debug, Default)]
struct Shared {
    state: Mutex<Wakes>,
    changed: Condvar,
    is_waiting: AtomicBool,
    read_ahead: AtomicU64,
    /// Where the bytes on disk past the reader end, as a share of the file
    /// (`f32` bits).
    received: AtomicU32,
}

#[derive(Debug, Default)]
struct Wakes {
    count: u64,
    is_closed: bool,
}

impl Signal {
    #[must_use]
    pub fn new() -> Self {
        let signal = Self::default();
        signal.set_read_ahead(DEFAULT_READ_AHEAD);
        signal
    }

    /// The file changed: a waiting read looks again.
    pub fn wake(&self) {
        let mut state = self.0.state.lock().expect("stream signal lock poisoned");
        state.count = state.count.wrapping_add(1);
        self.0.changed.notify_all();
    }

    /// Stops the reader: a waiting read and every later one fail.
    pub fn close(&self) {
        self.0
            .state
            .lock()
            .expect("stream signal lock poisoned")
            .is_closed = true;
        self.0.changed.notify_all();
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0
            .state
            .lock()
            .expect("stream signal lock poisoned")
            .is_closed
    }

    /// Whether a read waits for bytes that are not there yet (the player's
    /// buffering spinner).
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.0.is_waiting.load(Ordering::Relaxed)
    }

    /// How far the file is on disk past the reader, 0 to 1: the seek
    /// bar's received part.
    #[must_use]
    pub fn received(&self) -> f32 {
        f32::from_bits(self.0.received.load(Ordering::Relaxed))
    }

    fn set_received(&self, share: f32) {
        self.0.received.store(share.to_bits(), Ordering::Relaxed);
    }

    /// What to keep downloading past the reader, in bytes.
    pub fn set_read_ahead(&self, bytes: u64) {
        self.0.read_ahead.store(bytes.max(1), Ordering::Relaxed);
    }

    fn read_ahead(&self) -> u64 {
        self.0.read_ahead.load(Ordering::Relaxed)
    }

    fn set_waiting(&self, is_waiting: bool) {
        self.0.is_waiting.store(is_waiting, Ordering::Relaxed);
    }

    fn count(&self) -> u64 {
        self.0
            .state
            .lock()
            .expect("stream signal lock poisoned")
            .count
    }

    /// Sleeps until a wake after `seen`, a close, or `POLL`.
    fn wait(&self, seen: u64) {
        let state = self.0.state.lock().expect("stream signal lock poisoned");
        let _ = self
            .0
            .changed
            .wait_timeout_while(state, POLL, |state| state.count == seen && !state.is_closed)
            .expect("stream signal lock poisoned");
    }
}

/// Reads a [`FileSource`] as FFmpeg's custom I/O does: `Read` blocks until
/// the bytes are there, `Seek` only moves.
pub struct Stream {
    source: Box<dyn FileSource>,
    signal: Signal,
    position: u64,
    /// The range asked for last.
    window: Option<(u64, u64)>,
    stalls: u32,
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("position", &self.position)
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

impl Stream {
    pub fn new(source: impl FileSource, signal: Signal) -> Self {
        Self {
            source: Box::new(source),
            signal,
            position: 0,
            window: None,
            stalls: 0,
        }
    }

    #[must_use]
    pub fn signal(&self) -> &Signal {
        &self.signal
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.source.size()
    }

    fn report_received(&self, ready: u64) {
        let size = self.source.size().max(1);
        #[expect(clippy::cast_precision_loss, reason = "a share of a file size")]
        let share = (self.position + ready).min(size) as f32 / size as f32;
        self.signal.set_received(share);
    }

    /// Asks for the bytes past the reader unless the last request covers
    /// half the read-ahead.
    fn keep_ahead(&mut self, ready: u64) -> io::Result<()> {
        let size = self.source.size();
        let ahead = self.signal.read_ahead();
        let missing = self.position + ready;
        if missing >= size.min(self.position + ahead) {
            return Ok(());
        }
        let half = size.min(self.position + ahead / 2);
        let is_covered = self
            .window
            .is_some_and(|(offset, limit)| offset <= missing && half <= offset + limit);
        if is_covered {
            return Ok(());
        }
        let limit = ahead.min(size - missing);
        self.source.request(missing, limit)?;
        self.window = Some((missing, limit));
        Ok(())
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.position >= self.source.size() {
            return Ok(0);
        }
        let result = self.read_waiting(buf);
        self.signal.set_waiting(false);
        result
    }
}

impl Stream {
    fn read_waiting(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let seen = self.signal.count();
            if self.signal.is_closed() {
                return Err(io::Error::other("the player closed the stream"));
            }
            let ready = self.source.ready(self.position)?;
            self.report_received(ready);
            self.keep_ahead(ready)?;
            if ready > 0 {
                let len = buf.len().min(usize::try_from(ready).unwrap_or(usize::MAX));
                let read = self.source.read_at(self.position, &mut buf[..len])?;
                if read == 0 {
                    return Err(io::Error::other("the downloaded part cannot be read"));
                }
                self.position += read as u64;
                self.stalls = 0;
                return Ok(read);
            }
            self.signal.set_waiting(true);
            self.signal.wait(seen);
            if self.signal.is_closed() || self.source.ready(self.position)? > 0 {
                continue;
            }
            if !self.source.is_loading()? {
                self.stalls += 1;
                if self.stalls >= MAX_STALLS {
                    return Err(io::Error::other("the download stopped short"));
                }
                // Asks again: TDLib may have dropped the request.
                self.window = None;
            }
        }
    }
}

impl Seek for Stream {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let size = self.source.size();
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(delta) => size.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
        };
        let target =
            target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before 0"))?;
        self.position = target;
        Ok(target)
    }
}

#[cfg(test)]
mod tests;
