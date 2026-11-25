use ticonnect_core::ipc::events::{Call, CallEndReason, RemoteEndReason, WatchPartyEndReason};
use ticonnect_overlay::card::{
    AddOutcome, Card, CardKind, CardQueue, CardRouter, DEDUP_CAPACITY, MAX_VISIBLE, preview,
};

fn card(id: &str) -> Card {
    Card {
        id: id.to_owned(),
        user_id: format!("user-{id}"),
        sender: "Ada".to_owned(),
        preview: format!("body {id}"),
        detail: None,
        sent_at: 1,
        kind: CardKind::Message,
        persistent: false,
        actions: Vec::new(),
    }
}

#[test]
fn queue_caps_visible_and_promotes_pending() {
    let mut queue = CardQueue::new(1_000);
    for index in 0..5 {
        queue.enqueue(card(&index.to_string()));
    }
    assert_eq!(queue.visible().count(), MAX_VISIBLE);
    assert!(queue.advance(1_000));
    assert_eq!(queue.visible().count(), 2);
}

#[test]
fn hover_pauses_expiry() {
    let mut queue = CardQueue::new(1_000);
    queue.enqueue(card("a"));
    assert!(queue.set_hovered("a", true));
    assert!(!queue.advance(5_000));
    assert_eq!(queue.visible().count(), 1);
    queue.set_hovered("a", false);
    assert!(queue.advance(1_000));
    assert!(queue.is_empty());
}

#[test]
fn router_deduplicates_by_message_id() {
    let mut router = CardRouter::new(1_000);
    assert_eq!(
        router.route(card("a"), true, true, true),
        AddOutcome::Overlay
    );
    assert_eq!(
        router.route(card("a"), true, true, true),
        AddOutcome::Duplicate
    );
    router.dismiss("a");
    assert_eq!(
        router.route(card("a"), true, true, true),
        AddOutcome::Duplicate
    );
}

#[test]
fn dedup_history_is_bounded() {
    let mut router = CardRouter::new(1_000);
    for index in 0..=DEDUP_CAPACITY {
        router.route(card(&index.to_string()), true, true, true);
    }
    assert_eq!(
        router.route(card("0"), true, true, true),
        AddOutcome::Overlay
    );
}

#[test]
fn router_respects_notify_policy_and_overlay_state() {
    let mut router = CardRouter::new(1_000);
    assert_eq!(
        router.route(card("a"), false, true, true),
        AddOutcome::Suppressed
    );
    assert_eq!(
        router.route(card("b"), true, false, true),
        AddOutcome::Notification
    );
    assert_eq!(
        router.route(card("c"), true, true, false),
        AddOutcome::Notification
    );
}

#[test]
fn persistent_cards_do_not_expire() {
    let mut queue = CardQueue::new(1_000);
    let mut call = card("call");
    call.persistent = true;
    call.actions = vec![ticonnect_overlay::card::CardAction::AcceptCall];
    queue.enqueue(call);
    assert!(!queue.advance(60_000));
    assert_eq!(queue.visible().count(), 1);
}

#[test]
fn preview_collapses_whitespace_and_truncates() {
    assert_eq!(preview(" hello\n\n world "), "hello world");
    let long = "x".repeat(200);
    let preview = preview(&long);
    assert!(preview.ends_with("..."));
    assert!(preview.chars().count() <= 143);
}

#[test]
fn messages_use_toasts_and_requests_use_the_request_surface() {
    let mut router = CardRouter::new(1_000);
    let mut message = card("message");
    message.kind = CardKind::Message;
    let mut request = card("request");
    request.kind = CardKind::IncomingCall;
    request.persistent = true;
    assert_eq!(router.route(message, true, true, true), AddOutcome::Overlay);
    assert_eq!(router.route(request, true, true, true), AddOutcome::Overlay);
    assert_eq!(router.visible_toasts().count(), 1);
    assert_eq!(router.visible_requests().count(), 1);
}

#[test]
fn action_labels_match_the_design() {
    use ticonnect_overlay::card::CardAction;
    assert_eq!(CardAction::AcceptCall.label(), "Accept");
    assert_eq!(CardAction::DeclineCall.label(), "Decline");
    assert_eq!(CardAction::AcceptRemote.label(), "Accept");
    assert_eq!(CardAction::DenyRemote.label(), "Decline");
    assert_eq!(CardAction::HangupCall.label(), "End");
    assert_eq!(CardAction::OpenParty.label(), "Open");
}

#[test]
fn remote_request_card_shows_the_verified_fingerprint() {
    use ticonnect_core::ipc::events::RemoteSession;

    let session = RemoteSession {
        session_id: "session-1".to_owned(),
        user_id: "user-1".to_owned(),
        fingerprint: "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90".to_owned(),
        role: "host".to_owned(),
        state: "requested".to_owned(),
        indicator: false,
        input_allowed: true,
        expires_at: 1,
    };
    let card = Card::from_remote_request(&session, "Ada".to_owned());
    let detail = card.detail.expect("fingerprint detail");
    assert!(detail.starts_with("Identity: "));
    assert!(detail.contains("a1b2 c3d4 e5f6 0718"));
}

#[test]
fn connecting_calls_use_the_calling_card() {
    let call = Call {
        id: "call-1".to_owned(),
        user_id: "user-1".to_owned(),
        direction: "outgoing".to_owned(),
        state: "connecting".to_owned(),
        started_at: 1,
        connected_at: 0,
        input_muted: false,
        output_muted: false,
    };
    let card = Card::from_active_call(&call, "Ada".to_owned());
    assert_eq!(card.kind, CardKind::CallConnecting);
    assert_eq!(card.preview, "Connecting — waiting for answer");
}

#[test]
fn ended_cards_keep_the_typed_reason() {
    let call = Card::from_call_ended(
        "Ada".to_owned(),
        "user-1".to_owned(),
        "call-1".to_owned(),
        0,
        CallEndReason::Declined,
    );
    assert_eq!(call.kind, CardKind::CallEnded);
    assert!(call.preview.contains("declined"));

    let remote = Card::from_remote_ended(
        "Ada".to_owned(),
        "user-1".to_owned(),
        "session-1".to_owned(),
        RemoteEndReason::KillSwitch,
    );
    assert_eq!(remote.kind, CardKind::RemoteEnded);
    assert!(remote.preview.contains("stopped locally"));

    let party = Card::from_party_ended(
        "Ada".to_owned(),
        "user-1".to_owned(),
        "party-1".to_owned(),
        WatchPartyEndReason::HostDisconnected,
    );
    assert_eq!(party.kind, CardKind::PartyEnded);
    assert!(party.preview.contains("Host disconnected"));
}

#[test]
fn offline_card_carries_the_daemon_reason() {
    let card = Card::from_daemon_offline("bus not available".to_owned());
    assert_eq!(card.kind, CardKind::DaemonOffline);
    assert_eq!(card.detail.as_deref(), Some("bus not available"));
}

#[test]
fn lifecycle_toasts_route_to_the_toast_surface() {
    let mut router = CardRouter::new(1_000);
    let card = Card::from_remote_ended(
        "Ada".to_owned(),
        "user-1".to_owned(),
        "session-1".to_owned(),
        RemoteEndReason::Denied,
    );
    assert_eq!(router.route(card, true, true, true), AddOutcome::Overlay);
    assert_eq!(router.visible_toasts().count(), 1);
    assert_eq!(router.visible_requests().count(), 0);
}
