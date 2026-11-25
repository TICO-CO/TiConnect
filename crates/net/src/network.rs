use if_addrs::{IfAddr, get_if_addrs};

use crate::discovery::{Discovery, LanDiscovery};
use crate::transport::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanState {
    Disabled,
    NoInterface,
    BindFailed,
    DiscoveryDown,
    NoPeers,
    PeersDiscovered,
    PeersUnreachable,
    Reachable,
}

impl LanState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NoInterface => "no_interface",
            Self::BindFailed => "bind_failed",
            Self::DiscoveryDown => "discovery_down",
            Self::NoPeers => "no_peers",
            Self::PeersDiscovered => "peers_discovered",
            Self::PeersUnreachable => "peers_unreachable",
            Self::Reachable => "reachable",
        }
    }

    pub fn confidence(self) -> &'static str {
        match self {
            Self::Disabled
            | Self::NoInterface
            | Self::BindFailed
            | Self::DiscoveryDown
            | Self::Reachable => "confirmed",
            Self::PeersDiscovered | Self::PeersUnreachable => "probable",
            Self::NoPeers => "unknown",
        }
    }

    pub fn hint(self, transport_port: u16) -> String {
        match self {
            Self::Disabled => "Discovery is disabled in Settings.".to_owned(),
            Self::NoInterface => {
                "No non-loopback IPv4 address is available. Connect to Wi-Fi or Ethernet."
                    .to_owned()
            }
            Self::BindFailed => {
                "The transport socket could not bind. Check the configured port and other instances."
                    .to_owned()
            }
            Self::DiscoveryDown => {
                "Discovery sockets failed to start. Check sandbox permissions and port conflicts."
                    .to_owned()
            }
            Self::NoPeers => format!(
                "No peers replied. Start TiConnect on another device on the same network. If it still fails, allow UDP 5353, 45870, and {transport_port} in the firewall."
            ),
            Self::PeersDiscovered => {
                "Peers were found. Waiting for an encrypted transport session.".to_owned()
            }
            Self::PeersUnreachable => format!(
                "Peers were found but no transport session opened. Allow UDP 5353, 45870, and {transport_port} in the firewall. Also check client isolation or VPN settings."
            ),
            Self::Reachable => "At least one peer is connected over the LAN.".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanStatus {
    pub state: LanState,
    pub confidence: &'static str,
    pub hint: String,
    pub interfaces: String,
    pub interface_count: u32,
    pub transport_port: u16,
    pub transport_bound: bool,
    pub mdns_active: bool,
    pub broadcast_active: bool,
    pub candidate_count: u32,
    pub active_sessions: u32,
    pub beacons_sent: u64,
    pub beacons_received: u64,
    pub beacon_send_errors: u64,
    pub dropped_packets: u64,
    pub last_beacon_sent_at: u64,
    pub last_beacon_received_at: u64,
    pub last_candidate_at: u64,
    pub last_session_at: u64,
    pub last_failure_at: u64,
    pub transport_failures: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct LanInput {
    pub enabled: bool,
    pub interface_count: u32,
    pub transport_bound: bool,
    pub mdns_active: bool,
    pub broadcast_active: bool,
    pub candidate_count: u32,
    pub active_sessions: u32,
    pub transport_failures: u64,
    pub last_failure_at: u64,
    pub last_session_at: u64,
}

pub fn classify(input: LanInput) -> LanState {
    if !input.enabled {
        return LanState::Disabled;
    }
    if input.interface_count == 0 {
        return LanState::NoInterface;
    }
    if !input.transport_bound {
        return LanState::BindFailed;
    }
    if !input.mdns_active && !input.broadcast_active {
        return LanState::DiscoveryDown;
    }
    if input.active_sessions > 0 {
        return LanState::Reachable;
    }
    if input.candidate_count > 0 {
        if input.transport_failures > 0 && input.last_failure_at >= input.last_session_at {
            return LanState::PeersUnreachable;
        }
        return LanState::PeersDiscovered;
    }
    LanState::NoPeers
}

pub fn inspect(discovery: &LanDiscovery, transport: &Transport) -> LanStatus {
    let discovery_status = discovery.status();
    let metrics = transport.metrics();
    let local_addr = transport.local_addr().ok();
    let interfaces = lan_interfaces();
    let interface_count = u32::try_from(interfaces.len()).unwrap_or(u32::MAX);
    let transport_port = local_addr.map(|address| address.port()).unwrap_or(0);
    let candidate_count = u32::try_from(discovery_status.candidate_count).unwrap_or(u32::MAX);
    let active_sessions = u32::try_from(metrics.active_sessions).unwrap_or(u32::MAX);
    let last_candidate_at = discovery
        .snapshot()
        .iter()
        .map(|candidate| candidate.last_seen_unix_ms())
        .max()
        .unwrap_or(0);
    let state = classify(LanInput {
        enabled: discovery_status.enabled,
        interface_count,
        transport_bound: local_addr.is_some(),
        mdns_active: discovery_status.mdns_active,
        broadcast_active: discovery_status.broadcast_active,
        candidate_count,
        active_sessions,
        transport_failures: metrics.failures,
        last_failure_at: metrics.last_failure_at,
        last_session_at: metrics.last_session_at,
    });

    LanStatus {
        state,
        confidence: state.confidence(),
        hint: state.hint(transport_port),
        interfaces: interfaces.join(", "),
        interface_count,
        transport_port,
        transport_bound: local_addr.is_some(),
        mdns_active: discovery_status.mdns_active,
        broadcast_active: discovery_status.broadcast_active,
        candidate_count,
        active_sessions,
        beacons_sent: discovery_status.beacons_sent,
        beacons_received: discovery_status.beacons_received,
        beacon_send_errors: discovery_status.beacon_send_errors,
        dropped_packets: discovery_status.dropped_packets,
        last_beacon_sent_at: discovery_status.last_beacon_sent_at,
        last_beacon_received_at: discovery_status.last_beacon_received_at,
        last_candidate_at,
        last_session_at: metrics.last_session_at,
        last_failure_at: metrics.last_failure_at,
        transport_failures: metrics.failures,
    }
}

fn lan_interfaces() -> Vec<String> {
    let Ok(interfaces) = get_if_addrs() else {
        return Vec::new();
    };
    let mut values: Vec<String> = interfaces
        .into_iter()
        .filter_map(|interface| match interface.addr {
            IfAddr::V4(address) if !address.is_loopback() => {
                Some(format!("{}: {}", interface.name, address.ip))
            }
            _ => None,
        })
        .collect();
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> LanInput {
        LanInput {
            enabled: true,
            interface_count: 1,
            transport_bound: true,
            mdns_active: true,
            broadcast_active: true,
            candidate_count: 0,
            active_sessions: 0,
            transport_failures: 0,
            last_failure_at: 0,
            last_session_at: 0,
        }
    }

    #[test]
    fn disabled_wins() {
        let value = LanInput {
            enabled: false,
            ..input()
        };
        assert_eq!(classify(value), LanState::Disabled);
    }

    #[test]
    fn no_interface_is_confirmed() {
        let value = LanInput {
            interface_count: 0,
            ..input()
        };
        assert_eq!(classify(value), LanState::NoInterface);
        assert_eq!(LanState::NoInterface.confidence(), "confirmed");
    }

    #[test]
    fn active_session_is_reachable() {
        let value = LanInput {
            active_sessions: 1,
            ..input()
        };
        assert_eq!(classify(value), LanState::Reachable);
        assert_eq!(LanState::Reachable.confidence(), "confirmed");
    }

    #[test]
    fn peers_without_session_are_probably_blocked() {
        let value = LanInput {
            candidate_count: 2,
            transport_failures: 3,
            last_failure_at: 200,
            ..input()
        };
        assert_eq!(classify(value), LanState::PeersUnreachable);
        assert_eq!(LanState::PeersUnreachable.confidence(), "probable");
    }

    #[test]
    fn no_peers_stays_unknown() {
        assert_eq!(classify(input()), LanState::NoPeers);
        assert_eq!(LanState::NoPeers.confidence(), "unknown");
        assert!(LanState::NoPeers.hint(45871).contains("No peers replied"));
    }
}
