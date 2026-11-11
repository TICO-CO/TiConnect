use ticonnect_ui::model::{Peer, PeerListModel, PeerPolicy};

fn peer(id: &str, name: &str) -> Peer {
    Peer {
        id: id.to_owned(),
        fingerprint: "f".repeat(64),
        display_name: name.to_owned(),
        policy: PeerPolicy::Accept,
        trusted_at: 0,
        last_seen: 0,
        connected: false,
    }
}

#[test]
fn peer_list_updates_presence_and_policy() {
    let mut model = PeerListModel::default();
    model.replace(vec![peer("b", "Beta"), peer("a", "Alpha")]);
    assert_eq!(model.peers()[0].display_label(), "Alpha");
    model.set_connected("a", true);
    model.set_policy("a", PeerPolicy::Mute);
    assert!(model.peers()[0].connected);
    assert_eq!(model.peers()[0].policy.as_str(), "mute");
}

#[test]
fn selecting_unknown_peer_is_ignored() {
    let mut model = PeerListModel::default();
    model.replace(vec![peer("a", "Alpha")]);
    model.select("missing");
    assert!(model.selected().is_none());
    model.select("a");
    assert_eq!(model.selected().map(|peer| peer.id.as_str()), Some("a"));
}
