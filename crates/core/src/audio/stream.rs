use super::adpcm::ImaAdpcmDecoder;

#[derive(Debug)]
pub struct AtvvAudioDecoder {
    pending: Vec<u8>,
    frame_size: usize,
    sync: Option<(i16, u8)>,
    decoder: ImaAdpcmDecoder,
}

impl Default for AtvvAudioDecoder {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            frame_size: 120,
            sync: None,
            decoder: ImaAdpcmDecoder::default(),
        }
    }
}

impl AtvvAudioDecoder {
    pub fn new(frame_size: usize) -> Self {
        Self {
            frame_size: frame_size.clamp(16, 512),
            ..Self::default()
        }
    }

    pub fn set_frame_size(&mut self, frame_size: usize) {
        if (16..=512).contains(&frame_size) {
            self.frame_size = frame_size;
            self.pending.clear();
        }
    }

    pub fn synchronize(&mut self, predictor: i16, step_index: u8) {
        self.sync = Some((predictor, step_index));
    }

    pub fn reset(&mut self) {
        self.pending.clear();
        self.sync = None;
        self.decoder.reset();
    }

    pub fn decode(&mut self, chunk: &[u8]) -> Vec<i16> {
        self.pending.reserve(chunk.len());
        self.pending.extend_from_slice(chunk);
        let frame_count = self.pending.len() / self.frame_size;
        if frame_count == 0 {
            return Vec::new();
        }

        let complete = frame_count * self.frame_size;
        let mut pending = std::mem::take(&mut self.pending);
        let mut output = Vec::with_capacity(complete * 2);
        for frame in pending[..complete].chunks_exact(self.frame_size) {
            if let Some((predictor, step_index)) = self.sync.take() {
                self.decoder.synchronize(predictor, step_index);
            }
            self.decoder.decode_append(frame, &mut output);
        }
        pending.drain(..complete);
        self.pending = pending;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_complete_frames() {
        let mut stream = AtvvAudioDecoder::new(160);
        assert!(stream.decode(&[0; 100]).is_empty());
        assert_eq!(stream.decode(&[0; 60]).len(), 320);
    }

    #[test]
    fn joins_partial_chunks_across_calls() {
        let mut stream = AtvvAudioDecoder::new(120);
        assert!(stream.decode(&[0; 40]).is_empty());
        assert!(stream.decode(&[0; 40]).is_empty());
        assert_eq!(stream.decode(&[0; 40]).len(), 240);
    }
}
