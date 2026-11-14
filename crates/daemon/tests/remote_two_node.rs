mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use support::{TestNode, connect_pair, wait_true};
use ticonnect_core::remote::{InputKind, InputPacket};
use ticonnect_daemon::remote::RemoteEventMessage;

#[tokio::test]
async fn consent_pin_indicator_and_kill_switch_between_two_nodes() {
    let first = TestNode::start("remote-first").await;
    let second = TestNode::start("remote-second").await;
    connect_pair(&first, &second).await;
    let mut first_events = first.remote.subscribe();
    let mut second_events = second.remote.subscribe();

    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request session");
    assert_eq!(viewer.role, "viewer");
    assert_eq!(viewer.state, "requested");

    let requested = recv_event(&mut second_events, |event| match event {
        RemoteEventMessage::Requested(snapshot) if snapshot.session_id == viewer.session_id => {
            Some(snapshot)
        }
        _ => None,
    })
    .await
    .expect("host request event");
    assert_eq!(requested.role, "host");
    assert_eq!(requested.state, "requested");
    assert_eq!(requested.pin.len(), 6);
    assert_eq!(requested.peer_id, first.node_id().to_string());

    second
        .remote
        .accept_session(&viewer.session_id, "")
        .await
        .expect("host consent");
    let _consented = recv_event(&mut first_events, |event| match event {
        RemoteEventMessage::Requested(snapshot)
            if snapshot.session_id == viewer.session_id && snapshot.state == "consented" =>
        {
            Some(snapshot)
        }
        _ => None,
    })
    .await
    .expect("viewer consent");

    first
        .remote
        .accept_session(&viewer.session_id, &requested.pin)
        .await
        .expect("viewer pin");
    recv_event(&mut first_events, |event| match event {
        RemoteEventMessage::Started(snapshot) if snapshot.session_id == viewer.session_id => {
            Some(())
        }
        _ => None,
    })
    .await
    .expect("viewer started");
    recv_event(&mut second_events, |event| match event {
        RemoteEventMessage::Started(snapshot) if snapshot.session_id == viewer.session_id => {
            Some(())
        }
        _ => None,
    })
    .await
    .expect("host started");
    recv_event(&mut second_events, |event| match event {
        RemoteEventMessage::Indicator {
            session_id,
            visible: true,
        } if session_id == viewer.session_id => Some(()),
        _ => None,
    })
    .await
    .expect("indicator visible");

    let host_active = second.remote.snapshot().expect("host session");
    assert_eq!(host_active.state, "active");
    assert!(host_active.indicator);
    let viewer_active = first.remote.snapshot().expect("viewer session");
    assert_eq!(viewer_active.state, "active");
    assert!(viewer_active.indicator);

    second
        .remote
        .end_session(&viewer.session_id)
        .await
        .expect("kill switch");
    assert!(second.remote.snapshot().is_none());
    wait_true(Duration::from_secs(5), || async {
        first.remote.snapshot().is_none()
    })
    .await
    .expect("viewer teardown");

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn wrong_pin_is_bounded_and_denies_session() {
    let first = TestNode::start("remote-pin-first").await;
    let second = TestNode::start("remote-pin-second").await;
    connect_pair(&first, &second).await;
    let mut second_events = second.remote.subscribe();

    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    let requested = recv_event(&mut second_events, |event| match event {
        RemoteEventMessage::Requested(snapshot) if snapshot.session_id == viewer.session_id => {
            Some(snapshot)
        }
        _ => None,
    })
    .await
    .expect("host request");
    second
        .remote
        .accept_session(&viewer.session_id, "")
        .await
        .expect("consent");

    let wrong = if requested.pin == "000000" {
        "000001"
    } else {
        "000000"
    };
    for attempt in 0..2 {
        first
            .remote
            .accept_session(&viewer.session_id, wrong)
            .await
            .expect("failed pin accepted");
        assert_eq!(
            second.remote.snapshot().expect("session").state,
            "consented",
            "attempt {attempt}"
        );
    }
    first
        .remote
        .accept_session(&viewer.session_id, wrong)
        .await
        .expect("third pin");
    recv_event(&mut second_events, |event| match event {
        RemoteEventMessage::Ended { session_id, reason }
            if session_id == viewer.session_id && reason == "pin" =>
        {
            Some(())
        }
        _ => None,
    })
    .await
    .expect("host pin denial");
    wait_true(Duration::from_secs(5), || async {
        first.remote.snapshot().is_none()
    })
    .await
    .expect("viewer denied");

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn deny_path_and_expiry_leave_no_session() {
    let first = TestNode::start("remote-deny-first").await;
    let second = TestNode::start("remote-deny-second").await;
    connect_pair(&first, &second).await;
    let mut first_events = first.remote.subscribe();
    let mut second_events = second.remote.subscribe();

    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    recv_event(&mut second_events, |event| match event {
        RemoteEventMessage::Requested(snapshot) if snapshot.session_id == viewer.session_id => {
            Some(())
        }
        _ => None,
    })
    .await
    .expect("host request");
    second
        .remote
        .deny_session(&viewer.session_id)
        .await
        .expect("deny");
    recv_event(&mut first_events, |event| match event {
        RemoteEventMessage::Ended { session_id, .. } if session_id == viewer.session_id => Some(()),
        _ => None,
    })
    .await
    .expect("viewer deny");
    wait_true(Duration::from_secs(5), || async {
        first.remote.snapshot().is_none()
    })
    .await
    .expect("viewer cleanup");
    assert!(second.remote.snapshot().is_none());

    let clock = Arc::new(AtomicI64::new(1_000_000));
    let clock_fn = {
        let clock = clock.clone();
        Arc::new(move || clock.load(Ordering::Relaxed)) as Arc<dyn Fn() -> i64 + Send + Sync>
    };
    let first = TestNode::start_with_clock("remote-expiry-first", clock_fn.clone()).await;
    let second = TestNode::start_with_clock("remote-expiry-second", clock_fn).await;
    connect_pair(&first, &second).await;
    let mut second_events = second.remote.subscribe();
    let viewer = first
        .remote
        .request_session(&second.node_id().to_string())
        .await
        .expect("request");
    recv_event(&mut second_events, |event| match event {
        RemoteEventMessage::Requested(snapshot) if snapshot.session_id == viewer.session_id => {
            Some(())
        }
        _ => None,
    })
    .await
    .expect("host request");
    clock.fetch_add(400_000, Ordering::Relaxed);
    assert!(matches!(
        second.remote.accept_session(&viewer.session_id, "").await,
        Err(ticonnect_daemon::remote::RemoteError::Expired)
    ));
    assert!(
        second.remote.snapshot().is_none(),
        "expired host session kept"
    );
    wait_true(Duration::from_secs(5), || async {
        first.remote.snapshot().is_none()
    })
    .await
    .expect("expired viewer session kept");

    first.shutdown().await;
    second.shutdown().await;
}

#[tokio::test]
async fn input_is_rejected_without_active_session() {
    let first = TestNode::start("remote-input-first").await;
    let second = TestNode::start("remote-input-second").await;
    connect_pair(&first, &second).await;
    let peer = second
        .transport
        .session_by_node_id(first.node_id())
        .expect("session")
        .peer()
        .clone();
    let packet = InputPacket {
        session_id: [0; 16],
        kind: InputKind::Motion,
        x: 10,
        y: 20,
        pressed: false,
    };
    assert!(matches!(
        second.remote.inject_input(peer, packet),
        Err(ticonnect_daemon::remote::RemoteError::Inactive)
    ));

    first.shutdown().await;
    second.shutdown().await;
}

async fn recv_event<T>(
    receiver: &mut tokio::sync::broadcast::Receiver<RemoteEventMessage>,
    mut predicate: impl FnMut(RemoteEventMessage) -> Option<T>,
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
