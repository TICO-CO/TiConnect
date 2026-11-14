use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::NodeId;
use ticonnect_net::discovery::{Discovery, DiscoveryEvent, LanDiscovery};
use ticonnect_net::transport::{DialTarget, Transport, TransportEvent};
use tokio::task::JoinHandle;

use crate::trust::TrustStore;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(15);

pub fn spawn(
    transport: Arc<Transport>,
    discovery: Arc<LanDiscovery>,
    trust: Arc<TrustStore>,
) -> JoinHandle<()> {
    let mut discovery_events = discovery.subscribe();
    let mut transport_events = transport.subscribe();
    let mut interval = tokio::time::interval(RECONNECT_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let inflight = Arc::new(Mutex::new(HashSet::new()));

    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = discovery_events.recv() => {
                    match event {
                        Ok(DiscoveryEvent::Found(candidate)) => {
                            let Some((fingerprint, target)) = target(&candidate) else {
                                continue;
                            };
                            reconnect(transport.clone(), trust.clone(), inflight.clone(), fingerprint, target);
                        }
                        Ok(DiscoveryEvent::Lost(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                event = transport_events.recv() => {
                    if matches!(event, Ok(TransportEvent::PeerDisconnected(_))) {
                        reconnect_all(transport.clone(), discovery.clone(), trust.clone(), inflight.clone());
                    }
                    if matches!(event, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                        break;
                    }
                }
                _ = interval.tick() => {
                    reconnect_all(transport.clone(), discovery.clone(), trust.clone(), inflight.clone());
                }
            }
        }
    })
}

fn reconnect_all(
    transport: Arc<Transport>,
    discovery: Arc<LanDiscovery>,
    trust: Arc<TrustStore>,
    inflight: Arc<Mutex<HashSet<Fingerprint>>>,
) {
    let mut wanted = std::collections::HashMap::new();
    for candidate in discovery.snapshot() {
        if let Some((fingerprint, target)) = target(&candidate) {
            wanted.entry(fingerprint).or_insert(target);
        }
    }
    for (fingerprint, target) in wanted {
        reconnect(
            transport.clone(),
            trust.clone(),
            inflight.clone(),
            fingerprint,
            target,
        );
    }
}

fn reconnect(
    transport: Arc<Transport>,
    trust: Arc<TrustStore>,
    inflight: Arc<Mutex<HashSet<Fingerprint>>>,
    fingerprint: Fingerprint,
    target: DialTarget,
) {
    if !trust.is_trusted(&fingerprint.to_hex()) {
        return;
    }
    if target
        .node_id
        .is_some_and(|peer| transport.local_node_id() > peer)
    {
        return;
    }
    if transport
        .sessions()
        .iter()
        .any(|session| session.peer().fingerprint == fingerprint)
    {
        return;
    }
    if !inflight
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(fingerprint)
    {
        return;
    }

    tokio::spawn(async move {
        match transport.connect_with_retry(target).await {
            Ok(_) => tracing::debug!(%fingerprint, "reconnected to trusted peer"),
            Err(error) => {
                tracing::debug!(%error, %fingerprint, "trusted peer reconnect failed");
            }
        }
        inflight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&fingerprint);
    });
}

fn target(
    candidate: &ticonnect_net::discovery::candidate::Candidate,
) -> Option<(Fingerprint, DialTarget)> {
    let fingerprint = Fingerprint::from_str(candidate.fingerprint()).ok()?;
    let node_id = NodeId::from_str(candidate.node_id()).ok()?;
    let address = candidate
        .addresses()
        .next()
        .and_then(|address| address.parse::<IpAddr>().ok())?;
    if candidate.port() == 0 {
        return None;
    }
    let target = DialTarget::new(SocketAddr::new(address, candidate.port()), fingerprint)
        .with_node_id(node_id);
    Some((fingerprint, target))
}
