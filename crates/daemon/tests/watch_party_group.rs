mod support;

use std::time::Duration;

use support::{TestNode, connect_pair, wait_for, wait_true};

#[tokio::test]
async fn group_watch_party_syncs_roster_and_playback() {
    let host = TestNode::start("party-host").await;
    let first = TestNode::start("party-first").await;
    let second = TestNode::start("party-second").await;
    connect_pair(&host, &first).await;
    connect_pair(&host, &second).await;

    let party = host
        .watch_party
        .start(
            &[
                first.node_id().to_string().as_str(),
                second.node_id().to_string().as_str(),
            ],
            "dQw4w9WgXcQ",
        )
        .await
        .expect("start group");
    assert_eq!(party.participants.len(), 1);
    assert_eq!(party.invited.len(), 2);

    wait_true(Duration::from_secs(5), || async {
        first.watch_party.snapshot().is_some() && second.watch_party.snapshot().is_some()
    })
    .await
    .expect("invites");

    for guest in [&first, &second] {
        let snapshot = guest.watch_party.snapshot().expect("guest party");
        assert_eq!(snapshot.role, "guest");
        assert_eq!(snapshot.host_id, host.node_id().to_string());
        assert_eq!(snapshot.video_id, "dQw4w9WgXcQ");
    }

    first
        .watch_party
        .accept(&party.party_id)
        .await
        .expect("first accepts");
    second
        .watch_party
        .accept(&party.party_id)
        .await
        .expect("second accepts");

    wait_true(Duration::from_secs(5), || async {
        host.watch_party
            .snapshot()
            .is_some_and(|snapshot| snapshot.participants.len() == 3)
            && first
                .watch_party
                .snapshot()
                .is_some_and(|snapshot| snapshot.participants.len() == 3)
            && second
                .watch_party
                .snapshot()
                .is_some_and(|snapshot| snapshot.participants.len() == 3)
    })
    .await
    .expect("three participants");

    host.watch_party
        .publish(&party.party_id, 42_000, true)
        .await
        .expect("host publishes");
    wait_true(Duration::from_secs(5), || async {
        first
            .watch_party
            .snapshot()
            .is_some_and(|snapshot| snapshot.position_ms == 42_000 && snapshot.playing)
            && second
                .watch_party
                .snapshot()
                .is_some_and(|snapshot| snapshot.position_ms == 42_000 && snapshot.playing)
    })
    .await
    .expect("guests see host playback");

    first
        .watch_party
        .publish(&party.party_id, 50_000, false)
        .await
        .expect("guest publishes");
    wait_true(Duration::from_secs(5), || async {
        host.watch_party
            .snapshot()
            .is_some_and(|snapshot| snapshot.position_ms == 50_000 && !snapshot.playing)
            && second
                .watch_party
                .snapshot()
                .is_some_and(|snapshot| snapshot.position_ms == 50_000 && !snapshot.playing)
    })
    .await
    .expect("everyone sees guest playback");

    second
        .watch_party
        .leave(&party.party_id)
        .await
        .expect("second leaves");
    wait_true(Duration::from_secs(5), || async {
        host.watch_party
            .snapshot()
            .is_some_and(|snapshot| snapshot.participants.len() == 2)
            && first
                .watch_party
                .snapshot()
                .is_some_and(|snapshot| snapshot.participants.len() == 2)
    })
    .await
    .expect("roster updates after leave");
    assert!(second.watch_party.snapshot().is_none());

    host.watch_party
        .end(&party.party_id)
        .await
        .expect("host ends");
    wait_for(Duration::from_secs(5), || async {
        if host.watch_party.snapshot().is_none() && first.watch_party.snapshot().is_none() {
            Some(())
        } else {
            None
        }
    })
    .await
    .expect("party ended everywhere");

    second.shutdown().await;
    first.shutdown().await;
    host.shutdown().await;
}
