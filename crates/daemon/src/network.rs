use std::collections::HashMap;
use std::time::{Duration, Instant};

use ticonnect_core::ipc;
use ticonnect_net::network::{LanState, LanStatus};
use zbus::OwnedVariant;

use crate::service::DaemonState;

const MAX_CHECK_SECONDS: u64 = 15;

pub fn status_map(state: &DaemonState) -> HashMap<String, OwnedVariant> {
    status_variant(ticonnect_net::network::inspect(
        state.discovery(),
        state.transport(),
    ))
}

pub async fn check_map(state: &DaemonState) -> HashMap<String, OwnedVariant> {
    let before = ticonnect_net::network::inspect(state.discovery(), state.transport());
    if matches!(
        before.state,
        LanState::Disabled
            | LanState::NoInterface
            | LanState::BindFailed
            | LanState::DiscoveryDown
            | LanState::Reachable
    ) {
        return status_variant(before);
    }

    let interval = u64::from(state.settings().discovery_beacon_interval_secs()).clamp(1, 5);
    let timeout = Duration::from_secs(interval.saturating_mul(3).min(MAX_CHECK_SECONDS));
    let deadline = Instant::now() + timeout;
    state.discovery().probe();

    loop {
        let status = ticonnect_net::network::inspect(state.discovery(), state.transport());
        if status.active_sessions > before.active_sessions
            || status.candidate_count > before.candidate_count
            || status.state == LanState::DiscoveryDown
            || Instant::now() >= deadline
        {
            return status_variant(status);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn status_variant(status: LanStatus) -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (
            ipc::NETWORK_STATE.to_owned(),
            OwnedVariant::from(zbus::Str::from(status.state.as_str())),
        ),
        (
            ipc::NETWORK_CONFIDENCE.to_owned(),
            OwnedVariant::from(zbus::Str::from(status.confidence)),
        ),
        (
            ipc::NETWORK_HINT.to_owned(),
            OwnedVariant::from(zbus::Str::from(status.hint.as_str())),
        ),
        (
            ipc::NETWORK_INTERFACES.to_owned(),
            OwnedVariant::from(zbus::Str::from(status.interfaces.as_str())),
        ),
        (
            ipc::NETWORK_INTERFACE_COUNT.to_owned(),
            OwnedVariant::from(status.interface_count),
        ),
        (
            ipc::NETWORK_TRANSPORT_PORT.to_owned(),
            OwnedVariant::from(u32::from(status.transport_port)),
        ),
        (
            ipc::NETWORK_TRANSPORT_BOUND.to_owned(),
            OwnedVariant::from(status.transport_bound),
        ),
        (
            ipc::NETWORK_MDNS_ACTIVE.to_owned(),
            OwnedVariant::from(status.mdns_active),
        ),
        (
            ipc::NETWORK_BROADCAST_ACTIVE.to_owned(),
            OwnedVariant::from(status.broadcast_active),
        ),
        (
            ipc::NETWORK_CANDIDATE_COUNT.to_owned(),
            OwnedVariant::from(status.candidate_count),
        ),
        (
            ipc::NETWORK_ACTIVE_SESSIONS.to_owned(),
            OwnedVariant::from(status.active_sessions),
        ),
        (
            ipc::NETWORK_BEACONS_SENT.to_owned(),
            OwnedVariant::from(status.beacons_sent),
        ),
        (
            ipc::NETWORK_BEACONS_RECEIVED.to_owned(),
            OwnedVariant::from(status.beacons_received),
        ),
        (
            ipc::NETWORK_BEACON_SEND_ERRORS.to_owned(),
            OwnedVariant::from(status.beacon_send_errors),
        ),
        (
            ipc::NETWORK_DROPPED_PACKETS.to_owned(),
            OwnedVariant::from(status.dropped_packets),
        ),
        (
            ipc::NETWORK_LAST_BEACON_SENT_AT.to_owned(),
            OwnedVariant::from(status.last_beacon_sent_at),
        ),
        (
            ipc::NETWORK_LAST_BEACON_RECEIVED_AT.to_owned(),
            OwnedVariant::from(status.last_beacon_received_at),
        ),
        (
            ipc::NETWORK_LAST_CANDIDATE_AT.to_owned(),
            OwnedVariant::from(status.last_candidate_at),
        ),
        (
            ipc::NETWORK_LAST_SESSION_AT.to_owned(),
            OwnedVariant::from(status.last_session_at),
        ),
        (
            ipc::NETWORK_LAST_FAILURE_AT.to_owned(),
            OwnedVariant::from(status.last_failure_at),
        ),
        (
            ipc::NETWORK_TRANSPORT_FAILURES.to_owned(),
            OwnedVariant::from(status.transport_failures),
        ),
    ])
}
