use ticonnect_core::canonical::{CanonicalError, Decoder, Encoder};

#[test]
fn canonical_bytes_are_stable_and_round_trip() {
    let mut encoder = Encoder::new("pairing").expect("encoder");
    encoder.u16(1).expect("version");
    encoder.u8(2).expect("mode");
    encoder.bytes(b"token").expect("token");
    encoder.string("node-a").expect("node");
    encoder.u64(1234).expect("expiry");
    let bytes = encoder.finish();

    assert_eq!(bytes, stable_bytes());

    let mut decoder = Decoder::new("pairing", &bytes).expect("decoder");
    assert_eq!(decoder.u16().expect("version"), 1);
    assert_eq!(decoder.u8().expect("mode"), 2);
    assert_eq!(decoder.bytes().expect("token"), b"token");
    assert_eq!(decoder.string().expect("node"), "node-a");
    assert_eq!(decoder.u64().expect("expiry"), 1234);
    decoder.finish().expect("finished");
}

#[test]
fn canonical_decoder_rejects_wrong_domain_and_malformed_input() {
    let bytes = stable_bytes();
    assert!(matches!(
        Decoder::new("wire", &bytes),
        Err(CanonicalError::DomainMismatch)
    ));

    let mut decoder = Decoder::new("pairing", &bytes[..bytes.len() - 1]).expect("decoder");
    decoder.u16().expect("version");
    decoder.u8().expect("mode");
    decoder.bytes().expect("token");
    decoder.string().expect("node");
    assert!(matches!(decoder.u64(), Err(CanonicalError::Truncated)));
}

#[test]
fn canonical_encoder_rejects_oversized_fields() {
    let mut encoder = Encoder::new("pairing").expect("encoder");
    let oversized = vec![0_u8; ticonnect_core::canonical::MAX_CANONICAL_BYTES];
    assert!(matches!(
        encoder.bytes(&oversized),
        Err(CanonicalError::TooLarge)
    ));
}

fn stable_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"TiConnect canonical v1");
    bytes.extend_from_slice(&7_u32.to_be_bytes());
    bytes.extend_from_slice(b"pairing");
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.push(2);
    bytes.extend_from_slice(&5_u32.to_be_bytes());
    bytes.extend_from_slice(b"token");
    bytes.extend_from_slice(&6_u32.to_be_bytes());
    bytes.extend_from_slice(b"node-a");
    bytes.extend_from_slice(&1234_u64.to_be_bytes());
    bytes
}
