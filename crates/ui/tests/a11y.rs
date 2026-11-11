use ticonnect_ui::model::PeerPolicy;

#[test]
fn accessible_labels_name_every_policy() {
    assert_eq!(PeerPolicy::Accept.label(), "Accept");
    assert_eq!(PeerPolicy::Mute.label(), "Mute");
    assert_eq!(PeerPolicy::Ignore.label(), "Ignore");
}
