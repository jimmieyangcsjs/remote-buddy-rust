use remote_buddy_core::audio::ImaAdpcmDecoder;

#[test]
fn decodes_zero_nibbles_to_zero() {
    let mut decoder = ImaAdpcmDecoder::default();
    let mut output = Vec::new();
    decoder.decode(&[0x00, 0x00], &mut output);
    assert_eq!(output, vec![0, 0, 0, 0]);
}

#[test]
fn synchronizes_predictor_and_step_index() {
    let mut decoder = ImaAdpcmDecoder::default();
    decoder.synchronize(1234, 200);
    let mut output = Vec::new();
    decoder.decode(&[0x00], &mut output);
    assert_eq!(output.len(), 2);
    assert!(output[0] >= 1234);
}

#[test]
fn accepts_low_nibble_first_order() {
    let mut decoder = ImaAdpcmDecoder::with_nibble_order(true);
    let mut output = Vec::new();
    decoder.decode(&[0x0F], &mut output);
    assert_eq!(output.len(), 2);
    assert_ne!(output[0], output[1]);
}
