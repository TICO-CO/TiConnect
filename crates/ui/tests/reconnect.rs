use ticonnect_ui::model::Backoff;

#[test]
fn reconnect_backoff_resets_after_connect() {
    let mut backoff = Backoff::default();
    assert_eq!(backoff.next_delay(), 1_000);
    assert_eq!(backoff.next_delay(), 2_000);
    backoff.reset();
    assert_eq!(backoff.next_delay(), 1_000);
}
