//! 48 kHz mono samples to an OGG Opus stream (`Media::Capture::Inner`):
//! silent for the first 400 ms, faded in over 300 ms and out over the last
//! 300 ms, in 20 ms packets at 32 kbit/s, with a peak every 10 ms for the
//! waveform.

use std::io::Write;

use ogg::writing::{PacketWriteEndInfo, PacketWriter};

use crate::waveform;

pub(crate) const RATE: u32 = 48_000;
const RATE_SAMPLES: usize = RATE as usize;
const FRAME: usize = RATE_SAMPLES / 50;
const BITRATE: i32 = 32_000;
/// The first 400 ms are dropped to silence the click of the press
/// (`kCaptureSkipDuration`).
const SKIP: usize = RATE_SAMPLES * 400 / 1000;
const FADE: usize = RATE_SAMPLES * 300 / 1000;
const WAVEFORM_EACH: usize = RATE_SAMPLES / 100;
/// Recordings shorter than a second are dropped, as tdesktop's capture does.
const MIN_SAMPLES: u64 = RATE as u64;
/// RFC 6716's largest packet.
const MAX_PACKET: usize = 1275;
const SERIAL: u32 = 0x7267_7276;

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("opus encoder")]
    Opus(#[from] opus::Error),
    #[error("writing the voice file")]
    Io(#[from] std::io::Error),
}

/// What a finished recording holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Encoded {
    pub samples: u64,
    /// Packed 5 bits per value, as TDLib takes it.
    pub waveform: Vec<u8>,
}

pub(crate) struct VoiceWriter<W: Write> {
    encoder: opus::Encoder,
    ogg: PacketWriter<'static, W>,
    pre_skip: u64,
    /// Received and not yet encoded; the last 300 ms wait for the fade out.
    pending: Vec<f32>,
    received: u64,
    encoded: u64,
    peaks: Vec<u8>,
    peak: u16,
    peak_count: usize,
    /// The loudest sample since `take_level`, as a 16-bit magnitude.
    level: u16,
}

impl<W: Write> std::fmt::Debug for VoiceWriter<W> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VoiceWriter")
            .field("received", &self.received)
            .field("encoded", &self.encoded)
            .finish_non_exhaustive()
    }
}

fn magnitude(sample: f32) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=32767"
    )]
    let value = (sample.abs().min(1.0) * 32767.0) as u16;
    value
}

/// The gain of sample `index` from the start: none while skipped, rising
/// over the fade.
fn fade_in(index: u64) -> f32 {
    let skip = SKIP as u64;
    if index < skip {
        return 0.0;
    }
    let into = index - skip;
    if into >= FADE as u64 {
        1.0
    } else {
        #[expect(clippy::cast_precision_loss, reason = "below the fade's length")]
        let gain = into as f32 / FADE as f32;
        gain
    }
}

impl<W: Write> VoiceWriter<W> {
    pub(crate) fn new(writer: W) -> Result<Self, EncodeError> {
        let mut encoder = opus::Encoder::new(RATE, opus::Channels::Mono, opus::Application::Voip)?;
        encoder.set_bitrate(opus::Bitrate::Bits(BITRATE))?;
        let pre_skip = u16::try_from(encoder.get_lookahead()?).unwrap_or(0);
        let mut ogg = PacketWriter::new(writer);
        ogg.write_packet(head(pre_skip), SERIAL, PacketWriteEndInfo::EndPage, 0)?;
        ogg.write_packet(tags(), SERIAL, PacketWriteEndInfo::EndPage, 0)?;
        Ok(Self {
            encoder,
            ogg,
            pre_skip: u64::from(pre_skip),
            pending: Vec::new(),
            received: 0,
            encoded: 0,
            peaks: Vec::new(),
            peak: 0,
            peak_count: 0,
            level: 0,
        })
    }

    pub(crate) fn received(&self) -> u64 {
        self.received
    }

    pub(crate) fn take_level(&mut self) -> u16 {
        std::mem::take(&mut self.level)
    }

    pub(crate) fn push(&mut self, samples: &[f32]) -> Result<(), EncodeError> {
        for &sample in samples {
            let faded = sample * fade_in(self.received);
            self.level = self.level.max(magnitude(faded));
            self.pending.push(faded);
            self.received += 1;
        }
        let ready = self.pending.len().saturating_sub(FADE) / FRAME * FRAME;
        self.encode_frames(ready, PacketWriteEndInfo::NormalPacket)
    }

    /// Ends the stream; `None` when it is shorter than a second.
    pub(crate) fn finish(mut self) -> Result<Option<(W, Encoded)>, EncodeError> {
        if self.received < MIN_SAMPLES {
            return Ok(None);
        }
        let length = self.pending.len();
        let fade = FADE.min(length);
        for (from_end, sample) in self.pending[length - fade..].iter_mut().rev().enumerate() {
            #[expect(clippy::cast_precision_loss, reason = "below the fade's length")]
            let gain = from_end as f32 / fade as f32;
            *sample *= gain;
        }
        self.pending.resize(length.div_ceil(FRAME) * FRAME, 0.0);
        let all = self.pending.len();
        self.encode_frames(all, PacketWriteEndInfo::EndStream)?;
        let encoded = Encoded {
            samples: self.received,
            waveform: waveform::encode_5bit(&waveform::collect(&self.peaks)),
        };
        Ok(Some((self.ogg.into_inner(), encoded)))
    }

    /// Encodes the first `count` pending samples, a whole number of frames;
    /// `last` ends the final packet.
    fn encode_frames(&mut self, count: usize, last: PacketWriteEndInfo) -> Result<(), EncodeError> {
        let frames: Vec<f32> = self.pending.drain(..count).collect();
        let total = frames.len() / FRAME;
        let mut packet = [0_u8; MAX_PACKET];
        for (index, frame) in frames.as_chunks::<FRAME>().0.iter().enumerate() {
            self.note_peaks(frame);
            let size = self.encoder.encode_float(frame, &mut packet)?;
            self.encoded += FRAME as u64;
            let is_last = index + 1 == total && matches!(last, PacketWriteEndInfo::EndStream);
            let (end, granule) = if is_last {
                // The granule trims the padding off the last frame.
                (PacketWriteEndInfo::EndStream, self.pre_skip + self.received)
            } else {
                (
                    PacketWriteEndInfo::NormalPacket,
                    self.pre_skip + self.encoded,
                )
            };
            self.ogg
                .write_packet(packet[..size].to_vec(), SERIAL, end, granule)?;
        }
        Ok(())
    }

    fn note_peaks(&mut self, frame: &[f32]) {
        for &sample in frame {
            self.peak = self.peak.max(magnitude(sample));
            self.peak_count += 1;
            if self.peak_count == WAVEFORM_EACH {
                self.peak_count = 0;
                self.peaks
                    .push(u8::try_from(self.peak / 256).unwrap_or(u8::MAX));
                self.peak = 0;
            }
        }
    }
}

/// RFC 7845's identification header: one channel, no mapping.
fn head(pre_skip: u16) -> Vec<u8> {
    let mut head = b"OpusHead".to_vec();
    head.push(1);
    head.push(1);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&RATE.to_le_bytes());
    head.extend_from_slice(&0_i16.to_le_bytes());
    head.push(0);
    head
}

fn tags() -> Vec<u8> {
    const VENDOR: &[u8] = b"OxiGram";
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&u32::try_from(VENDOR.len()).unwrap_or(0).to_le_bytes());
    tags.extend_from_slice(VENDOR);
    tags.extend_from_slice(&0_u32.to_le_bytes());
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|index| {
                #[expect(clippy::cast_precision_loss, reason = "a test tone")]
                let time = index as f32 / RATE as f32;
                (time * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect()
    }

    /// Each OGG page's header type and granule position.
    fn pages(bytes: &[u8]) -> Vec<(u8, u64)> {
        let mut reader = ogg::PacketReader::new(std::io::Cursor::new(bytes));
        let mut found = Vec::new();
        while let Ok(Some(packet)) = reader.read_packet() {
            if packet.last_in_page() {
                let kind =
                    u8::from(packet.first_in_stream()) | (u8::from(packet.last_in_stream()) << 2);
                found.push((kind, packet.absgp_page()));
            }
        }
        found
    }

    #[test]
    fn two_seconds_make_an_opus_stream_with_its_waveform() {
        let mut writer = VoiceWriter::new(Vec::new()).expect("the encoder opens");
        for chunk in tone(2 * RATE_SAMPLES + 123).chunks(512) {
            writer.push(chunk).expect("writing to memory");
        }
        let (bytes, encoded) = writer
            .finish()
            .expect("writing to memory")
            .expect("long enough");
        assert_eq!(&bytes[28..36], b"OpusHead");
        assert_eq!(encoded.samples, 2 * RATE as u64 + 123);
        assert_eq!(encoded.waveform.len(), 63, "100 values of 5 bits");
        let pages = pages(&bytes);
        assert_eq!(pages[0], (1, 0), "the head opens the stream");
        let (end, granule) = pages[pages.len() - 1];
        assert_eq!(end, 4, "the last page ends the stream");
        let pre_skip = u64::from(u16::from_le_bytes([bytes[38], bytes[39]]));
        assert_eq!(granule, pre_skip + encoded.samples);
    }

    #[test]
    fn under_a_second_is_dropped() {
        let mut writer = VoiceWriter::new(Vec::new()).expect("the encoder opens");
        writer
            .push(&tone(RATE_SAMPLES - 1))
            .expect("writing to memory");
        assert_eq!(writer.finish().expect("writing to memory"), None);
    }

    #[test]
    fn the_start_is_silent_then_fades_in() {
        assert_eq!(fade_in(0), 0.0);
        assert_eq!(fade_in(SKIP as u64), 0.0);
        assert_eq!(fade_in((SKIP + FADE / 2) as u64), 0.5);
        assert_eq!(fade_in((SKIP + FADE) as u64), 1.0);
    }
}
