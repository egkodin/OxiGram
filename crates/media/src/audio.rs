//! Streams the sound of any file FFmpeg reads, music or a video's track,
//! to the default output (`Media::Player` over `Media::Streaming`): a
//! thread decodes and resamples to the output's format a little ahead of
//! what plays, and the output callback takes from that buffer.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait as _, StreamTrait as _};
use ffmpeg::format::sample::{Sample, Type};
use ffmpeg::software::resampling;
use ffmpeg::{ChannelLayout, Packet, codec, frame, media};
use ffmpeg_next as ffmpeg;

use crate::input::Origin;
use crate::player::output_stream;
use crate::stream::{Signal, Stream};

/// The speeds the players take (`kSpeedMin`, `kSpeedMax` are 0.5 and 2.5;
/// a little room on both sides).
pub const MIN_SPEED: f32 = 0.25;
pub const MAX_SPEED: f32 = 4.0;

/// Decoded sound kept ahead of the output.
const AHEAD: Duration = Duration::from_millis(500);
/// How often a waiting decoder looks for a seek or the end.
const POLL: Duration = Duration::from_millis(20);
/// The silent output's format, and its step.
const SILENT_RATE: u32 = 48_000;
const SILENT_CHANNELS: u16 = 2;
const SILENT_TICK: Duration = Duration::from_millis(10);

/// Set by automated runs: players keep time without opening the output.
pub(crate) static IS_SILENT: AtomicBool = AtomicBool::new(false);

/// Players started from now on play nothing aloud; their clocks still run.
pub fn silence_output() {
    IS_SILENT.store(true, Ordering::Relaxed);
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("cannot open the file")]
    Open(#[source] ffmpeg::Error),
    #[error("the file has no sound")]
    NoAudio,
    #[error("cannot start the decoder thread")]
    Thread(#[source] std::io::Error),
    #[error("the decoder thread stopped")]
    Stopped,
}

/// What the decoder and the output share.
struct Shared {
    /// Interleaved samples in the output's format.
    buffer: Mutex<VecDeque<f32>>,
    space: Condvar,
    channels: usize,
    rate: u32,
    /// Where the buffer's first sample is, in microseconds.
    base: AtomicU64,
    /// Frames played since `base`.
    played: AtomicU64,
    seek: Mutex<Option<Duration>>,
    is_paused: AtomicBool,
    /// How many source frames play per output frame, as `f32` bits.
    speed: AtomicU32,
    /// The part of a source frame the speed stepped past, as `f64` bits.
    fraction: AtomicU64,
    /// The decoder reached the end of the file.
    is_drained: AtomicBool,
    is_closed: AtomicBool,
    /// The file's length in microseconds, once it opened.
    duration: AtomicU64,
}

impl Shared {
    fn buffer(&self) -> MutexGuard<'_, VecDeque<f32>> {
        self.buffer.lock().expect("audio buffer lock poisoned")
    }

    /// Fills `output` from the buffer, never waiting on the decoder.
    fn pull(&self, output: &mut [f32]) {
        output.fill(0.0);
        if self.is_paused.load(Ordering::Relaxed) {
            return;
        }
        let mut buffer = match self.buffer.try_lock() {
            Ok(buffer) => buffer,
            // The decoder holds it for a moment: a few samples of silence.
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(_)) => panic!("audio buffer lock poisoned"),
        };
        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let fraction = f64::from_bits(self.fraction.load(Ordering::Relaxed));
        let (frames, fraction) = step(&buffer, output, self.channels, speed, fraction);
        buffer.drain(..frames * self.channels);
        self.fraction.store(fraction.to_bits(), Ordering::Relaxed);
        drop(buffer);
        let frames = u64::try_from(frames).unwrap_or(0);
        self.played.fetch_add(frames, Ordering::Relaxed);
        self.space.notify_one();
    }

    fn position(&self) -> Duration {
        let played = self.played.load(Ordering::Relaxed) * 1_000_000 / u64::from(self.rate);
        Duration::from_micros(self.base.load(Ordering::Relaxed) + played)
    }

    fn ahead(&self) -> usize {
        let frames = AHEAD.as_secs_f64() * f64::from(self.rate);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "half a second of frames"
        )]
        let frames = frames as usize;
        frames * self.channels
    }
}

/// Where the samples go.
enum Sink {
    Device {
        _stream: cpal::Stream,
    },
    /// No output, or a silent run: a thread takes samples in real time
    /// until the player closes.
    Clock,
}

pub struct AudioPlayer {
    shared: Arc<Shared>,
    /// Wakes a decoder that waits for a streamed file's bytes.
    signal: Option<Signal>,
    _sink: Sink,
}

impl std::fmt::Debug for AudioPlayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioPlayer")
            .field("position", &self.position())
            .finish_non_exhaustive()
    }
}

impl AudioPlayer {
    /// Starts playing `path`'s sound from `start`.
    // shortcut: speed steps through the samples, so the pitch moves with it; tdesktop keeps the pitch (row 302). No volume.
    ///
    /// # Errors
    ///
    /// When the file cannot be opened or has no sound.
    pub fn open(path: &Path, start: Duration) -> Result<Self, AudioError> {
        Self::start(Origin::Path(path.to_owned()), start, Wait::Opened)
    }

    /// Starts playing a file while it downloads, from `start`. It returns
    /// at once: the duration is zero until the file's header arrives, and a
    /// file without sound plays nothing.
    ///
    /// # Errors
    ///
    /// When the decoder thread or the output cannot start.
    pub fn open_stream(stream: Stream, start: Duration) -> Result<Self, AudioError> {
        Self::start(Origin::Stream(stream), start, Wait::No)
    }

    /// Whether the decoder waits for bytes still downloading.
    pub fn is_buffering(&self) -> bool {
        self.signal.as_ref().is_some_and(Signal::is_waiting)
    }

    fn start(origin: Origin, start: Duration, wait: Wait) -> Result<Self, AudioError> {
        let output = Output::open();
        let (rate, channels) = output.format();
        let shared = Arc::new(Shared {
            buffer: Mutex::new(VecDeque::new()),
            space: Condvar::new(),
            channels: usize::from(channels),
            rate,
            base: AtomicU64::new(0),
            played: AtomicU64::new(0),
            seek: Mutex::new((start > Duration::ZERO).then_some(start)),
            is_paused: AtomicBool::new(false),
            speed: AtomicU32::new(1.0_f32.to_bits()),
            fraction: AtomicU64::new(0),
            is_drained: AtomicBool::new(false),
            is_closed: AtomicBool::new(false),
            duration: AtomicU64::new(0),
        });
        let (opened, is_open) = mpsc::sync_channel(1);
        let decoding = Arc::clone(&shared);
        let signal = origin.signal();
        thread::Builder::new()
            .name("audio decoder".to_owned())
            .spawn(move || decode(origin, &decoding, &opened))
            .map_err(AudioError::Thread)?;
        // Dropped on an error below, it stops the decoder.
        let mut player = Self {
            shared: Arc::clone(&shared),
            signal,
            _sink: Sink::Clock,
        };
        if wait == Wait::Opened {
            is_open.recv().map_err(|_| AudioError::Stopped)??;
        }
        player._sink = output.start(&shared)?;
        Ok(player)
    }

    /// Zero until a streamed file's header arrives.
    pub fn duration(&self) -> Duration {
        Duration::from_micros(self.shared.duration.load(Ordering::Relaxed))
    }

    pub fn position(&self) -> Duration {
        let duration = self.duration();
        let position = self.shared.position();
        if duration.is_zero() {
            position
        } else {
            position.min(duration)
        }
    }

    /// Played to the end of the file.
    pub fn is_finished(&self) -> bool {
        self.shared.is_drained.load(Ordering::Relaxed) && self.shared.buffer().is_empty()
    }

    pub fn is_paused(&self) -> bool {
        self.shared.is_paused.load(Ordering::Relaxed)
    }

    pub fn pause(&self) {
        self.shared.is_paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.shared.is_paused.store(false, Ordering::Relaxed);
    }

    /// Plays `speed` times as fast, from the next output block.
    pub fn set_speed(&self, speed: f32) {
        let speed = speed.clamp(MIN_SPEED, MAX_SPEED);
        self.shared.speed.store(speed.to_bits(), Ordering::Relaxed);
    }

    /// Plays on from `to`; the decoder starts over there.
    pub fn seek(&self, to: Duration) {
        *self.shared.seek.lock().expect("audio seek lock poisoned") = Some(to);
        self.shared.space.notify_all();
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.shared.is_closed.store(true, Ordering::Relaxed);
        self.shared.space.notify_all();
        if let Some(signal) = &self.signal {
            signal.close();
        }
    }
}

/// Whether opening waits for the decoder to read the file's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wait {
    Opened,
    No,
}

/// The output, picked before the decoder starts so it resamples to it.
enum Output {
    Device {
        device: cpal::Device,
        config: cpal::SupportedStreamConfig,
    },
    Silent,
}

impl Output {
    fn open() -> Self {
        if IS_SILENT.load(Ordering::Relaxed) {
            return Self::Silent;
        }
        let device = crate::devices::device(crate::devices::Direction::Output);
        match device.map(|device| (device.default_output_config(), device)) {
            Some((Ok(config), device)) => Self::Device { device, config },
            Some((Err(error), _)) => {
                tracing::warn!(%error, "no audio output format: playing silently");
                Self::Silent
            }
            None => {
                tracing::warn!("no audio output: playing silently");
                Self::Silent
            }
        }
    }

    fn format(&self) -> (u32, u16) {
        match self {
            Self::Device { config, .. } => (config.sample_rate(), config.channels().max(1)),
            Self::Silent => (SILENT_RATE, SILENT_CHANNELS),
        }
    }

    fn start(self, shared: &Arc<Shared>) -> Result<Sink, AudioError> {
        if let Self::Device { device, config } = self {
            let pulling = Arc::clone(shared);
            let render = move |data: &mut [f32]| pulling.pull(data);
            match output_stream(&device, &config.config(), config.sample_format(), render)
                .and_then(|stream| stream.play().map(|()| stream).map_err(Into::into))
            {
                Ok(stream) => return Ok(Sink::Device { _stream: stream }),
                Err(error) => tracing::warn!(%error, "the audio output failed: playing silently"),
            }
        }
        let pulling = Arc::clone(shared);
        thread::Builder::new()
            .name("audio clock".to_owned())
            .spawn(move || keep_time(&pulling))
            .map_err(AudioError::Thread)?;
        Ok(Sink::Clock)
    }
}

/// The silent output: takes a tick's samples every tick until closed.
fn keep_time(shared: &Shared) {
    let frames =
        u64::from(shared.rate) * u64::try_from(SILENT_TICK.as_millis()).unwrap_or(10) / 1000;
    let mut scratch = vec![0.0; usize::try_from(frames).unwrap_or(0) * shared.channels];
    while !shared.is_closed.load(Ordering::Relaxed) {
        shared.pull(&mut scratch);
        thread::sleep(SILENT_TICK);
    }
}

/// The decoder thread: opens the file, says how long it is, then decodes
/// until the player closes.
fn decode(origin: Origin, shared: &Shared, opened: &SyncSender<Result<(), AudioError>>) {
    let mut source = match Source::open(origin, shared) {
        Ok(source) => source,
        Err(error) => {
            if !matches!(error, AudioError::NoAudio) {
                tracing::warn!(%error, "the sound cannot open");
            }
            shared.is_drained.store(true, Ordering::Relaxed);
            // Nobody waits for a stream; it ends silent.
            let _ = opened.send(Err(error));
            return;
        }
    };
    let micros = u64::try_from(source.duration.as_micros()).unwrap_or(u64::MAX);
    shared.duration.store(micros, Ordering::Relaxed);
    let _ = opened.send(Ok(()));
    if let Err(error) = source.play(shared) {
        tracing::warn!(%error, "the sound stopped");
    }
    shared.is_drained.store(true, Ordering::Relaxed);
}

struct Source {
    input: ffmpeg::format::context::Input,
    index: usize,
    decoder: ffmpeg::decoder::Audio,
    resampler: Option<(resampling::Context, (Sample, ChannelLayout, u32))>,
    target: ChannelLayout,
    duration: Duration,
}

impl Source {
    fn open(origin: Origin, shared: &Shared) -> Result<Self, AudioError> {
        ffmpeg::init().map_err(AudioError::Open)?;
        ffmpeg::log::set_level(ffmpeg::log::Level::Fatal);
        let input = origin.open().map_err(AudioError::Open)?;
        let stream = input
            .streams()
            .best(media::Type::Audio)
            .ok_or(AudioError::NoAudio)?;
        let index = stream.index();
        let decoder = codec::Context::from_parameters(stream.parameters())
            .and_then(|context| context.decoder().audio())
            .map_err(AudioError::Open)?;
        let duration =
            u64::try_from(input.duration()).map_or(Duration::ZERO, Duration::from_micros);
        let channels = i32::try_from(shared.channels).unwrap_or(2);
        Ok(Self {
            input,
            index,
            decoder,
            resampler: None,
            target: ChannelLayout::default(channels),
            duration,
        })
    }

    /// Decodes into the buffer until the end or the player closes.
    fn play(&mut self, shared: &Shared) -> Result<(), ffmpeg::Error> {
        loop {
            if shared.is_closed.load(Ordering::Relaxed) {
                return Ok(());
            }
            self.seek_if_asked(shared)?;
            let mut packet = Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() == self.index => {
                    self.decoder.send_packet(&packet)?;
                    self.drain(shared)?;
                }
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => {
                    self.decoder.send_eof()?;
                    self.drain(shared)?;
                    shared.is_drained.store(true, Ordering::Relaxed);
                    // A seek may still come back from the end.
                    if !self.wait_at_end(shared) {
                        return Ok(());
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Waits for a seek or the close at the end; true on a seek.
    fn wait_at_end(&self, shared: &Shared) -> bool {
        let mut buffer = shared.buffer();
        loop {
            if shared.is_closed.load(Ordering::Relaxed) {
                return false;
            }
            if shared
                .seek
                .lock()
                .expect("audio seek lock poisoned")
                .is_some()
            {
                shared.is_drained.store(false, Ordering::Relaxed);
                return true;
            }
            buffer = shared
                .space
                .wait_timeout(buffer, POLL)
                .expect("audio buffer lock poisoned")
                .0;
        }
    }

    fn seek_if_asked(&mut self, shared: &Shared) -> Result<(), ffmpeg::Error> {
        let Some(to) = shared.seek.lock().expect("audio seek lock poisoned").take() else {
            return Ok(());
        };
        let micros = i64::try_from(to.as_micros()).unwrap_or(i64::MAX);
        self.input.seek(micros, ..micros)?;
        self.decoder.flush();
        let mut buffer = shared.buffer();
        buffer.clear();
        shared.played.store(0, Ordering::Relaxed);
        shared.fraction.store(0, Ordering::Relaxed);
        shared
            .base
            .store(u64::try_from(micros).unwrap_or(0), Ordering::Relaxed);
        Ok(())
    }

    fn drain(&mut self, shared: &Shared) -> Result<(), ffmpeg::Error> {
        let mut decoded = frame::Audio::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            let samples = self.resample(&decoded, shared.rate)?;
            push(shared, &samples);
            if shared.is_closed.load(Ordering::Relaxed) {
                break;
            }
        }
        Ok(())
    }

    /// `decoded` as interleaved f32 in the output's layout and rate.
    fn resample(&mut self, decoded: &frame::Audio, rate: u32) -> Result<Vec<f32>, ffmpeg::Error> {
        let layout = if decoded.channel_layout().is_empty() {
            ChannelLayout::default(i32::from(decoded.channels()))
        } else {
            decoded.channel_layout()
        };
        let key = (decoded.format(), layout, decoded.rate());
        let resampler = match self.resampler.take() {
            Some((context, was)) if was == key => (context, was),
            _ => (
                resampling::Context::get(
                    key.0,
                    key.1,
                    key.2,
                    Sample::F32(Type::Packed),
                    self.target,
                    rate,
                )?,
                key,
            ),
        };
        let (context, _) = self.resampler.insert(resampler);
        let mut out = frame::Audio::empty();
        context.run(decoded, &mut out)?;
        let length = out.samples() * usize::from(out.channels()) * 4;
        Ok(out.data(0)[..length.min(out.data(0).len())]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_ne_bytes(*bytes))
            .collect())
    }
}

/// Fills `output` from `buffer`, `speed` source frames an output frame,
/// each output frame the source frame it lands in; silence once the buffer
/// runs out. Returns the whole source frames passed and the part of one
/// left over for the next call.
fn step(
    buffer: &VecDeque<f32>,
    output: &mut [f32],
    channels: usize,
    speed: f32,
    mut fraction: f64,
) -> (usize, f64) {
    let channels = channels.max(1);
    let available = buffer.len() / channels;
    let mut frame = 0;
    for out in output.chunks_exact_mut(channels) {
        if frame >= available {
            break;
        }
        for (channel, sample) in out.iter_mut().enumerate() {
            *sample = buffer[frame * channels + channel];
        }
        fraction += f64::from(speed);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a few frames, positive"
        )]
        let whole = fraction.floor() as usize;
        frame += whole;
        #[expect(clippy::cast_precision_loss, reason = "a few frames")]
        let passed = whole as f64;
        fraction -= passed;
    }
    (frame.min(available), fraction)
}

/// Adds `samples` to the buffer, waiting while it holds enough; a seek or
/// the close drops them.
fn push(shared: &Shared, samples: &[f32]) {
    let ahead = shared.ahead();
    let mut buffer = shared.buffer();
    while buffer.len() >= ahead {
        let is_moving = shared.is_closed.load(Ordering::Relaxed)
            || shared
                .seek
                .lock()
                .expect("audio seek lock poisoned")
                .is_some();
        if is_moving {
            return;
        }
        buffer = shared
            .space
            .wait_timeout(buffer, POLL)
            .expect("audio buffer lock poisoned")
            .0;
    }
    buffer.extend(samples);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const TONE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/tone.m4a");
    const CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/clip.mp4");

    fn silent(path: &str, start: Duration) -> AudioPlayer {
        silence_output();
        AudioPlayer::open(Path::new(path), start).expect("the tone opens")
    }

    fn wait_until(what: &str, done: impl Fn() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "{what} never came"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_tone_plays_to_its_end_on_the_silent_clock() {
        let player = silent(TONE, Duration::ZERO);
        assert!((player.duration().as_secs_f32() - 1.0).abs() < 0.05);
        wait_until("the end", || player.is_finished());
        assert!(player.position() >= Duration::from_millis(900));
    }

    #[test]
    fn a_paused_player_holds_its_place_and_a_seek_moves_it() {
        let player = silent(TONE, Duration::from_millis(500));
        player.pause();
        thread::sleep(Duration::from_millis(100));
        let held = player.position();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(player.position(), held);
        assert!(held >= Duration::from_millis(400), "{held:?}");
        player.seek(Duration::from_millis(100));
        player.resume();
        wait_until("the seek", || {
            player.position() < Duration::from_millis(400)
        });
    }

    #[test]
    fn speed_steps_through_the_frames() {
        let buffer: VecDeque<f32> = (0..8).map(|n| n as f32).collect();
        let mut output = [0.0; 4];
        assert_eq!(step(&buffer, &mut output, 2, 1.0, 0.0), (2, 0.0));
        assert_eq!(output, [0.0, 1.0, 2.0, 3.0]);
        let (frames, fraction) = step(&buffer, &mut output, 1, 1.5, 0.0);
        assert_eq!((frames, fraction), (6, 0.0));
        assert_eq!(output, [0.0, 1.0, 3.0, 4.0]);
        let (frames, fraction) = step(&buffer, &mut output, 1, 0.5, 0.0);
        assert_eq!((frames, fraction), (2, 0.0));
        assert_eq!(output, [0.0, 0.0, 1.0, 1.0]);
        // Past the buffer's end the rest stays silent.
        let mut long = [9.0; 6];
        assert_eq!(step(&buffer, &mut long, 2, 2.0, 0.0).0, 4);
        assert_eq!(long, [0.0, 1.0, 4.0, 5.0, 9.0, 9.0]);
    }

    #[test]
    fn a_fast_player_moves_faster() {
        let player = silent(TONE, Duration::ZERO);
        player.set_speed(2.0);
        let start = Instant::now();
        wait_until("the end", || player.is_finished());
        assert!(
            start.elapsed() < Duration::from_millis(800),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_file_without_sound_says_so() {
        silence_output();
        let error = AudioPlayer::open(Path::new(CLIP), Duration::ZERO).expect_err("no sound");
        assert!(matches!(error, AudioError::NoAudio), "{error:?}");
    }
}
