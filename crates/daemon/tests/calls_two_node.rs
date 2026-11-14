mod support;

use std::time::Duration;

use support::{TestNode, connect_pair, wait_for, wait_true};
use ticonnect_core::call::CallState;
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
    assert_eq!(call.state, CallState::Ringing);
    assert_eq!(first.calls.state().expect("outgoing state").id, call.id);

    let incoming = recv_event(&mut second_events, |event| match event {
        CallEvent::IncomingCall(info) if info.id == call.id => Some(info),
        _ => None,
    })
    .await
    .expect("incoming call");
    assert_eq!(incoming.peer_id, first.node_id().to_string());
    assert_eq!(incoming.state, CallState::Ringing);

    let accepted = second.calls.accept_call(call.id).await.expect("accept");
    assert_eq!(accepted.state, CallState::InCall);
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
        .set_peer_policy(
            &first.node_id().to_string(),
            ticonnect_daemon::storage::PeerPolicy::Ignore,
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
