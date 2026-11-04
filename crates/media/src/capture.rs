//! The microphone, read by cpal and encoded on a worker thread so the
//! audio callback only mixes to mono and hands the samples over.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait as _, StreamTrait as _};

use crate::encode::{EncodeError, RATE, VoiceWriter};
use crate::resample::Resampler;

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("no microphone")]
    NoDevice,
    #[error("microphone")]
    Device(#[from] cpal::Error),
    #[error("the microphone gives {0} samples, which rgram does not read")]
    SampleFormat(cpal::SampleFormat),
    #[error("encoding the voice message")]
    Encode(#[from] EncodeError),
    #[error("creating the voice file")]
    Io(#[from] std::io::Error),
    #[error("the voice encoder stopped unexpectedly")]
    Worker,
}

/// A finished voice message, ready for `inputMessageVoiceNote`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub path: PathBuf,
    pub duration: Duration,
    /// 5 bits per value.
    pub waveform: Vec<u8>,
}

/// How far the recording got, for the bar's timer and the button's pulse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub duration: Duration,
    /// The loudest sample since the last call, 0 to 1.
    pub level: f32,
}

#[derive(Debug, Default)]
struct Shared {
    samples: AtomicU64,
    level: AtomicU32,
    is_cancelled: AtomicBool,
}

type Outcome = Result<Option<Recorded>, RecordError>;

/// Where a recording's samples come from.
#[derive(Debug, Clone, Default)]
pub enum Source {
    #[default]
    Microphone,
    /// Mono samples at `rate`, all at once, as tests and snapshots record
    /// without a microphone.
    Samples { rate: u32, samples: Arc<[f32]> },
}

/// Records until stopped; dropping it throws the recording away.
pub struct Recorder {
    active: Option<Active>,
    shared: Arc<Shared>,
}

struct Active {
    /// `None` for preset samples.
    stream: Option<cpal::Stream>,
    samples: mpsc::Sender<Vec<f32>>,
    worker: JoinHandle<Outcome>,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Recorder")
            .field("shared", &self.shared)
            .finish_non_exhaustive()
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if self.active.is_some() {
            self.shared.is_cancelled.store(true, Ordering::Relaxed);
            // The worker removes the file once it ends.
            drop(self.halt());
        }
    }
}

/// A stopped recording whose last packets are still being written.
#[derive(Debug)]
pub struct Stopping {
    worker: JoinHandle<Outcome>,
}

impl Stopping {
    /// Blocks until the file is complete; `None` for a recording under a
    /// second, which leaves no file.
    ///
    /// # Errors
    ///
    /// When the file could not be written or the encoder failed.
    pub fn wait(self) -> Outcome {
        self.worker.join().map_err(|_| RecordError::Worker)?
    }
}

impl Recorder {
    /// Starts recording from `source` into `path` (an `.ogg` file).
    ///
    /// # Errors
    ///
    /// When there is no microphone, it cannot be opened, or the file cannot
    /// be created.
    pub fn start(source: &Source, path: PathBuf) -> Result<Self, RecordError> {
        let (samples, received) = mpsc::channel();
        let (stream, rate) = match source {
            Source::Microphone => {
                let device = crate::devices::device(crate::devices::Direction::Input)
                    .ok_or(RecordError::NoDevice)?;
                let supported = device.default_input_config()?;
                let config = supported.config();
                let stream =
                    build_stream(&device, &config, supported.sample_format(), samples.clone())?;
                (Some(stream), config.sample_rate)
            }
            Source::Samples {
                rate,
                samples: preset,
            } => {
                // The receiver holds them until the worker reads them.
                let _ = samples.send(preset.to_vec());
                (None, *rate)
            }
        };
        let writer = VoiceWriter::new(BufWriter::new(File::create(&path)?))?;
        let shared = Arc::new(Shared::default());
        let worker = {
            let shared = Arc::clone(&shared);
            let resampler = Resampler::new(rate, RATE);
            std::thread::Builder::new()
                .name("voice-encoder".to_owned())
                .spawn(move || encode(writer, resampler, &received, &shared, path))?
        };
        if let Some(stream) = &stream {
            stream.play()?;
        }
        Ok(Self {
            active: Some(Active {
                stream,
                samples,
                worker,
            }),
            shared,
        })
    }

    pub fn progress(&self) -> Progress {
        let samples = self.shared.samples.load(Ordering::Relaxed);
        let level = self.shared.level.swap(0, Ordering::Relaxed);
        #[expect(clippy::cast_precision_loss, reason = "a 16-bit level")]
        let level = level as f32 / f32::from(i16::MAX);
        Progress {
            duration: Duration::from_micros(samples * 1_000_000 / u64::from(RATE)),
            level,
        }
    }

    /// Stops the microphone; the file completes in the background.
    pub fn stop(mut self) -> Stopping {
        self.halt()
    }

    /// Stops and throws the recording away.
    pub fn cancel(self) {
        // Dropping cancels.
        drop(self);
    }

    fn halt(&mut self) -> Stopping {
        let Active {
            stream,
            samples,
            worker,
        } = self
            .active
            .take()
            .expect("a recorder halts once, from stop or drop");
        drop(stream);
        // The worker ends once every sender is gone.
        drop(samples);
        Stopping { worker }
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    samples: mpsc::Sender<Vec<f32>>,
) -> Result<cpal::Stream, RecordError> {
    let channels = usize::from(config.channels.max(1));
    let on_error = |error: cpal::Error| tracing::warn!(%error, "the microphone failed");
    let stream = match format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            *config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // A closed channel means the recording stopped.
                let _ = samples.send(mono(data, channels, |sample| sample));
            },
            on_error,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            *config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let _ = samples.send(mono(data, channels, |sample| f32::from(sample) / 32768.0));
            },
            on_error,
            None,
        )?,
        other => return Err(RecordError::SampleFormat(other)),
    };
    Ok(stream)
}

/// The channels' mean per frame.
fn mono<T: Copy>(data: &[T], channels: usize, to_f32: impl Fn(T) -> f32) -> Vec<f32> {
    #[expect(clippy::cast_precision_loss, reason = "a channel count")]
    let scale = 1.0 / channels as f32;
    data.chunks_exact(channels)
        .map(|frame| frame.iter().map(|&sample| to_f32(sample)).sum::<f32>() * scale)
        .collect()
}

fn encode(
    mut writer: VoiceWriter<BufWriter<File>>,
    mut resampler: Resampler,
    received: &mpsc::Receiver<Vec<f32>>,
    shared: &Shared,
    path: PathBuf,
) -> Outcome {
    let result = (|| {
        let mut resampled = Vec::new();
        while let Ok(chunk) = received.recv() {
            if shared.is_cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            resampled.clear();
            resampler.push(&chunk, &mut resampled);
            writer.push(&resampled)?;
            shared.samples.store(writer.received(), Ordering::Relaxed);
            shared
                .level
                .fetch_max(u32::from(writer.take_level()), Ordering::Relaxed);
        }
        if shared.is_cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let Some((file, encoded)) = writer.finish()? else {
            return Ok(None);
        };
        file.into_inner()
            .map_err(|error| error.into_error())?
            .sync_all()?;
        Ok(Some(Recorded {
            path: path.clone(),
            duration: Duration::from_micros(encoded.samples * 1_000_000 / u64::from(RATE)),
            waveform: encoded.waveform,
        }))
    })();
    if !matches!(result, Ok(Some(_))) {
        // Nothing to send: no file is left behind.
        let _ = std::fs::remove_file(&path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, seconds: u32) -> Arc<[f32]> {
        (0..rate * seconds)
            .map(|index| {
                #[expect(clippy::cast_precision_loss, reason = "a test tone")]
                let time = index as f32 / rate as f32;
                (time * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect()
    }

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rgram-media-{}-{name}.ogg", std::process::id()))
    }

    #[test]
    fn preset_samples_record_to_a_file() {
        let source = Source::Samples {
            rate: 44_100,
            samples: tone(44_100, 2),
        };
        let path = temp("recorded");
        let recorder = Recorder::start(&source, path.clone()).expect("the temp dir is writable");
        let recorded = recorder
            .stop()
            .wait()
            .expect("encoding works")
            .expect("two seconds are kept");
        assert_eq!(recorded.path, path);
        assert!(recorded.duration.abs_diff(Duration::from_secs(2)) < Duration::from_millis(5));
        let bytes = std::fs::read(&path).expect("the file was written");
        assert_eq!(&bytes[..4], b"OggS");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_cancelled_recording_leaves_no_file() {
        let source = Source::Samples {
            rate: 48_000,
            samples: tone(48_000, 2),
        };
        let path = temp("cancelled");
        let recorder = Recorder::start(&source, path.clone()).expect("the temp dir is writable");
        let stopping = {
            // Dropping cancels; keep the worker to wait for it here.
            let mut recorder = recorder;
            recorder.shared.is_cancelled.store(true, Ordering::Relaxed);
            recorder.halt()
        };
        assert_eq!(stopping.wait().expect("cancelling works"), None);
        assert!(!path.exists());
    }

    #[test]
    fn stereo_mixes_to_the_mean() {
        assert_eq!(
            mono(&[1.0_f32, 0.0, 0.5, 0.5], 2, |sample| sample),
            [0.5, 0.5]
        );
        assert_eq!(
            mono(&[16384_i16], 1, |sample| f32::from(sample) / 32768.0),
            [0.5]
        );
    }
}
