const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

const INDEX_TABLE: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

#[derive(Debug, Clone, Default)]
pub struct ImaAdpcmDecoder {
    predictor: i32,
    step_index: usize,
    low_nibble_first: bool,
}

impl ImaAdpcmDecoder {
    pub fn with_nibble_order(low_nibble_first: bool) -> Self {
        Self {
            low_nibble_first,
            ..Self::default()
        }
    }

    pub fn synchronize(&mut self, predictor: i16, step_index: u8) {
        self.predictor = i32::from(predictor);
        self.step_index = usize::from(step_index.min(88));
    }

    pub fn reset(&mut self) {
        self.predictor = 0;
        self.step_index = 0;
    }

    pub fn decode(&mut self, input: &[u8], output: &mut Vec<i16>) {
        output.clear();
        self.decode_append(input, output);
    }

    pub fn decode_append(&mut self, input: &[u8], output: &mut Vec<i16>) {
        output.reserve(input.len() * 2);
        for byte in input {
            let (first, second) = if self.low_nibble_first {
                (byte & 0x0F, (byte >> 4) & 0x0F)
            } else {
                ((byte >> 4) & 0x0F, byte & 0x0F)
            };
            output.push(self.decode_sample(first));
            output.push(self.decode_sample(second));
        }
    }

    fn decode_sample(&mut self, nibble: u8) -> i16 {
        let step = STEP_TABLE[self.step_index];
        let mut diff = step >> 3;
        if nibble & 4 != 0 {
            diff += step;
        }
        if nibble & 2 != 0 {
            diff += step >> 1;
        }
        if nibble & 1 != 0 {
            diff += step >> 2;
        }

        if nibble & 8 != 0 {
            self.predictor -= diff;
        } else {
            self.predictor += diff;
        }
        self.predictor = self
            .predictor
            .clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        self.step_index =
            (self.step_index as i32 + INDEX_TABLE[usize::from(nibble & 7)]).clamp(0, 88) as usize;
        self.predictor as i16
    }
}
