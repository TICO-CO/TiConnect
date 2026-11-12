use ticonnect_overlay::card::{
    AddOutcome, Card, CardQueue, CardRouter, DEDUP_CAPACITY, MAX_VISIBLE, preview,
};

fn card(id: &str) -> Card {
    Card {
        id: id.to_owned(),
        sender: "Ada".to_owned(),
        preview: format!("body {id}"),
        sent_at: 1,
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
fn preview_collapses_whitespace_and_truncates() {
    assert_eq!(preview(" hello\n\n world "), "hello world");
    let long = "x".repeat(200);
    let preview = preview(&long);
    assert!(preview.ends_with("..."));
    assert!(preview.chars().count() <= 143);
}
