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

/// Frames decoded ahead of the one on screen (tdesktop keeps a few too).
const QUEUE: usize = 2;

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
}

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

/// A video decoding on its thread; dropping it stops the thread.
#[derive(Debug)]
pub struct VideoDecoder {
    commands: Sender<Command>,
    output: Receiver<Sent>,
    generation: u32,
}

impl VideoDecoder {
    /// Starts decoding `path` into frames that fit `(width, height)`.
    ///
    /// # Errors
    /// When the thread cannot start; the file's own errors arrive as
    /// [`Output::Failed`].
    pub fn open(path: &Path, (width, height): (u32, u32)) -> Result<Self, VideoError> {
        let (commands, received) = mpsc::channel();
        let (sender, output) = mpsc::sync_channel(QUEUE);
        let path = path.to_owned();
        thread::Builder::new()
            .name("video decoder".to_owned())
            .spawn(move || run(&path, (width, height), &received, &sender))
            .map_err(VideoError::Thread)?;
        Ok(Self {
            commands,
            output,
            generation: 0,
        })
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

fn run(path: &Path, size: (u32, u32), commands: &Receiver<Command>, output: &SyncSender<Sent>) {
    let mut playback = Playback {
        generation: 0,
        target: size,
        skip_until: None,
        scaler: None,
        output,
        is_logged: false,
    };
    if let Err(error) = playback.play(path, commands) {
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
    fn open(path: &Path) -> Result<Self, VideoError> {
        ffmpeg::init().map_err(VideoError::Open)?;
        // FFmpeg's own log goes to stderr; rgram logs through `tracing`,
        // and a hardware decoder that declines a stream is not an error.
        ffmpeg::log::set_level(ffmpeg::log::Level::Fatal);
        let input = ffmpeg::format::input(path).map_err(VideoError::Open)?;
        let stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or(VideoError::NoVideo)?;
        let (index, time_base) = (stream.index(), f64::from(stream.time_base()));
        let decoder = match open_decoder(&stream, Decoding::Hardware) {
            Ok(decoder) => decoder,
            Err(_) => open_decoder(&stream, Decoding::Software)?,
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
    context.decoder().video().map_err(VideoError::Open)
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

    fn play(&mut self, path: &Path, commands: &Receiver<Command>) -> Result<(), VideoError> {
        let mut source = Source::open(path)?;
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
                    let _ = source.decoder.send_eof();
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
            if source.decoder.receive_frame(&mut decoded).is_err() {
                return Ok(Flow::Go);
            }
            #[expect(clippy::cast_precision_loss, reason = "timestamps fit an f64")]
            let seconds = decoded.timestamp().unwrap_or(0) as f64 * source.time_base;
            let position = Duration::from_secs_f64(seconds.max(0.0));
            if self
                .skip_until
                .is_some_and(|until| position + frame_slack() < until)
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
    use std::time::Instant;

    const CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/clip.mp4");
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
    fn a_seek_drops_what_came_before_and_starts_at_the_target() {
        let mut decoder = VideoDecoder::open(Path::new(CLIP), (64, 64)).expect("the thread starts");
        assert!(matches!(next(&decoder), Output::Info(_)));
        let _ = frame(next(&decoder));
        decoder.seek(Duration::from_millis(500));
        let after = frame(next(&decoder));
        assert_eq!(after.position, Duration::from_millis(500));
        assert_eq!((after.width, after.height), (64, 36));
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
