mod support;

use std::time::Duration;

use support::{TestNode, connect_pair, wait_true};
use ticonnect_core::call::{CallSignalFrame, CallState};
use ticonnect_daemon::calls::{CallError, CallEvent};

#[tokio::test]
async fn place_answer_and_hangup_call_between_two_nodes() {
    let first = TestNode::start("call-first").await;
    let second = TestNode::start("call-second").await;
    connect_pair(&first, &second).await;
    let mut first_events = first.calls.subscribe();
    let mut second_events = second.calls.subscribe();

    let call = first
        .calls
        .start_call(&second.node_id().to_string())
        .await
        .expect("start call");
    assert_eq!(call.state, CallState::Connecting);
    assert_eq!(first.calls.state().expect("outgoing state").id, call.id);

    let incoming = recv_event(&mut second_events, |event| match event {
        CallEvent::IncomingCall(info) if info.id == call.id => Some(info),
        _ => None,
    })
    .await
    .expect("incoming call");
    assert_eq!(incoming.user_id, first.node_id().to_string());
    assert_eq!(incoming.state, CallState::Ringing);
    recv_event(&mut first_events, |event| match event {
        CallEvent::StateChanged(info) if info.id == call.id && info.state == CallState::Ringing => {
            Some(())
        }
        _ => None,
    })
    .await
    .expect("caller ringing");
    assert_eq!(
        first.calls.state().expect("caller ringing state").state,
        CallState::Ringing
    );

    let accepted = second.calls.accept_call(call.id).await.expect("accept");
    assert_eq!(accepted.state, CallState::InCall);
    let repeated = second
        .calls
        .accept_call(call.id)
        .await
        .expect("repeat accept");
    assert_eq!(repeated.state, CallState::InCall);
    assert_eq!(repeated.connected_at, accepted.connected_at);
    assert_eq!(
        second.calls.state().expect("callee state").state,
        CallState::InCall
    );
    recv_event(&mut first_events, |event| match event {
        CallEvent::StateChanged(info) if info.id == call.id && info.state == CallState::InCall => {
            Some(())
        }
        _ => None,
    })
    .await
    .expect("caller in-call");

    let muted = first
        .calls
        .set_muted(call.id, true, false)
        .expect("mute input");
    assert!(muted.input_muted);
    assert!(!muted.output_muted);

    first.calls.hangup_call(call.id).await.expect("hangup");
    assert!(first.calls.state().is_none());
    recv_event(&mut first_events, |event| match event {
        CallEvent::Ended {
            call_id, reason, ..
        } if call_id == call.id && reason == "hangup" => Some(()),
        _ => None,
    })
    .await
    .expect("caller ended");
    recv_event(&mut second_events, |event| match event {
        CallEvent::Ended {
            call_id, reason, ..
        } if call_id == call.id && reason == "hangup" => Some(()),
        _ => None,
    })
    .await
    .expect("callee ended");
    wait_true(Duration::from_secs(5), || async {
        second.calls.state().is_none()
    })
    .await
    .expect("callee cleanup");

    let logs = first
        .database
        .list_call_logs(None, 10)
        .await
        .expect("call logs");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].id, call.id.to_string());
    assert_eq!(logs[0].user_id, second.node_id().to_string());
    assert_eq!(logs[0].direction, "outgoing");
    assert_eq!(logs[0].reason, "hangup");
    let cleared = first
        .database
        .clear_call_logs(Some(&second.node_id().to_string()))
        .await
        .expect("clear call logs");
    assert_eq!(cleared, 1);

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn accept_that_overtakes_ringing_still_connects_both_sides() {
    let first = TestNode::start("call-reorder-first").await;
    let second = TestNode::start("call-reorder-second").await;
    connect_pair(&first, &second).await;

    let call = first
        .calls
        .start_call(&second.node_id().to_string())
        .await
        .expect("start call");
    let callee = first
        .transport
        .session_by_node_id(second.node_id())
        .expect("callee session")
        .user()
        .clone();

    first
        .calls
        .handle_signal(callee.clone(), CallSignalFrame::Accept(call.id))
        .await
        .expect("early accept");
    first
        .calls
        .handle_signal(callee, CallSignalFrame::Ringing(call.id))
        .await
        .expect("late ringing");

    let caller = first.calls.state().expect("caller state");
    assert_eq!(caller.state, CallState::InCall);
    assert!(caller.connected_at.is_some());
    assert!(!matches!(
        first.calls.state(),
        Some(info) if info.state == CallState::Ended
    ));

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn immediate_accept_after_incoming_stays_in_call() {
    let first = TestNode::start("call-fast-first").await;
    let second = TestNode::start("call-fast-second").await;
    connect_pair(&first, &second).await;
    let mut second_events = second.calls.subscribe();

    let call = first
        .calls
        .start_call(&second.node_id().to_string())
        .await
        .expect("start call");
    recv_event(&mut second_events, |event| match event {
        CallEvent::IncomingCall(info) if info.id == call.id => Some(()),
        _ => None,
    })
    .await
    .expect("incoming call");

    second.calls.accept_call(call.id).await.expect("accept");
    wait_true(Duration::from_secs(5), || async {
        first
            .calls
            .state()
            .is_some_and(|info| info.state == CallState::InCall)
            && second
                .calls
                .state()
                .is_some_and(|info| info.state == CallState::InCall)
    })
    .await
    .expect("both sides in call");

    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(
        first.calls.state().expect("caller state").state,
        CallState::InCall
    );
    assert_eq!(
        second.calls.state().expect("callee state").state,
        CallState::InCall
    );

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn hangup_cancels_ringing_call_and_persists_both_sides() {
    let first = TestNode::start("call-cancel-first").await;
    let second = TestNode::start("call-cancel-second").await;
    connect_pair(&first, &second).await;

    let call = first
        .calls
        .start_call(&second.node_id().to_string())
        .await
        .expect("start call");
    wait_true(Duration::from_secs(5), || async {
        second.calls.state().is_some()
    })
    .await
    .expect("callee ringing");

    first
        .calls
        .hangup_call(call.id)
        .await
        .expect("cancel ringing");
    assert!(first.calls.state().is_none());
    wait_true(Duration::from_secs(5), || async {
        second.calls.state().is_none()
    })
    .await
    .expect("callee cleanup");

    for node in [&first, &second] {
        wait_true(Duration::from_secs(5), || async {
            node.database
                .list_call_logs(None, 10)
                .await
                .is_ok_and(|logs| logs.len() == 1)
        })
        .await
        .expect("call log persisted");
        let logs = node
            .database
            .list_call_logs(None, 10)
            .await
            .expect("call logs");
        assert_eq!(logs[0].id, call.id.to_string());
        assert_eq!(logs[0].reason, "hangup");
    }

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn decline_and_single_active_call_are_enforced() {
    let first = TestNode::start("call-decline-first").await;
    let second = TestNode::start("call-decline-second").await;
    connect_pair(&first, &second).await;
    let mut first_events = first.calls.subscribe();

    let call = first
        .calls
        .start_call(&second.node_id().to_string())
        .await
        .expect("start call");
    assert!(matches!(
        first.calls.start_call(&second.node_id().to_string()).await,
        Err(CallError::State(_))
    ));
    wait_true(Duration::from_secs(5), || async {
        second.calls.state().is_some()
    })
    .await
    .expect("callee ringing");

    second
        .calls
        .decline_call(call.id)
        .await
        .expect("decline call");
    recv_event(&mut first_events, |event| match event {
        CallEvent::Ended {
            call_id, reason, ..
        } if call_id == call.id && reason == "declined" => Some(()),
        _ => None,
    })
    .await
    .expect("caller declined");
    wait_true(Duration::from_secs(5), || async {
        first.calls.state().is_none() && second.calls.state().is_none()
    })
    .await
    .expect("decline cleanup");

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn ignore_policy_rejects_incoming_call_and_caller_can_cancel() {
    let first = TestNode::start("call-ignore-first").await;
    let second = TestNode::start("call-ignore-second").await;
    connect_pair(&first, &second).await;
    second
        .messaging
        .set_user_policy(
            &first.node_id().to_string(),
            ticonnect_daemon::storage::UserPolicy::Ignore,
        )
        .await
        .expect("ignore caller");
    let mut second_events = second.calls.subscribe();

    let call = first
        .calls
        .start_call(&second.node_id().to_string())
        .await
        .expect("start call");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(second.calls.state().is_none(), "ignored call started");
    assert!(
        recv_event(&mut second_events, |event| match event {
            CallEvent::IncomingCall(_) => Some(()),
            _ => None,
        })
        .await
        .is_none(),
        "ignored call emitted IncomingCall"
    );
    first
        .calls
        .decline_call(call.id)
        .await
        .expect("cancel ringing");
    assert!(first.calls.state().is_none());

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn ringing_call_times_out_and_cleans_up() {
    let first = TestNode::start("call-timeout-first").await;
    let second = TestNode::start("call-timeout-second").await;
    connect_pair(&first, &second).await;
    let mut first_settings = first.calls.settings();
    first_settings.set_call_timeout_secs(5);
    first.calls.apply_settings(first_settings.clone());
    second.calls.apply_settings(first_settings);
    let mut first_events = first.calls.subscribe();

    let call = first
        .calls
        .start_call(&second.node_id().to_string())
        .await
        .expect("start call");
    recv_event(&mut first_events, |event| match event {
        CallEvent::Ended {
            call_id, reason, ..
        } if call_id == call.id && reason == "timeout" => Some(()),
        _ => None,
    })
    .await
    .expect("timeout");
    assert!(first.calls.state().is_none());
    wait_true(Duration::from_secs(5), || async {
        second.calls.state().is_none()
    })
    .await
    .expect("callee timeout cleanup");

    first.shutdown().await;
    second.shutdown().await;
}

async fn recv_event<T>(
    receiver: &mut tokio::sync::broadcast::Receiver<CallEvent>,
    mut predicate: impl FnMut(CallEvent) -> Option<T>,
) -> Option<T> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if let Some(value) = predicate(event) {
                        return Some(value);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => return None,
            }
        }
    })
    .await
    .ok()
    .flatten()
}
