use remote_buddy_core::protocol::{AtvvControl, AtvvNotification, decode_control, encode_control};

#[test]
fn encodes_mic_close_with_session_id() {
    assert_eq!(
        encode_control(&AtvvControl::MicClose { session_id: 0x6C }),
        vec![0x0D, 0x6C]
    );
}

#[test]
fn encodes_get_capabilities() {
    assert_eq!(
        encode_control(&AtvvControl::GetCapabilities),
        vec![0x0A, 0x01, 0x00, 0x00, 0x03, 0x03]
    );
}

#[test]
fn decodes_capabilities() {
    let notification =
        decode_control(&[0x0B, 0x00, 0x01, 0x02, 0x00, 0xA0, 0x00]).expect("packet should decode");
    let Some(AtvvNotification::Capabilities(capabilities)) = notification else {
        panic!("expected capabilities");
    };
    assert_eq!(capabilities.version, 0x0100);
    assert_eq!(capabilities.codec, 0x02);
    assert_eq!(capabilities.frame_size, 160);
}

#[test]
fn decodes_audio_sync_as_big_endian_predictor() {
    let notification =
        decode_control(&[0x0A, 0, 0, 0, 0x12, 0x34, 0x21]).expect("packet should decode");
    assert_eq!(
        notification,
        Some(AtvvNotification::AudioSync {
            predictor: 0x1234,
            step_index: 0x21
        })
    );
}

#[test]
fn rejects_truncated_capabilities() {
    assert!(decode_control(&[0x0B, 0, 1]).is_err());
}
