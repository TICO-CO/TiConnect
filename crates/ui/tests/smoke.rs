use ticonnect_ui::model::{ConversationModel, PeerListModel};

#[test]
fn models_start_empty() {
    let peers = PeerListModel::default();
    assert!(peers.is_empty());
    assert!(peers.selected().is_none());

    let conversation = ConversationModel::default();
    assert!(conversation.is_empty());
    assert!(conversation.conversation_id().is_none());
}
