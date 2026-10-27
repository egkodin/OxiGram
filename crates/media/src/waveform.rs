//! The waveform under a voice message: one peak per 10 ms while recording,
//! reduced to 100 values of 0-31 (`CollectWaveform`) and packed 5 bits each
//! (`documentWaveformEncode5bit`).

/// Values in the waveform sent (`Player::kWaveformSamplesCount`).
const SAMPLES_COUNT: usize = 100;
/// The quietest recording still spans this peak (`CollectWaveform`).
const MIN_PEAK: u64 = 2500;
const MAX_VALUE: u64 = 31;

/// Up to 100 values of 0-31 from the 10 ms peaks (each a sample's
/// magnitude / 256); none from fewer than 100 peaks.
pub(crate) fn collect(peaks: &[u8]) -> Vec<u8> {
    let count = peaks.len();
    if count < SAMPLES_COUNT {
        return Vec::new();
    }
    let mut reduced: Vec<u16> = Vec::with_capacity(SAMPLES_COUNT);
    let (mut peak, mut sum) = (0_u16, 0_usize);
    for &value in peaks {
        peak = peak.max(u16::from(value) * 256);
        sum += SAMPLES_COUNT;
        if sum >= count {
            sum -= count;
            reduced.push(peak);
            peak = 0;
        }
    }
    let total: u64 = reduced.iter().map(|&value| u64::from(value)).sum();
    // tdesktop scales by 1.8 times the mean, kept above the floor.
    let top = (total * 18 / (10 * reduced.len() as u64)).max(MIN_PEAK);
    reduced
        .iter()
        .map(|&value| {
            let scaled = u64::from(value).min(top) * MAX_VALUE / top;
            u8::try_from(scaled.min(MAX_VALUE)).unwrap_or(0)
        })
        .collect()
}

/// Packs each value's low 5 bits, least significant first.
pub(crate) fn encode_5bit(values: &[u8]) -> Vec<u8> {
    let bits = values.len() * 5;
    // One spare byte so any value can be written as two bytes.
    let mut result = vec![0_u8; bits.div_ceil(8) + 1];
    for (index, &value) in values.iter().enumerate() {
        let (byte, shift) = (index * 5 / 8, index * 5 % 8);
        let shifted = (u16::from(value) & 0x1F) << shift;
        let [low, high] = shifted.to_le_bytes();
        result[byte] |= low;
        result[byte + 1] |= high;
    }
    result.truncate(bits.div_ceil(8));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_pack_five_bits_each() {
        assert_eq!(encode_5bit(&[31, 0, 1]), [0x1F, 0x04]);
        assert_eq!(encode_5bit(&[1, 1]), [0x21, 0x00]);
        assert_eq!(encode_5bit(&[]), Vec::<u8>::new());
    }

    #[test]
    fn peaks_reduce_to_a_hundred_values_scaled_to_the_mean() {
        assert!(collect(&[10; 99]).is_empty());
        let mut peaks = vec![20_u8; 200];
        peaks[0] = 120;
        let values = collect(&peaks);
        assert_eq!(values.len(), 100);
        // A peak above 1.8 times the mean saturates; the rest sit below.
        assert_eq!(values[0], 31);
        assert_eq!(values[1], 16);
    }
}
