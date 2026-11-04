//! Plays a decoded voice message on the default output (`Media::Player`
//! for voices): pause, seek and speed while the callback reads the shared
//! position.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use cpal::traits::{DeviceTrait as _, StreamTrait as _};

use crate::decode::Decoded;
use crate::encode::RATE;

/// Positions count source samples in 32.32 fixed point.
const ONE: u64 = 1 << 32;

#[derive(Debug, thiserror::Error)]
pub enum PlayError {
    #[error("no audio output")]
    NoDevice,
    #[error("audio output")]
    Device(#[from] cpal::Error),
    #[error("the audio output takes {0} samples, which rgram does not write")]
    SampleFormat(cpal::SampleFormat),
}

struct Shared {
    samples: Arc<[i16]>,
    position: AtomicU64,
    /// Source samples per output frame, speed included.
    step: AtomicU64,
    is_paused: AtomicBool,
}

pub struct Player {
    shared: Arc<Shared>,
    /// Source samples per output frame at normal speed.
    base_step: f64,
    _stream: cpal::Stream,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Player")
            .field("position", &self.position())
            .finish_non_exhaustive()
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "steps are small and positive"
)]
fn fixed(value: f64) -> u64 {
    (value * ONE as f64) as u64
}

/// Writes `output` (interleaved, `channels` per frame) from `samples` at
/// `position`, moving it by `step` a frame; silence past the end.
fn fill(output: &mut [f32], channels: usize, samples: &[i16], position: &mut u64, step: u64) {
    for frame in output.chunks_exact_mut(channels.max(1)) {
        let index = usize::try_from(*position >> 32).unwrap_or(usize::MAX);
        #[expect(clippy::cast_precision_loss, reason = "a 32-bit fraction")]
        let fraction = (*position & (ONE - 1)) as f32 / ONE as f32;
        let at = |index: usize| samples.get(index).map_or(0.0, |&s| f32::from(s) / 32768.0);
        let value = at(index) * (1.0 - fraction) + at(index + 1) * fraction;
        frame.fill(value);
        if index < samples.len() {
            *position += step;
        }
    }
}

impl Player {
    /// Starts playing `decoded` from its start.
    // shortcut: speed resamples, so the pitch moves with it; tdesktop keeps the pitch (row 302).
    ///
    /// # Errors
    ///
    /// When there is no output device or it cannot be opened.
    pub fn start(decoded: &Decoded) -> Result<Self, PlayError> {
        let device =
            crate::devices::device(crate::devices::Direction::Output).ok_or(PlayError::NoDevice)?;
        let supported = device.default_output_config()?;
        let config = supported.config();
        let base_step = f64::from(RATE) / f64::from(config.sample_rate);
        let shared = Arc::new(Shared {
            samples: Arc::clone(&decoded.samples),
            position: AtomicU64::new(0),
            step: AtomicU64::new(fixed(base_step)),
            is_paused: AtomicBool::new(false),
        });
        let stream = build_stream(&device, &config, supported.sample_format(), &shared)?;
        stream.play()?;
        Ok(Self {
            shared,
            base_step,
            _stream: stream,
        })
    }

    fn total(&self) -> u64 {
        u64::try_from(self.shared.samples.len()).unwrap_or(u64::MAX)
    }

    pub fn duration(&self) -> Duration {
        Duration::from_micros(self.total() * 1_000_000 / u64::from(RATE))
    }

    pub fn position(&self) -> Duration {
        let samples = (self.shared.position.load(Ordering::Relaxed) >> 32).min(self.total());
        Duration::from_micros(samples * 1_000_000 / u64::from(RATE))
    }

    /// The share played, 0 to 1.
    pub fn progress(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            return 1.0;
        }
        let samples = (self.shared.position.load(Ordering::Relaxed) >> 32).min(total);
        #[expect(clippy::cast_precision_loss, reason = "a share")]
        let share = samples as f32 / total as f32;
        share
    }

    pub fn is_finished(&self) -> bool {
        self.shared.position.load(Ordering::Relaxed) >> 32 >= self.total()
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

    /// Jumps to `share` (0 to 1) of the message.
    pub fn seek(&self, share: f32) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "clamped to the sample count"
        )]
        let sample = (f64::from(share.clamp(0.0, 1.0)) * self.total() as f64) as u64;
        self.shared.position.store(sample << 32, Ordering::Relaxed);
    }

    /// Plays `speed` times as fast (tdesktop offers 0.5 to 2).
    pub fn set_speed(&self, speed: f32) {
        let step = fixed(self.base_step * f64::from(speed.clamp(0.25, 4.0)));
        self.shared.step.store(step, Ordering::Relaxed);
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, PlayError> {
    let channels = usize::from(config.channels.max(1));
    let shared = Arc::clone(shared);
    let render = move |data: &mut [f32]| {
        data.fill(0.0);
        if !shared.is_paused.load(Ordering::Relaxed) {
            let start = shared.position.load(Ordering::Relaxed);
            let step = shared.step.load(Ordering::Relaxed);
            let mut position = start;
            fill(data, channels, &shared.samples, &mut position, step);
            // A seek while filling wins over the advanced position.
            let _ = shared.position.compare_exchange(
                start,
                position,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
    };
    output_stream(device, config, format, render)
}

/// An output stream that `render` fills with interleaved f32 frames, in
/// whichever of f32 and i16 the device takes.
pub(crate) fn output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    mut render: impl FnMut(&mut [f32]) + Send + 'static,
) -> Result<cpal::Stream, PlayError> {
    let on_error = |error: cpal::Error| tracing::warn!(%error, "the audio output failed");
    let stream = match format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            *config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| render(data),
            on_error,
            None,
        )?,
        cpal::SampleFormat::I16 => {
            let mut scratch: Vec<f32> = Vec::new();
            device.build_output_stream(
                *config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    scratch.clear();
                    scratch.resize(data.len(), 0.0);
                    render(&mut scratch);
                    for (out, &value) in data.iter_mut().zip(&scratch) {
                        #[expect(clippy::cast_possible_truncation, reason = "clamped")]
                        let sample = (value.clamp(-1.0, 1.0) * 32767.0) as i16;
                        *out = sample;
                    }
                },
                on_error,
                None,
            )?
        }
        other => return Err(PlayError::SampleFormat(other)),
    };
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_copy_the_sample_to_every_channel() {
        let samples = [0_i16, 16_384, -16_384];
        let mut position = 0;
        let mut output = [1.0_f32; 6];
        fill(&mut output, 2, &samples, &mut position, ONE);
        assert_eq!(output, [0.0, 0.0, 0.5, 0.5, -0.5, -0.5]);
        assert_eq!(position >> 32, 3);
    }

    #[test]
    fn half_steps_interpolate_and_the_end_is_silent() {
        let samples = [0_i16, 16_384];
        let mut position = 0;
        let mut output = [1.0_f32; 5];
        fill(&mut output, 1, &samples, &mut position, ONE / 2);
        assert_eq!(output, [0.0, 0.25, 0.5, 0.25, 0.0]);
        // It stops moving once past the last sample.
        assert_eq!(position >> 32, 2);
    }
}
