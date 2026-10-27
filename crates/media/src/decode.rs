//! An OGG Opus voice file to 48 kHz mono samples, the way voice messages
//! are sent; tdesktop decodes them with FFmpeg (`Media::Audio`).

use std::io::{Read, Seek};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::encode::RATE;

/// Opus's largest frame: 120 ms at 48 kHz.
const MAX_FRAME: usize = 5760;

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("reading the voice file")]
    Io(#[from] std::io::Error),
    #[error("reading the OGG stream")]
    Ogg(#[from] ogg::OggReadError),
    #[error("opus decoder")]
    Opus(#[from] opus::Error),
    #[error("the file is not OGG Opus")]
    NotOpus,
}

/// A voice message's samples, ready to play.
#[derive(Debug, Clone)]
pub struct Decoded {
    pub(crate) samples: Arc<[i16]>,
}

impl Decoded {
    pub fn duration(&self) -> Duration {
        let samples = u64::try_from(self.samples.len()).unwrap_or(u64::MAX);
        Duration::from_micros(samples * 1_000_000 / u64::from(RATE))
    }
}

/// Decodes the whole file; voice messages are short.
// shortcut: whole-file decode in memory (about 5.8 MB a minute); stream it with the music player (row 261).
///
/// # Errors
///
/// When the file cannot be read or is not OGG Opus (some apps send MP3 or
/// M4A voices).
pub fn decode(path: &Path) -> Result<Decoded, DecodeError> {
    decode_from(std::io::BufReader::new(std::fs::File::open(path)?))
}

/// The channel count and pre-skip from the `OpusHead` packet.
fn head(packet: &[u8]) -> Result<(usize, usize), DecodeError> {
    if packet.len() < 19 || &packet[..8] != b"OpusHead" {
        return Err(DecodeError::NotOpus);
    }
    let channels = usize::from(packet[9]);
    let pre_skip = usize::from(u16::from_le_bytes([packet[10], packet[11]]));
    if !(1..=2).contains(&channels) {
        return Err(DecodeError::NotOpus);
    }
    Ok((channels, pre_skip))
}

fn decode_from<R: Read + Seek>(reader: R) -> Result<Decoded, DecodeError> {
    let mut reader = ogg::PacketReader::new(reader);
    let first = reader.read_packet()?.ok_or(DecodeError::NotOpus)?;
    let (channels, pre_skip) = head(&first.data)?;
    let layout = if channels == 2 {
        opus::Channels::Stereo
    } else {
        opus::Channels::Mono
    };
    let mut decoder = opus::Decoder::new(RATE, layout)?;
    let mut frame = vec![0_i16; MAX_FRAME * channels];
    let mut samples: Vec<i16> = Vec::new();
    let mut end = None;
    // The second packet holds the tags.
    let _ = reader.read_packet()?;
    while let Some(packet) = reader.read_packet()? {
        let count = decoder.decode(&packet.data, &mut frame, false)?;
        samples.extend(
            frame[..count * channels]
                .chunks_exact(channels)
                .map(|pair| {
                    let sum: i32 = pair.iter().map(|&sample| i32::from(sample)).sum();
                    let mean = sum / i32::try_from(channels).unwrap_or(1);
                    i16::try_from(mean).unwrap_or(0)
                }),
        );
        if packet.last_in_page() {
            end = usize::try_from(packet.absgp_page()).ok();
        }
    }
    // The last granule says where the sound ends, past the encoder's padding.
    if let Some(end) = end.filter(|&end| end < samples.len()) {
        samples.truncate(end);
    }
    let samples = samples.split_off(pre_skip.min(samples.len()));
    Ok(Decoded {
        samples: samples.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::VoiceWriter;

    #[test]
    fn a_recording_decodes_to_its_length() {
        let tone: Vec<f32> = (0..96_000_u32)
            .map(|index| {
                #[expect(clippy::cast_precision_loss, reason = "a test tone")]
                let time = index as f32 / 48_000.0;
                (time * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect();
        let mut writer = VoiceWriter::new(Vec::new()).expect("the encoder opens");
        writer.push(&tone).expect("writing to memory");
        let (bytes, encoded) = writer
            .finish()
            .expect("writing to memory")
            .expect("long enough");
        let decoded = decode_from(std::io::Cursor::new(bytes)).expect("our own file decodes");
        // The encoder's 6.5 ms lookahead is not flushed at the end.
        let missing = encoded.samples - decoded.samples.len() as u64;
        assert!(missing <= 960, "{missing} samples short");
        assert!(decoded.duration() > Duration::from_millis(1980));
        // Past the silent start the tone is there.
        let loudest = decoded.samples[48_000..]
            .iter()
            .map(|s| s.unsigned_abs())
            .max();
        assert!(loudest > Some(8_000), "{loudest:?}");
    }

    #[test]
    fn other_files_are_not_opus() {
        let error = decode_from(std::io::Cursor::new(b"ID3 not an ogg".to_vec()));
        assert!(error.is_err());
        assert!(matches!(head(b"OpusHead"), Err(DecodeError::NotOpus)));
    }
}
