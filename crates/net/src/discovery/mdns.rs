use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;
use std::time::Instant;

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
use ticonnect_core::id::NodeId;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::candidate::{Candidate, CandidateSource};
use super::{
    DiscoveryConfig, DiscoveryError, MAX_ADDRESS_LENGTH, MAX_DISPLAY_NAME, PROTOCOL_VERSION,
    SERVICE_TYPE, Sink,
};

const TXT_VERSION: &str = "v";
const TXT_NODE_ID: &str = "id";
const TXT_FINGERPRINT: &str = "fp";
const TXT_DISPLAY_NAME: &str = "name";
const TXT_PORT: &str = "port";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxtRecord {
    pub protocol_version: u16,
    pub node_id: String,
    pub fingerprint: String,
    pub display_name: String,
    pub port: u16,
}

impl TxtRecord {
    pub fn new(
        protocol_version: u16,
        node_id: impl Into<String>,
        fingerprint: impl Into<String>,
        display_name: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            protocol_version,
            node_id: node_id.into(),
            fingerprint: fingerprint.into(),
            display_name: display_name.into(),
            port,
        }
    }

    pub fn encode(&self) -> HashMap<String, String> {
        HashMap::from([
            (TXT_VERSION.to_owned(), self.protocol_version.to_string()),
            (TXT_NODE_ID.to_owned(), self.node_id.clone()),
            (TXT_FINGERPRINT.to_owned(), self.fingerprint.clone()),
            (TXT_DISPLAY_NAME.to_owned(), self.display_name.clone()),
            (TXT_PORT.to_owned(), self.port.to_string()),
        ])
    }

    pub fn decode(properties: &HashMap<String, String>) -> Result<Self, DiscoveryError> {
        let version = properties
            .get(TXT_VERSION)
            .ok_or_else(|| DiscoveryError::InvalidRecord("missing TXT version".to_owned()))?
            .parse::<u16>()
            .map_err(|_| DiscoveryError::InvalidRecord("invalid TXT version".to_owned()))?;
        if version != PROTOCOL_VERSION {
            return Err(DiscoveryError::ProtocolVersion(version));
        }

        let node_id = properties
            .get(TXT_NODE_ID)
            .ok_or_else(|| DiscoveryError::InvalidRecord("missing TXT node id".to_owned()))?;
        NodeId::from_str(node_id)
            .map_err(|error| DiscoveryError::InvalidRecord(error.to_string()))?;

        let fingerprint = properties
            .get(TXT_FINGERPRINT)
            .ok_or_else(|| DiscoveryError::InvalidRecord("missing TXT fingerprint".to_owned()))?;
        if fingerprint.len() != 64 || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DiscoveryError::InvalidRecord(
                "invalid TXT fingerprint".to_owned(),
            ));
        }

        let display_name = properties
            .get(TXT_DISPLAY_NAME)
            .cloned()
            .unwrap_or_default();
        if display_name.len() > MAX_DISPLAY_NAME {
            return Err(DiscoveryError::InvalidRecord(
                "TXT display name is too long".to_owned(),
            ));
        }

        let port = properties
            .get(TXT_PORT)
            .map_or(Ok(0), |value| value.parse::<u16>())
            .map_err(|_| DiscoveryError::InvalidRecord("invalid TXT port".to_owned()))?;

        Ok(Self {
            protocol_version: version,
            node_id: node_id.clone(),
            fingerprint: fingerprint.to_ascii_lowercase(),
            display_name,
            port,
        })
    }

    fn from_resolved(resolved: &ResolvedService) -> Result<Self, DiscoveryError> {
        let mut properties = HashMap::new();
        for key in [
            TXT_VERSION,
            TXT_NODE_ID,
            TXT_FINGERPRINT,
            TXT_DISPLAY_NAME,
            TXT_PORT,
        ] {
            if let Some(value) = resolved.get_property_val_str(key) {
                properties.insert(key.to_owned(), value.to_owned());
            }
        }
        Self::decode(&properties)
    }
}

pub(crate) fn spawn(
    config: DiscoveryConfig,
    sink: Sink,
    mut shutdown: watch::Receiver<bool>,
) -> Result<JoinHandle<()>, DiscoveryError> {
    let daemon = ServiceDaemon::new().map_err(|error| DiscoveryError::Mdns(error.to_string()))?;
    let instance = instance_name(&config);
    let host = format!("ticonnect-{}.local.", compact_node_id(&config.node_id));
    let properties = TxtRecord::new(
        PROTOCOL_VERSION,
        config.node_id.clone(),
        config.fingerprint.clone(),
        config.display_name.clone(),
        0,
    )
    .encode();
    let ip = local_ip_argument();
    let service = ServiceInfo::new(SERVICE_TYPE, &instance, &host, ip.as_str(), 0, properties)
        .map_err(|error| DiscoveryError::Mdns(error.to_string()))?
        .enable_addr_auto();

    if let Err(error) = daemon.register(service) {
        let _ = daemon.shutdown();
        return Err(DiscoveryError::Mdns(error.to_string()));
    }
    let receiver = match daemon.browse(SERVICE_TYPE) {
        Ok(receiver) => receiver,
        Err(error) => {
            let _ = daemon.shutdown();
            return Err(DiscoveryError::Mdns(error.to_string()));
        }
    };

    Ok(tokio::spawn(async move {
        sink.set_mdns_active(true);
        let mut names: HashMap<String, String> = HashMap::new();

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                event = receiver.recv_async() => {
                    let Ok(event) = event else {
                        break;
                    };
                    match event {
                        ServiceEvent::ServiceResolved(resolved) => {
                            resolved_service(&resolved, &sink, &mut names);
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            if let Some(fingerprint) = names.remove(&fullname) {
                                sink.remove(&fingerprint);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        sink.set_mdns_active(false);
        let _ = daemon.shutdown();
    }))
}

fn resolved_service(resolved: &ResolvedService, sink: &Sink, names: &mut HashMap<String, String>) {
    let record = match TxtRecord::from_resolved(resolved) {
        Ok(record) => record,
        Err(error) => {
            tracing::debug!(%error, "discarded malformed mDNS TXT record");
            sink.note_dropped();
            return;
        }
    };

    let mut addresses: BTreeSet<String> = resolved
        .get_addresses_v4()
        .into_iter()
        .map(|address| address.to_string())
        .collect();
    addresses.retain(|address| address.len() <= MAX_ADDRESS_LENGTH);
    if addresses.is_empty() {
        tracing::debug!("discarded mDNS candidate with no IPv4 addresses");
        sink.note_dropped();
        return;
    }

    names.insert(
        resolved.get_fullname().to_owned(),
        record.fingerprint.clone(),
    );
    sink.offer(Candidate::new(
        record.node_id,
        record.fingerprint,
        record.display_name,
        addresses,
        record.port,
        record.protocol_version,
        CandidateSource::Mdns,
        Instant::now(),
    ));
}

fn instance_name(config: &DiscoveryConfig) -> String {
    let mut name: String = config
        .display_name
        .trim()
        .chars()
        .filter(|character| !character.is_control() && *character != '/' && *character != '\\')
        .collect();
    if name.is_empty() {
        name = config.node_id.clone();
    }
    let mut bounded = String::new();
    for character in name.chars() {
        if bounded.len() + character.len_utf8() > 63 {
            break;
        }
        bounded.push(character);
    }
    if bounded.is_empty() {
        compact_node_id(&config.node_id)
    } else {
        bounded
    }
}

fn compact_node_id(node_id: &str) -> String {
    node_id
        .chars()
        .filter(char::is_ascii_hexdigit)
        .take(32)
        .collect()
}

fn local_ip_argument() -> String {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return "127.0.0.1".to_owned();
    };
    let addresses: Vec<String> = interfaces
        .into_iter()
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(address) if !address.is_loopback() => Some(address.ip.to_string()),
            _ => None,
        })
        .collect();

    if addresses.is_empty() {
        "127.0.0.1".to_owned()
    } else {
        addresses.join(",")
    }
}
