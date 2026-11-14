use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_core::identity::IdentityKeyPair;
use ticonnect_core::wire::{Frame, FrameKind, HandshakeFrame, MAX_PAIRING_FRAME, WireError};

#[test]
fn frames_round_trip_and_enforce_kind_limits() {
    let frame = Frame::new(FrameKind::Pairing, b"pairing-frame").expect("frame");
    let decoded = Frame::decode(&frame.encode()).expect("decode");
    assert_eq!(decoded, frame);

    assert!(matches!(
        Frame::new(FrameKind::Pairing, vec![0_u8; MAX_PAIRING_FRAME + 1]),
        Err(WireError::FrameTooLarge)
    ));
}

#[test]
fn frame_decode_rejects_every_malformed_shape() {
    let valid = Frame::new(FrameKind::Handshake, Vec::new())
        .expect("frame")
        .encode();

    assert!(matches!(
        Frame::decode(&valid[..9]),
        Err(WireError::Truncated)
    ));

    let mut bad_magic = valid.clone();
    bad_magic[0] = b'X';
    assert!(matches!(
        Frame::decode(&bad_magic),
        Err(WireError::BadMagic)
    ));

    let mut bad_version = valid.clone();
    bad_version[4] = 2;
    assert!(matches!(
        Frame::decode(&bad_version),
        Err(WireError::UnsupportedVersion(2))
    ));

    let mut bad_kind = valid.clone();
    bad_kind[5] = 12;
    assert!(matches!(
        Frame::decode(&bad_kind),
        Err(WireError::UnsupportedKind(12))
    ));

    let mut trailing = valid;
    trailing.push(0);
    assert!(matches!(
        Frame::decode(&trailing),
        Err(WireError::TrailingBytes)
    ));
}

#[test]
fn handshake_frame_round_trips() {
    let identity = IdentityKeyPair::from_secret_bytes([51_u8; 32]);
    let handshake = HandshakeFrame::new(
        NodeId::generate(),
        Fingerprint::of(&identity.verifying_key()),
    );

    let decoded = HandshakeFrame::decode(&handshake.encode()).expect("decode");
    assert_eq!(decoded, handshake);

    let frame = Frame::new(FrameKind::Handshake, handshake.encode()).expect("frame");
    let decoded = Frame::decode(&frame.encode()).expect("decode frame");
    assert_eq!(
        HandshakeFrame::decode(decoded.payload()).expect("decode payload"),
        handshake
    );
}
