//! Mono samples from the device's rate to 48 kHz, chunk by chunk.

// shortcut: linear interpolation without a low-pass filter; fine for voice from 44.1 or 48 kHz microphones, replace with a windowed-sinc resampler if low-rate devices alias.
#[derive(Debug)]
pub(crate) struct Resampler {
    /// Input samples per output sample.
    step: f64,
    /// The next output's place, where 0 is the previous chunk's last sample
    /// and 1 this chunk's first.
    position: f64,
    last: f32,
}

impl Resampler {
    pub(crate) fn new(from: u32, to: u32) -> Self {
        Self {
            step: f64::from(from) / f64::from(to),
            position: 1.0,
            last: 0.0,
        }
    }

    pub(crate) fn push(&mut self, input: &[f32], output: &mut Vec<f32>) {
        #[expect(clippy::float_cmp, reason = "equal rates give exactly 1")]
        if self.step == 1.0 {
            output.extend_from_slice(input);
            return;
        }
        let Some(&end) = input.last() else {
            return;
        };
        let at = |index: usize| {
            if index == 0 {
                self.last
            } else {
                input[index - 1]
            }
        };
        let length = input.len() as f64;
        while self.position < length {
            #[expect(clippy::cast_possible_truncation, reason = "below the chunk's length")]
            #[expect(clippy::cast_sign_loss, reason = "the position stays positive")]
            let index = self.position.floor() as usize;
            #[expect(clippy::cast_possible_truncation, reason = "a fraction")]
            let fraction = (self.position - self.position.floor()) as f32;
            output.push(at(index) * (1.0 - fraction) + at(index + 1) * fraction);
            self.position += self.step;
        }
        self.position -= length;
        self.last = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_rate_passes_samples_through_across_chunks() {
        let mut resampler = Resampler::new(48_000, 48_000);
        let mut output = Vec::new();
        resampler.push(&[1.0, 2.0], &mut output);
        resampler.push(&[3.0], &mut output);
        assert_eq!(output, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn halving_the_rate_interpolates_between_chunks() {
        let mut resampler = Resampler::new(24_000, 48_000);
        let mut output = Vec::new();
        resampler.push(&[0.0, 2.0], &mut output);
        resampler.push(&[4.0], &mut output);
        // The last sample waits for the next chunk to interpolate towards.
        assert_eq!(output, [0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn a_second_of_44_1_khz_gives_one_of_48() {
        let mut resampler = Resampler::new(44_100, 48_000);
        let mut output = Vec::new();
        for _ in 0..10 {
            resampler.push(&[0.5; 4410], &mut output);
        }
        assert!(output.len().abs_diff(48_000) <= 1, "{}", output.len());
    }
}
