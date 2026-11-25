use ticonnect_net::trust::{
    Role, Spake2Session, TrustMessage, TrustSecret, TrustTranscript, UserIdentity, decode_message,
    encode_message,
};

#[test]
fn trust_frames_round_trip_and_reject_garbage() {
    let message = TrustMessage::Hello {
        node_id: "018f0000-0000-7000-8000-000000000001".to_owned(),
        fingerprint: "11".repeat(32),
        certificate_fingerprint: "22".repeat(32),
    };
    let bytes = encode_message(&message).expect("encode");
    assert_eq!(decode_message(&bytes).expect("decode"), message);
    assert!(decode_message(b"not a postcard trust message").is_err());
}

#[test]
fn trust_transcript_binds_both_identities_and_token() {
    let transcript = fixture([7_u8; 32]);
    let hash = transcript.hash().expect("hash");
    let other = fixture([8_u8; 32]);
    assert_ne!(hash, other.hash().expect("hash"));

    let secret = TrustSecret::new([7_u8; 32]);
    let (initiator, initiator_message) =
        Spake2Session::start(Role::Initiator, &secret, &transcript).expect("initiator");
    let (responder, responder_message) =
        Spake2Session::start(Role::Responder, &secret, &transcript).expect("responder");
    let initiator_confirmation = initiator
        .finish(&responder_message, &hash)
        .expect("initiator confirm");
    let responder_confirmation = responder
        .finish(&initiator_message, &hash)
        .expect("responder confirm");
    assert!(initiator_confirmation.matches(&responder_confirmation));
}

fn fixture(token: [u8; 32]) -> TrustTranscript {
    TrustTranscript::new(
        UserIdentity {
            node_id: "018f0000-0000-7000-8000-000000000001".to_owned(),
            fingerprint: "11".repeat(32),
            certificate_fingerprint: "22".repeat(32),
        },
        UserIdentity {
            node_id: "018f0000-0000-7000-8000-000000000002".to_owned(),
            fingerprint: "33".repeat(32),
            certificate_fingerprint: "44".repeat(32),
        },
        token,
    )
}
