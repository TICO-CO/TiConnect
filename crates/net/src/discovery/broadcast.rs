use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

use if_addrs::{IfAddr, Ifv4Addr};
use socket2::{Domain, Protocol, Socket, Type};
use ticonnect_core::id::NodeId;
use tokio::net::UdpSocket;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::candidate::{Candidate, CandidateSource};
use super::{
    DiscoveryConfig, DiscoveryError, MAX_ADDRESS_LENGTH, MAX_DISPLAY_NAME, MAX_PACKET_SIZE,
    PROTOCOL_VERSION, Sink,
};

const MAGIC: &[u8; 8] = b"TICONNCT";
const HEADER_LEN: usize = MAGIC.len() + 2 + 16 + 32 + 2 + 1 + 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beacon {
    pub protocol_version: u16,
    pub node_id: String,
    pub fingerprint: String,
    pub port: u16,
    pub address: String,
    pub display_name: String,
}

impl Beacon {
    pub fn new(
        protocol_version: u16,
        node_id: impl Into<String>,
        fingerprint: impl Into<String>,
        port: u16,
        address: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            protocol_version,
            node_id: node_id.into(),
            fingerprint: fingerprint.into(),
            port,
            address: address.into(),
            display_name: display_name.into(),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, DiscoveryError> {
        let node_id = NodeId::from_str(&self.node_id)
            .map_err(|error| DiscoveryError::InvalidRecord(error.to_string()))?;
        let fingerprint = hex::decode(&self.fingerprint)
            .map_err(|_| DiscoveryError::InvalidRecord("invalid fingerprint".to_owned()))?;
        if fingerprint.len() != 32 {
            return Err(DiscoveryError::InvalidRecord(
                "fingerprint must be 32 bytes".to_owned(),
            ));
        }
        if self.address.len() > MAX_ADDRESS_LENGTH {
            return Err(DiscoveryError::InvalidRecord(
                "address is too long".to_owned(),
            ));
        }
        IpAddr::from_str(&self.address)
            .map_err(|_| DiscoveryError::InvalidRecord("invalid address".to_owned()))?;
        if self.display_name.len() > MAX_DISPLAY_NAME {
            return Err(DiscoveryError::InvalidRecord(
                "display name is too long".to_owned(),
            ));
        }

        let mut bytes =
            Vec::with_capacity(HEADER_LEN + self.address.len() + self.display_name.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(node_id.as_uuid().as_bytes());
        bytes.extend_from_slice(&fingerprint);
        bytes.extend_from_slice(&self.port.to_be_bytes());
        let address = self.address.as_bytes();
        bytes.push(u8::try_from(address.len()).unwrap_or(u8::MAX));
        bytes.extend_from_slice(address);
        let display_name = self.display_name.as_bytes();
        bytes.push(u8::try_from(display_name.len()).unwrap_or(u8::MAX));
        bytes.extend_from_slice(display_name);
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DiscoveryError> {
        if bytes.len() > MAX_PACKET_SIZE {
            return Err(DiscoveryError::InvalidRecord(
                "beacon exceeds maximum packet size".to_owned(),
            ));
        }
        if bytes.len() < HEADER_LEN {
            return Err(DiscoveryError::InvalidRecord(
                "beacon is truncated".to_owned(),
            ));
        }
        if &bytes[0..MAGIC.len()] != MAGIC {
            return Err(DiscoveryError::InvalidRecord(
                "beacon magic does not match".to_owned(),
            ));
        }

        let mut cursor = MAGIC.len();
        let protocol_version = u16::from_be_bytes(read_array::<2>(bytes, &mut cursor)?);
        if protocol_version != PROTOCOL_VERSION {
            return Err(DiscoveryError::ProtocolVersion(protocol_version));
        }
        let node_id = uuid_string(&read_array::<16>(bytes, &mut cursor)?);
        let fingerprint = hex::encode(read_array::<32>(bytes, &mut cursor)?);
        let port = u16::from_be_bytes(read_array::<2>(bytes, &mut cursor)?);
        let address_length = usize::from(read_array::<1>(bytes, &mut cursor)?[0]);
        if address_length > MAX_ADDRESS_LENGTH {
            return Err(DiscoveryError::InvalidRecord(
                "address is too long".to_owned(),
            ));
        }
        let address = read_string(bytes, &mut cursor, address_length)?;
        IpAddr::from_str(&address)
            .map_err(|_| DiscoveryError::InvalidRecord("invalid address".to_owned()))?;
        let display_name_length = usize::from(read_array::<1>(bytes, &mut cursor)?[0]);
        if display_name_length > MAX_DISPLAY_NAME {
            return Err(DiscoveryError::InvalidRecord(
                "display name is too long".to_owned(),
            ));
        }
        let display_name = read_string(bytes, &mut cursor, display_name_length)?;
        if cursor != bytes.len() {
            return Err(DiscoveryError::InvalidRecord(
                "beacon has trailing bytes".to_owned(),
            ));
        }

        Ok(Self {
            protocol_version,
            node_id,
            fingerprint,
            port,
            address,
            display_name,
        })
    }
}

pub(crate) fn spawn(
    config: DiscoveryConfig,
    sink: Sink,
    shutdown: watch::Receiver<bool>,
) -> Result<JoinHandle<()>, DiscoveryError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), config.beacon_port).into())?;
    let std_socket: std::net::UdpSocket = socket.into();
    let socket = UdpSocket::from_std(std_socket)?;

    let targets = beacon_targets(&config);
    Ok(tokio::spawn(async move {
        run(config, socket, targets, sink, shutdown).await;
    }))
}

async fn run(
    config: DiscoveryConfig,
    socket: UdpSocket,
    targets: Vec<Target>,
    sink: Sink,
    mut shutdown: watch::Receiver<bool>,
) {
    sink.set_broadcast_active(true);
    let mut ticker = tokio::time::interval(config.beacon_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut buffer = vec![0_u8; MAX_PACKET_SIZE + 1];

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = ticker.tick() => {
                send_beacons(&config, &socket, &targets, &sink).await;
            }
            received = socket.recv_from(&mut buffer) => {
                match received {
                    Ok((length, source)) => {
                        if length > MAX_PACKET_SIZE {
                            sink.note_dropped();
                            continue;
                        }
                        receive_beacon(&buffer[..length], source, &sink);
                    }
                    Err(error) => {
                        sink.note_dropped();
                        tracing::debug!(%error, "failed to receive discovery beacon");
                    }
                }
            }
        }
    }

    sink.set_broadcast_active(false);
}

async fn send_beacons(
    config: &DiscoveryConfig,
    socket: &UdpSocket,
    targets: &[Target],
    sink: &Sink,
) {
    for target in targets {
        let beacon = Beacon::new(
            PROTOCOL_VERSION,
            config.node_id.clone(),
            config.fingerprint.clone(),
            config.transport_port,
            target.local.to_string(),
            config.display_name.clone(),
        );
        let bytes = match beacon.encode() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::debug!(%error, "failed to encode discovery beacon");
                continue;
            }
        };
        if let Err(error) = socket.send_to(&bytes, target.destination).await {
            tracing::debug!(%error, destination = %target.destination, "failed to send discovery beacon");
            sink.note_dropped();
        }
    }
}

fn receive_beacon(bytes: &[u8], source: SocketAddr, sink: &Sink) {
    let beacon = match Beacon::decode(bytes) {
        Ok(beacon) => beacon,
        Err(error) => {
            tracing::debug!(%error, "discarded malformed discovery beacon");
            sink.note_dropped();
            return;
        }
    };

    let mut addresses = BTreeSet::new();
    addresses.insert(beacon.address);
    if !source.ip().is_unspecified() && !source.ip().is_multicast() {
        addresses.insert(source.ip().to_string());
    }

    sink.offer(Candidate::new(
        beacon.node_id,
        beacon.fingerprint,
        beacon.display_name,
        addresses,
        beacon.port,
        beacon.protocol_version,
        CandidateSource::Broadcast,
        std::time::Instant::now(),
    ));
}

#[derive(Debug, Clone, Copy)]
struct Target {
    destination: SocketAddr,
    local: IpAddr,
}

fn beacon_targets(config: &DiscoveryConfig) -> Vec<Target> {
    if let Some(targets) = &config.beacon_targets {
        return targets
            .iter()
            .map(|destination| Target {
                destination: *destination,
                local: if destination.ip().is_unspecified() {
                    IpAddr::V4(Ipv4Addr::LOCALHOST)
                } else {
                    destination.ip()
                },
            })
            .collect();
    }

    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return vec![Target {
            destination: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), config.beacon_port),
            local: IpAddr::V4(Ipv4Addr::LOCALHOST),
        }];
    };

    let mut targets: Vec<Target> = interfaces
        .into_iter()
        .filter_map(|interface| match interface.addr {
            IfAddr::V4(interface) if !interface.is_loopback() => {
                Some(v4_target(&interface, config))
            }
            _ => None,
        })
        .collect();

    if targets.is_empty() {
        targets.push(Target {
            destination: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), config.beacon_port),
            local: IpAddr::V4(Ipv4Addr::LOCALHOST),
        });
    }
    targets
}

fn v4_target(interface: &Ifv4Addr, config: &DiscoveryConfig) -> Target {
    let broadcast = interface
        .broadcast
        .unwrap_or_else(|| Ipv4Addr::from(u32::from(interface.ip) | !u32::from(interface.netmask)));
    Target {
        destination: SocketAddr::new(broadcast.into(), config.beacon_port),
        local: interface.ip.into(),
    }
}

fn read_array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], DiscoveryError> {
    let end = cursor
        .checked_add(N)
        .ok_or_else(|| DiscoveryError::InvalidRecord("beacon is truncated".to_owned()))?;
    let slice = bytes
        .get(*cursor..end)
        .ok_or_else(|| DiscoveryError::InvalidRecord("beacon is truncated".to_owned()))?;
    *cursor = end;
    let mut value = [0_u8; N];
    value.copy_from_slice(slice);
    Ok(value)
}

fn read_string(bytes: &[u8], cursor: &mut usize, length: usize) -> Result<String, DiscoveryError> {
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| DiscoveryError::InvalidRecord("beacon is truncated".to_owned()))?;
    let slice = bytes
        .get(*cursor..end)
        .ok_or_else(|| DiscoveryError::InvalidRecord("beacon is truncated".to_owned()))?;
    *cursor = end;
    String::from_utf8(slice.to_vec())
        .map_err(|_| DiscoveryError::InvalidRecord("invalid UTF-8".to_owned()))
}

fn uuid_string(bytes: &[u8; 16]) -> String {
    let hex = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
