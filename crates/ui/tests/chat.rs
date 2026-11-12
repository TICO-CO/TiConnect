use ticonnect_ui::model::{ConversationModel, Message, MessageState};

fn message(id: &str, sent_at: u64, sender: &str) -> Message {
    Message {
        id: id.to_owned(),
        conversation_id: "conversation".to_owned(),
        peer_id: "peer".to_owned(),
        sender_id: sender.to_owned(),
        local_sender: false,
        seq: sent_at,
        sent_at,
        received_at: sent_at,
        body: format!("body {id}"),
        state: MessageState::Sent,
        notify: true,
    }
}

#[test]
fn conversation_sorts_and_tracks_states() {
    let mut model = ConversationModel::default();
    model.reset(
        "conversation".to_owned(),
        "peer".to_owned(),
        2,
        vec![message("b", 2, "peer"), message("a", 1, "peer")],
    );
    assert_eq!(model.messages()[0].id, "a");
    model.update_state("a", MessageState::Read);
    assert_eq!(model.messages()[0].state, MessageState::Read);
}

#[test]
fn mark_read_skips_local_messages() {
    let mut model = ConversationModel::default();
    model.reset(
        "conversation".to_owned(),
        "peer".to_owned(),
        2,
        vec![message("a", 1, "local"), message("b", 2, "peer")],
    );
    let ids = model.mark_read_candidates(Some("local"));
    assert_eq!(ids, vec!["b".to_owned()]);
    assert!(model.mark_read_candidates(Some("local")).is_empty());
}
