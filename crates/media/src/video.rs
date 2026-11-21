//! Video decoding for the in-app player (`Media::Streaming`): FFmpeg on a
//! thread of its own, the platform's hardware decoder when it has one, and
//! frames scaled down to the size they show at. Two frames at most wait
//! ahead of the one on screen, so a video holds a few frames of memory.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::thread;
use std::time::Duration;

use ffmpeg::codec::threading;
use ffmpeg::format::{Pixel, context::Input};
use ffmpeg::software::scaling::{self, Flags};
use ffmpeg::{Packet, codec, decoder, frame, media};
use ffmpeg_next as ffmpeg;

use crate::hw;
use crate::input::Origin;
use crate::stream::{Signal, Stream};

/// Frames decoded ahead of the one on screen (tdesktop keeps a few too).
const QUEUE: usize = 2;
/// Maximum coded image area, including cinema 8K.
const MAX_FRAME_PIXELS: u32 = 8192 * 4320;

#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("cannot open the video")]
    Open(#[source] ffmpeg::Error),
    #[error("the file has no video stream")]
    NoVideo,
    #[error("cannot decode the video")]
    Decode(#[source] ffmpeg::Error),
    #[error("cannot start the decoder thread")]
    Thread(#[source] std::io::Error),
}

/// Where a decoder is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decoding {
    Hardware,
    Software,
    /// libvpx, for VP9 with an alpha channel: FFmpeg's own VP9 decoder
    /// drops the alpha that WebM stickers carry (`FindDecoder`).
    Alpha,
}

/// libvpx's VP9 decoder in FFmpeg, the one that reads WebM's alpha.
const ALPHA_DECODER: &str = "libvpx-vp9";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoInfo {
    /// Zero when the container does not say.
    pub duration: Duration,
    pub width: u32,
    pub height: u32,
}

/// A picture in BGRA, rows packed, at most the size asked for.
#[derive(Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub position: Duration,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    generation: u32,
}

impl std::fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoFrame")
            .field("position", &self.position)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Output {
    /// First, once the file opened.
    Info(VideoInfo),
    Frame(VideoFrame),
    /// The last frame went out; a seek starts again.
    End,
    Failed(VideoError),
}

/// What the thread sends, tagged with the seek it follows.
#[derive(Debug)]
struct Sent {
    generation: u32,
    output: Output,
}

#[derive(Debug, Clone, Copy)]
enum Command {
    Seek { to: Duration, generation: u32 },
    Resize { width: u32, height: u32 },
}

/// A video decoding on its thread; dropping it stops the thread, and
/// wakes it if it waits for a streamed file's bytes.
#[derive(Debug)]
pub struct VideoDecoder {
    commands: Sender<Command>,
    output: Receiver<Sent>,
    generation: u32,
    signal: Option<Signal>,
}

impl VideoDecoder {
    /// Starts decoding the complete file at `path` into frames that fit
    /// `(width, height)`.
    ///
    /// # Errors
    /// When the thread cannot start; the file's own errors arrive as
    /// [`Output::Failed`].
    pub fn open(path: &Path, size: (u32, u32)) -> Result<Self, VideoError> {
        Self::start(Origin::Path(path.to_owned()), size)
    }

    /// Starts decoding a file while it downloads; reads that reach missing
    /// bytes wait on the decoder's thread.
    ///
    /// # Errors
    /// As [`VideoDecoder::open`].
    pub fn open_stream(stream: Stream, size: (u32, u32)) -> Result<Self, VideoError> {
        Self::start(Origin::Stream(stream), size)
    }

    fn start(origin: Origin, size: (u32, u32)) -> Result<Self, VideoError> {
        let (commands, received) = mpsc::channel();
        let (sender, output) = mpsc::sync_channel(QUEUE);
        let signal = origin.signal();
        thread::Builder::new()
            .name("video decoder".to_owned())
            .spawn(move || run(origin, size, &received, &sender))
            .map_err(VideoError::Thread)?;
        Ok(Self {
            commands,
            output,
            generation: 0,
            signal,
        })
    }

    /// How far the file is on disk past what the decoder read, 0 to 1; all
    /// of a complete file.
    #[must_use]
    pub fn received(&self) -> f32 {
        self.signal.as_ref().map_or(1.0, Signal::received)
    }

    /// Whether the decoder waits for bytes still downloading.
    #[must_use]
    pub fn is_buffering(&self) -> bool {
        self.signal.as_ref().is_some_and(Signal::is_waiting)
    }

    /// The next output, if one is ready; what came before the last seek is
    /// dropped.
    #[must_use]
    pub fn poll(&self) -> Option<Output> {
        loop {
            let sent = self.output.try_recv().ok()?;
            if sent.generation == self.generation {
                return Some(sent.output);
            }
        }
    }

    /// Continues from the keyframe before `to`, skipping frames until it.
    pub fn seek(&mut self, to: Duration) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        // A send fails only once the thread ended, and then nothing plays.
        let _ = self.commands.send(Command::Seek { to, generation });
    }

    /// Frames from now on fit `(width, height)`.
    pub fn resize(&self, (width, height): (u32, u32)) {
        let _ = self.commands.send(Command::Resize { width, height });
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        if let Some(signal) = &self.signal {
            signal.close();
        }
    }
}

fn run(origin: Origin, size: (u32, u32), commands: &Receiver<Command>, output: &SyncSender<Sent>) {
    let mut playback = Playback {
        generation: 0,
        target: size,
        skip_until: None,
        scaler: None,
        output,
        is_logged: false,
    };
    if let Err(error) = playback.play(origin, commands) {
        tracing::warn!(%error, "video decoding failed");
        playback.send(Output::Failed(error));
    }
}

/// The open file and its video decoder.
struct Source {
    input: Input,
    index: usize,
    /// Seconds per timestamp tick.
    time_base: f64,
    decoder: decoder::Video,
    info: VideoInfo,
}

impl Source {
    fn open(origin: Origin) -> Result<Self, VideoError> {
        ffmpeg::init().map_err(VideoError::Open)?;
        ffmpeg::log::set_level(ffmpeg::log::Level::Quiet);
        let input = origin.open().map_err(VideoError::Open)?;
        let stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or(VideoError::NoVideo)?;
        let (index, time_base) = (stream.index(), f64::from(stream.time_base()));
        let first = if has_alpha(&stream) {
            Decoding::Alpha
        } else {
            Decoding::Hardware
        };
        let decoder = match open_decoder(&stream, first, MAX_FRAME_PIXELS) {
            Ok(decoder) => decoder,
            Err(error) => {
                if first == Decoding::Alpha {
                    tracing::warn!(%error, "no libvpx: the video loses its alpha");
                }
                open_decoder(&stream, Decoding::Software, MAX_FRAME_PIXELS)?
            }
        };
        let duration =
            u64::try_from(input.duration()).map_or(Duration::ZERO, Duration::from_micros);
        let info = VideoInfo {
            duration,
            width: decoder.width(),
            height: decoder.height(),
        };
        Ok(Self {
            input,
            index,
            time_base,
            decoder,
            info,
        })
    }
}

/// Opens the stream's decoder, on the platform's device for `Hardware`;
/// fails there when it has none.
fn open_decoder(
    stream: &ffmpeg::format::stream::Stream<'_>,
    decoding: Decoding,
    max_pixels: u32,
) -> Result<decoder::Video, VideoError> {
    let mut context =
        codec::Context::from_parameters(stream.parameters()).map_err(VideoError::Open)?;
    if decoding == Decoding::Hardware && !hw::attach(&mut context) {
        return Err(VideoError::Open(ffmpeg::Error::DecoderNotFound));
    }
    context.set_threading(threading::Config {
        kind: threading::Type::Frame,
        ..threading::Config::default()
    });
    let codec = if decoding == Decoding::Alpha {
        decoder::find_by_name(ALPHA_DECODER)
    } else {
        context.codec().or_else(|| decoder::find(context.id()))
    }
    .ok_or(VideoError::Open(ffmpeg::Error::DecoderNotFound))?;
    let mut options = ffmpeg::Dictionary::new();
    options.set("max_pixels", &max_pixels.to_string());
    context
        .decoder()
        .open_as_with(codec, options)
        .and_then(decoder::Opened::video)
        .map_err(VideoError::Open)
}

/// A VP9 stream that WebM marks as carrying alpha (`alpha_mode`).
fn has_alpha(stream: &ffmpeg::format::stream::Stream<'_>) -> bool {
    stream.parameters().id() == codec::Id::VP9 && stream.metadata().get("alpha_mode") == Some("1")
}

struct Playback<'a> {
    generation: u32,
    target: (u32, u32),
    skip_until: Option<Duration>,
    scaler: Option<Scaler>,
    output: &'a SyncSender<Sent>,
    /// Whether the first frame's decoder was logged.
    is_logged: bool,
}

/// What the loop does next.
enum Flow {
    Go,
    Stop,
}

impl Playback<'_> {
    /// False once nobody listens.
    fn send(&self, output: Output) -> bool {
        let generation = self.generation;
        self.output.send(Sent { generation, output }).is_ok()
    }

    fn play(&mut self, origin: Origin, commands: &Receiver<Command>) -> Result<(), VideoError> {
        let mut source = Source::open(origin)?;
        if !self.send(Output::Info(source.info)) {
            return Ok(());
        }
        loop {
            loop {
                match commands.try_recv() {
                    Ok(command) => self.apply(command, &mut source),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            }
            let mut packet = Packet::empty();
            match packet.read(&mut source.input) {
                Ok(()) if packet.stream() == source.index => {
                    source
                        .decoder
                        .send_packet(&packet)
                        .map_err(VideoError::Decode)?;
                }
                Ok(()) => continue,
                Err(ffmpeg::Error::Eof) => {
                    // Draining twice after the end is harmless.
                    match source.decoder.send_eof() {
                        Ok(()) | Err(ffmpeg::Error::Eof) => {}
                        Err(error) => return Err(VideoError::Decode(error)),
                    }
                    if let Flow::Stop = self.drain(&mut source)? {
                        return Ok(());
                    }
                    if !self.send(Output::End) {
                        return Ok(());
                    }
                    match commands.recv() {
                        Ok(command) => self.apply(command, &mut source),
                        Err(_) => return Ok(()),
                    }
                    continue;
                }
                Err(error) => return Err(VideoError::Decode(error)),
            }
            if let Flow::Stop = self.drain(&mut source)? {
                return Ok(());
            }
        }
    }

    fn apply(&mut self, command: Command, source: &mut Source) {
        match command {
            Command::Seek { to, generation } => {
                self.generation = generation;
                self.skip_until = Some(to);
                let micros = i64::try_from(to.as_micros()).unwrap_or(i64::MAX);
                if let Err(error) = source.input.seek(micros, ..micros) {
                    tracing::warn!(%error, "seeking the video failed");
                }
                source.decoder.flush();
            }
            Command::Resize { width, height } => self.target = (width, height),
        }
    }

    /// Sends every frame the decoder has ready.
    fn drain(&mut self, source: &mut Source) -> Result<Flow, VideoError> {
        loop {
            let mut decoded = frame::Video::empty();
            if !crate::input::frame_ready(source.decoder.receive_frame(&mut decoded))
                .map_err(VideoError::Decode)?
            {
                return Ok(Flow::Go);
            }
            let position = frame_position(decoded.timestamp(), source.time_base)?;
            if self
                .skip_until
                .is_some_and(|until| position.saturating_add(frame_slack()) < until)
            {
                continue;
            }
            self.skip_until = None;
            let system = hw::download(&decoded).map_err(VideoError::Decode)?;
            if !self.is_logged {
                self.is_logged = true;
                tracing::debug!(is_hardware = system.is_some(), "video decoding");
            }
            let picture = self.scale(system.as_ref().unwrap_or(&decoded))?;
            let sent = Output::Frame(VideoFrame {
                position,
                generation: self.generation,
                ..picture
            });
            if !self.send(sent) {
                return Ok(Flow::Stop);
            }
        }
    }

    fn scale(&mut self, source: &frame::Video) -> Result<VideoFrame, VideoError> {
        let (width, height) = fit((source.width(), source.height()), self.target);
        let key = (
            source.format(),
            source.width(),
            source.height(),
            width,
            height,
        );
        let scaler = match self.scaler.take() {
            Some(scaler) if scaler.key == key => scaler,
            _ => Scaler::new(key)?,
        };
        let scaler = self.scaler.insert(scaler);
        let mut scaled = frame::Video::empty();
        scaler
            .context
            .run(source, &mut scaled)
            .map_err(VideoError::Decode)?;
        Ok(VideoFrame {
            position: Duration::ZERO,
            width,
            height,
            pixels: packed(&scaled, width, height),
            generation: 0,
        })
    }
}

fn frame_position(timestamp: Option<i64>, time_base: f64) -> Result<Duration, VideoError> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "FFmpeg timestamps are converted to seconds"
    )]
    let seconds = timestamp.unwrap_or(0) as f64 * time_base;
    if !time_base.is_finite() || time_base <= 0.0 {
        return Err(VideoError::Decode(ffmpeg::Error::InvalidData));
    }
    Duration::try_from_secs_f64(seconds.max(0.0))
        .map_err(|_| VideoError::Decode(ffmpeg::Error::InvalidData))
}

/// Half a frame at 60 fps: a frame this close before the seek target is
/// the one to show.
fn frame_slack() -> Duration {
    Duration::from_secs_f64(0.5 / 60.0)
}

type ScalerKey = (Pixel, u32, u32, u32, u32);

struct Scaler {
    key: ScalerKey,
    context: scaling::Context,
}

impl Scaler {
    fn new(key: ScalerKey) -> Result<Self, VideoError> {
        let (format, width, height, to_width, to_height) = key;
        let context = scaling::Context::get(
            format,
            width,
            height,
            Pixel::BGRA,
            to_width,
            to_height,
            Flags::BILINEAR,
        )
        .map_err(VideoError::Decode)?;
        Ok(Self { key, context })
    }
}

/// The BGRA rows without FFmpeg's padding.
fn packed(frame: &frame::Video, width: u32, height: u32) -> Vec<u8> {
    let row = width as usize * 4;
    let stride = frame.stride(0);
    let data = frame.data(0);
    (0..height as usize)
        .flat_map(|y| &data[y * stride..y * stride + row])
        .copied()
        .collect()
}

/// `size` scaled down to fit `bounds`, keeping its shape; never scaled up.
fn fit((width, height): (u32, u32), (max_width, max_height): (u32, u32)) -> (u32, u32) {
    if width <= max_width && height <= max_height || width == 0 || height == 0 {
        return (width.max(1), height.max(1));
    }
    let scale =
        (f64::from(max_width) / f64::from(width)).min(f64::from(max_height) / f64::from(height));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive size below the original"
    )]
    let scaled = |side: u32| ((f64::from(side) * scale).round() as u32).max(1);
    (scaled(width), scaled(height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn frame_positions_handle_preroll_and_invalid_time_bases() {
        assert_eq!(frame_position(Some(-1), 1.0).unwrap(), Duration::ZERO);
        assert!(frame_position(Some(i64::MAX), 4.0).is_err());
        assert!(frame_position(None, f64::NAN).is_err());
    }

    const CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/clip.mp4");
    /// 64x64 VP9 with alpha: clear but for an opaque red square in the middle.
    const ALPHA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/alpha.webm");
    /// `clip.mp4` with its index (`moov`) after the frames.
    const MOOV_LAST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/clip-moov-last.mp4");
    const WAIT: Duration = Duration::from_secs(10);

    /// The next output, waiting for the thread.
    fn next(decoder: &VideoDecoder) -> Output {
        let start = Instant::now();
        loop {
            if let Some(output) = decoder.poll() {
                return output;
            }
            assert!(
                start.elapsed() < WAIT,
                "the decoder answered nothing in {WAIT:?}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn frame(output: Output) -> VideoFrame {
        match output {
            Output::Frame(frame) => frame,
            other => panic!("expected a frame, got {other:?}"),
        }
    }

    #[test]
    #[ignore = "offline RSS measurement requiring a generated local fixture"]
    fn offline_video_rss_workload() {
        let clip = std::env::var_os("OXIGRAM_OFFLINE_CLIP").expect("path to a local 1080p fixture");
        let decoder = VideoDecoder::open(Path::new(&clip), (960, 540)).unwrap();
        let Output::Info(info) = next(&decoder) else {
            panic!("the info comes first");
        };
        assert_eq!((info.width, info.height), (1920, 1080));
        let mut count = 0;
        let started = Instant::now();
        loop {
            match next(&decoder) {
                Output::Frame(frame) => {
                    assert_eq!((frame.width, frame.height), (960, 540));
                    assert_eq!(frame.pixels.len(), 960 * 540 * 4);
                    thread::sleep(frame.position.saturating_sub(started.elapsed()));
                    count += 1;
                }
                Output::End => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(count, 240);
    }

    #[test]
    fn a_decoder_rejects_frames_above_its_pixel_budget() {
        ffmpeg::init().unwrap();
        let mut input = Origin::Path(CLIP.into()).open().unwrap();
        let stream = input.streams().best(media::Type::Video).unwrap();
        let index = stream.index();
        let Ok(mut decoder) = open_decoder(&stream, Decoding::Software, 32 * 32) else {
            return;
        };
        for (stream, packet) in input.packets() {
            if stream.index() == index && decoder.send_packet(&packet).is_err() {
                return;
            }
            assert!(decoder.receive_frame(&mut frame::Video::empty()).is_err());
        }
        assert!(
            decoder.send_eof().is_err()
                || decoder.receive_frame(&mut frame::Video::empty()).is_err()
        );
    }

    #[test]
    fn a_webm_keeps_its_alpha() {
        let decoder = VideoDecoder::open(Path::new(ALPHA), (64, 64)).expect("the thread starts");
        assert!(matches!(next(&decoder), Output::Info(_)));
        let frame = frame(next(&decoder));
        let alpha = |x: u32, y: u32| {
            frame.pixels[usize::try_from((y * frame.width + x) * 4 + 3).expect("in the frame")]
        };
        assert_eq!(alpha(0, 0), 0, "the corner is clear");
        assert_eq!(alpha(32, 32), 255, "the square is opaque");
    }

    #[test]
    fn a_clip_decodes_scaled_down_in_order_to_its_end() {
        let decoder = VideoDecoder::open(Path::new(CLIP), (32, 32)).expect("the thread starts");
        let Output::Info(info) = next(&decoder) else {
            panic!("the info comes first");
        };
        assert_eq!((info.width, info.height), (64, 36));
        assert_eq!(info.duration, Duration::from_secs(1));
        let mut positions = Vec::new();
        loop {
            match next(&decoder) {
                Output::Frame(frame) => {
                    assert_eq!((frame.width, frame.height), (32, 18));
                    assert_eq!(frame.pixels.len(), 32 * 18 * 4);
                    positions.push(frame.position);
                }
                Output::End => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(positions.len(), 10);
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn saved_mp4_and_gif_animations_restart_after_decoder_eof() {
        let gif = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/animation.gif");
        for path in [CLIP, gif] {
            let mut decoder = VideoDecoder::open(Path::new(path), (320, 240)).unwrap();
            assert!(matches!(next(&decoder), Output::Info(_)), "{path}");
            let first = frame(next(&decoder));
            let mut changed = false;
            loop {
                match next(&decoder) {
                    Output::Frame(frame) => changed |= frame.pixels != first.pixels,
                    Output::End => break,
                    other => panic!("{path}: unexpected {other:?}"),
                }
            }
            assert!(changed, "{path} must animate");
            decoder.seek(Duration::ZERO);
            let restarted = frame(next(&decoder));
            assert_eq!(restarted.position, first.position);
            assert_eq!(restarted.pixels, first.pixels);
        }
    }

    #[test]
    fn a_seek_drops_what_came_before_and_starts_at_the_target() {
        let mut decoder = VideoDecoder::open(Path::new(CLIP), (64, 64)).expect("the thread starts");
        assert!(matches!(next(&decoder), Output::Info(_)));
        let _ = frame(next(&decoder));
        decoder.seek(Duration::from_millis(500));
        let after = frame(next(&decoder));
        assert_eq!(after.position, Duration::from_millis(500));
        assert_eq!((after.width, after.height), (64, 36));
    }

    /// A file whose bytes land in small chunks after each request, as a
    /// download does.
    struct Trickle {
        bytes: Arc<Vec<u8>>,
        present: Arc<std::sync::Mutex<Vec<bool>>>,
        signal: crate::stream::Signal,
    }

    impl crate::stream::FileSource for Trickle {
        fn size(&self) -> u64 {
            self.bytes.len() as u64
        }

        fn ready(&mut self, offset: u64) -> std::io::Result<u64> {
            let present = self.present.lock().expect("test lock");
            let start = usize::try_from(offset).expect("small").min(present.len());
            Ok(present[start..].iter().take_while(|byte| **byte).count() as u64)
        }

        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
            let start = usize::try_from(offset).expect("small");
            buf.copy_from_slice(&self.bytes[start..start + buf.len()]);
            Ok(buf.len())
        }

        fn request(&mut self, offset: u64, limit: u64) -> std::io::Result<()> {
            let (present, signal) = (Arc::clone(&self.present), self.signal.clone());
            let start = usize::try_from(offset).expect("small");
            let end = start + usize::try_from(limit).expect("small");
            thread::spawn(move || {
                for chunk in (start..end).step_by(512) {
                    thread::sleep(Duration::from_millis(1));
                    let mut present = present.lock().expect("test lock");
                    let till = (chunk + 512).min(end).min(present.len());
                    present[chunk..till].fill(true);
                    drop(present);
                    signal.wake();
                }
            });
            Ok(())
        }

        fn is_loading(&mut self) -> std::io::Result<bool> {
            Ok(true)
        }
    }

    #[test]
    fn a_streamed_file_with_its_index_last_plays_as_it_lands() {
        let bytes = Arc::new(std::fs::read(MOOV_LAST).expect("the fixture is there"));
        let signal = crate::stream::Signal::new();
        let source = Trickle {
            present: Arc::new(std::sync::Mutex::new(vec![false; bytes.len()])),
            bytes,
            signal: signal.clone(),
        };
        let stream = Stream::new(source, signal);
        let decoder = VideoDecoder::open_stream(stream, (32, 32)).expect("the thread starts");
        let Output::Info(info) = next(&decoder) else {
            panic!("the info comes first");
        };
        assert_eq!(info.duration, Duration::from_secs(1));
        let mut frames = 0;
        loop {
            match next(&decoder) {
                Output::Frame(_) => frames += 1,
                Output::End => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(frames, 10);
    }

    #[test]
    fn dropping_a_waiting_stream_ends_its_thread() {
        struct Never;
        impl crate::stream::FileSource for Never {
            fn size(&self) -> u64 {
                1 << 20
            }
            fn ready(&mut self, _: u64) -> std::io::Result<u64> {
                Ok(0)
            }
            fn read_at(&mut self, _: u64, _: &mut [u8]) -> std::io::Result<usize> {
                Ok(0)
            }
            fn request(&mut self, _: u64, _: u64) -> std::io::Result<()> {
                Ok(())
            }
            fn is_loading(&mut self) -> std::io::Result<bool> {
                Ok(true)
            }
        }
        let signal = crate::stream::Signal::new();
        let decoder =
            VideoDecoder::open_stream(Stream::new(Never, signal.clone()), (8, 8)).expect("starts");
        let start = Instant::now();
        while !decoder.is_buffering() {
            assert!(start.elapsed() < WAIT, "the read waits for bytes");
            thread::sleep(Duration::from_millis(5));
        }
        drop(decoder);
        assert!(signal.is_closed(), "the waiting read wakes and fails");
    }

    #[test]
    fn a_missing_file_fails_on_the_thread() {
        let decoder = VideoDecoder::open(Path::new("/nonexistent/clip.mp4"), (8, 8))
            .expect("the thread starts");
        assert!(matches!(
            next(&decoder),
            Output::Failed(VideoError::Open(_))
        ));
    }

    #[test]
    fn sizes_shrink_to_fit_and_never_grow() {
        assert_eq!(fit((1920, 1080), (960, 960)), (960, 540));
        assert_eq!(fit((1080, 1920), (500, 500)), (281, 500));
        assert_eq!(fit((64, 36), (500, 500)), (64, 36));
    }
}
