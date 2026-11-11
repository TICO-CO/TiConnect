use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_net::pairing::{
    PairingMachine, PairingMessage, PairingMode, PairingSecret, PairingState, PairingTranscript,
    PeerIdentity, PinPolicy, QrPayload, Role, Spake2Session, TOKEN_BYTES, decode_message,
    encode_frame,
};

#[test]
fn pin_policy_generates_and_validates_six_digit_pins() {
    let pin = PinPolicy::generate();
    assert_eq!(pin.value().len(), 6);
    assert!(pin.value().bytes().all(|byte| byte.is_ascii_digit()));
    assert!(PinPolicy::from_value("000123").is_ok());
    assert!(PinPolicy::from_value("12x456").is_err());
}

#[test]
fn qr_payload_round_trips_and_expires() {
    let payload = QrPayload::generate_with_ttl(
        NodeId::generate(),
        Fingerprint::from_bytes([5_u8; 32]),
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
        45871,
        Duration::from_secs(30),
    )
    .expect("qr payload");

    let decoded = QrPayload::decode(&payload.encode()).expect("decode");
    assert_eq!(decoded, payload);
    assert_eq!(decoded.token().len(), TOKEN_BYTES);
    assert!(decoded.is_expired(decoded.expires_at_unix()));
}

#[test]
fn spake2_accepts_matching_secrets_and_rejects_mismatches() {
    let transcript = transcript(PairingMode::Pin, None);
    let hash = transcript.hash().expect("transcript hash");
    let pin = PairingSecret::Pin("123456".to_owned());

    let (initiator, initiator_message) =
        Spake2Session::start(Role::Initiator, &pin, &transcript).expect("initiator");
    let (responder, responder_message) =
        Spake2Session::start(Role::Responder, &pin, &transcript).expect("responder");

    let first = initiator
        .finish(&responder_message, &hash)
        .expect("initiator confirmation");
    let second = responder
        .finish(&initiator_message, &hash)
        .expect("responder confirmation");
    assert!(first.matches(&second));

    let wrong = PairingSecret::Pin("654321".to_owned());
    let (initiator, initiator_message) =
        Spake2Session::start(Role::Initiator, &pin, &transcript).expect("initiator");
    let (responder, responder_message) =
        Spake2Session::start(Role::Responder, &wrong, &transcript).expect("responder");
    let first = initiator
        .finish(&responder_message, &hash)
        .expect("initiator confirmation");
    let second = responder
        .finish(&initiator_message, &hash)
        .expect("responder confirmation");
    assert!(!first.matches(&second));
}

#[test]
fn pairing_frames_round_trip_and_reject_garbage() {
    let message = PairingMessage::Hello {
        node_id: NodeId::generate().to_string(),
        fingerprint: Fingerprint::from_bytes([9_u8; 32]).to_hex(),
        certificate_fingerprint: Fingerprint::from_bytes([8_u8; 32]).to_hex(),
        mode: PairingMode::Pin.as_u8(),
    };
    let frame = encode_frame(&message).expect("frame");
    assert_eq!(decode_message(frame.payload()).expect("decode"), message);
    assert!(decode_message(b"not a postcard pairing message").is_err());
}

#[test]
fn pairing_machine_tracks_confirmation_and_attempts() {
    let mut machine = PairingMachine::new();
    machine.start().expect("start");
    machine.receive_message().expect("message");
    machine.confirm(true).expect("confirm");
    assert_eq!(machine.state(), PairingState::Trusted);
}

fn transcript(mode: PairingMode, token: Option<[u8; 32]>) -> PairingTranscript {
    PairingTranscript::new(
        mode,
        PeerIdentity {
            node_id: "018f0000-0000-7000-8000-000000000001".to_owned(),
            fingerprint: "11".repeat(32),
            certificate_fingerprint: "22".repeat(32),
        },
        PeerIdentity {
            node_id: "018f0000-0000-7000-8000-000000000002".to_owned(),
            fingerprint: "33".repeat(32),
            certificate_fingerprint: "44".repeat(32),
        },
        token,
    )
}
