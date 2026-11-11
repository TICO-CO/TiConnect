use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CandidateSource {
    Mdns,
    Broadcast,
}

impl CandidateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mdns => "mdns",
            Self::Broadcast => "broadcast",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    node_id: String,
    fingerprint: String,
    display_name: String,
    addresses: BTreeSet<String>,
    port: u16,
    protocol_version: u16,
    source: CandidateSource,
    last_seen: Instant,
    last_seen_unix_ms: u64,
}

impl Candidate {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        node_id: String,
        fingerprint: String,
        display_name: String,
        addresses: BTreeSet<String>,
        port: u16,
        protocol_version: u16,
        source: CandidateSource,
        now: Instant,
    ) -> Self {
        Self {
            node_id,
            fingerprint,
            display_name,
            addresses,
            port,
            protocol_version,
            source,
            last_seen: now,
            last_seen_unix_ms: unix_millis(),
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn addresses(&self) -> impl Iterator<Item = &str> {
        self.addresses.iter().map(String::as_str)
    }

    pub fn addresses_display(&self) -> String {
        self.addresses
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    pub fn source(&self) -> CandidateSource {
        self.source
    }

    pub fn last_seen_unix_ms(&self) -> u64 {
        self.last_seen_unix_ms
    }

    fn absorb(&mut self, other: Self) {
        self.addresses.extend(other.addresses);
        if self.display_name.is_empty() {
            self.display_name = other.display_name;
        }
        if self.port == 0 {
            self.port = other.port;
        }
        self.last_seen = other.last_seen;
        self.last_seen_unix_ms = other.last_seen_unix_ms;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateChange {
    Found(Candidate),
    Updated,
    Dropped,
}

pub(crate) struct CandidateCache {
    entries: HashMap<String, Candidate>,
    dropped: u64,
    max_candidates: usize,
}

impl CandidateCache {
    pub(crate) fn new(max_candidates: usize) -> Self {
        Self {
            entries: HashMap::new(),
            dropped: 0,
            max_candidates,
        }
    }

    pub(crate) fn upsert(&mut self, candidate: Candidate) -> CandidateChange {
        let fingerprint = candidate.fingerprint().to_owned();
        if let Some(existing) = self.entries.get_mut(&fingerprint) {
            existing.absorb(candidate);
            return CandidateChange::Updated;
        }

        if self.entries.len() >= self.max_candidates {
            self.dropped = self.dropped.saturating_add(1);
            return CandidateChange::Dropped;
        }

        self.entries.insert(fingerprint, candidate.clone());
        CandidateChange::Found(candidate)
    }

    pub(crate) fn expire(&mut self, now: Instant, ttl: Duration) -> Vec<Candidate> {
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, candidate)| now.duration_since(candidate.last_seen) > ttl)
            .map(|(fingerprint, _)| fingerprint.clone())
            .collect();

        expired
            .into_iter()
            .filter_map(|fingerprint| self.entries.remove(&fingerprint))
            .collect()
    }

    pub(crate) fn clear(&mut self) -> Vec<Candidate> {
        self.entries
            .drain()
            .map(|(_, candidate)| candidate)
            .collect()
    }

    pub(crate) fn remove(&mut self, fingerprint: &str) -> Option<Candidate> {
        self.entries.remove(fingerprint)
    }

    pub(crate) fn snapshot(&self) -> Vec<Candidate> {
        let mut candidates: Vec<Candidate> = self.entries.values().cloned().collect();
        candidates.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
        candidates
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }

    pub(crate) fn note_dropped(&mut self) {
        self.dropped = self.dropped.saturating_add(1);
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
