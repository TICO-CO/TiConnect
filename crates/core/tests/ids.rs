use std::str::FromStr;

use ticonnect_core::id::NodeId;

#[test]
fn generated_node_id_is_uuid_v7_and_round_trips() {
    let node_id = NodeId::generate();

    assert_eq!(node_id.as_uuid().get_version_num(), 7);
    assert_eq!(
        NodeId::from_str(&node_id.to_string()).expect("parse"),
        node_id
    );
}

#[test]
fn node_ids_are_unique() {
    assert_ne!(NodeId::generate(), NodeId::generate());
}

#[test]
fn invalid_node_id_is_rejected() {
    assert!(NodeId::from_str("ticonnect").is_err());
}
