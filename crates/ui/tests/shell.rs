use ticonnect_ui::model::{Peer, PeerPolicy, PeerPresentation, short_id};

fn peer(name: &str) -> Peer {
    Peer {
        id: "12345678-1234-1234-1234-123456789abc".to_owned(),
        fingerprint: "a".repeat(64),
        display_name: name.to_owned(),
        policy: PeerPolicy::Accept,
        trusted_at: 0,
        last_seen: 0,
        connected: false,
    }
}

#[test]
fn shell_labels_have_a_fallback_without_a_name() {
    assert_eq!(peer("").display_label(), short_id(&peer("").id));
    assert_eq!(peer("Ada Lovelace").initials(), "AL");
}
