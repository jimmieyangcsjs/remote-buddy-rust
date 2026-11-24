use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevel {
    pub rms: f32,
    pub peak: f32,
}

impl AudioLevel {
    pub fn from_pcm16(samples: &[i16]) -> Self {
        let mut accumulator = AudioLevelAccumulator::default();
        accumulator.push_pcm16(samples);
        accumulator.level()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioLevelAccumulator {
    sum_squares: f64,
    samples: usize,
    peak: f32,
}

impl AudioLevelAccumulator {
    pub fn push_pcm16(&mut self, samples: &[i16]) {
        for sample in samples {
            let normalized = f64::from(i32::from(*sample)) / 32768.0;
            self.sum_squares += normalized * normalized;
            self.peak = self.peak.max(normalized.abs() as f32);
        }
        self.samples = self.samples.saturating_add(samples.len());
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn level(&self) -> AudioLevel {
        if self.samples == 0 {
            return AudioLevel {
                rms: 0.0,
                peak: 0.0,
            };
        }
        AudioLevel {
            rms: ((self.sum_squares / self.samples as f64).sqrt() as f32).clamp(0.0, 1.0),
            peak: self.peak.clamp(0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.000_1,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn silence_has_zero_rms_and_peak() {
        let level = AudioLevel::from_pcm16(&[0, 0, 0, 0]);
        assert_eq!(
            level,
            AudioLevel {
                rms: 0.0,
                peak: 0.0
            }
        );
    }

    #[test]
    fn constant_half_scale_signal_reports_half_scale() {
        let level = AudioLevel::from_pcm16(&[16_384, -16_384, 16_384, -16_384]);
        assert_close(level.rms, 0.5);
        assert_close(level.peak, 0.5);
    }

    #[test]
    fn minimum_i16_sample_clamps_to_full_scale() {
        let level = AudioLevel::from_pcm16(&[i16::MIN, i16::MAX]);
        assert_close(level.peak, 1.0);
        assert_close(level.rms, 1.0);
    }

    #[test]
    fn empty_input_is_silent() {
        assert_eq!(
            AudioLevel::from_pcm16(&[]),
            AudioLevel {
                rms: 0.0,
                peak: 0.0
            }
        );
    }

    #[test]
    fn accumulator_matches_level_over_the_concatenated_chunks() {
        let first = [16_384, -16_384, 0, 0];
        let second = [8_192, 8_192, -8_192, -8_192];
        let combined = [first.as_slice(), second.as_slice()].concat();

        let mut accumulator = AudioLevelAccumulator::default();
        accumulator.push_pcm16(&first);
        accumulator.push_pcm16(&second);

        let accumulated = accumulator.level();
        let expected = AudioLevel::from_pcm16(&combined);
        assert_close(accumulated.rms, expected.rms);
        assert_close(accumulated.peak, expected.peak);
    }

    #[test]
    fn reset_discards_the_previous_window() {
        let mut accumulator = AudioLevelAccumulator::default();
        accumulator.push_pcm16(&[16_384, -16_384]);
        accumulator.reset();

        assert_eq!(
            accumulator.level(),
            AudioLevel {
                rms: 0.0,
                peak: 0.0
            }
        );
    }
}
