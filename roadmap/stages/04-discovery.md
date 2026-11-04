# Stage 4 — Discovery

## Goal

Advertise this node and discover other nodes on the LAN using mDNS, with UDP
broadcast as a fallback. Discovery produces candidates only; it never implies
trust. The daemon exposes the candidate list over D-Bus.

## Scope

- mDNS service advertisement and browsing via `mdns-sd`.
- TXT record with protocol version, node id, and fingerprint.
- UDP broadcast beacon and listener fallback.
- Candidate cache with expiry and deduplication.
- D-Bus methods and signals for candidates.

## Non-goals

- No pairing, TLS, or message exchange.
- No relay, WAN, or internet discovery.
- No UI.
- No trust decisions based on discovery data.

## Prerequisites

Stages 1 and 3 complete.

## Deliverables

- `crates/net/src/discovery/mod.rs`
- `crates/net/src/discovery/mdns.rs`
- `crates/net/src/discovery/broadcast.rs`
- `crates/net/src/discovery/candidate.rs`
- `crates/daemon/src/discovery_service.rs`
- D-Bus additions in `crates/daemon/src/service.rs`.

## Protocol

- [ ] Service type `_ticonnect._udp.local.`.
- [ ] Instance name is the display name or node id, sanitized.
- [ ] TXT keys: `v` (protocol version), `id` (node id), `fp` (fingerprint),
  `name` (display name), `port` (transport port placeholder).
- [ ] UDP fallback beacon on a fixed port with a magic header, version, node id,
  fingerprint, and address.
- [ ] Beacon interval configurable; default 5 seconds.
- [ ] Candidate expiry: 3 missed intervals or a fixed TTL, whichever is clearer.
- [ ] Ignore own node id and own fingerprint.
- [ ] Reject candidates with a mismatched protocol major version.

## Implementation

- [ ] `Discovery` trait with `start`, `stop`, `snapshot`, `subscribe`.
- [ ] mDNS backend advertises on start and browses continuously.
- [ ] Handle `ServiceResolved`, `ServiceRemoved`, and TXT updates.
- [ ] Broadcast backend opens a UDP socket, sets `SO_BROADCAST`, and sends
  beacons to the subnet broadcast address.
- [ ] Broadcast listener parses and validates beacons; malformed packets are
  dropped and counted.
- [ ] Run mDNS and broadcast concurrently; deduplicate by fingerprint.
- [ ] Candidate struct: node id, fingerprint, display name, addresses, port,
  protocol version, source (`mdns` or `broadcast`), last_seen.
- [ ] Candidate store is behind a lock or actor; snapshot is cheap.
- [ ] Graceful stop closes sockets and stops advertise/browse.
- [ ] Config keys: `discovery.enabled`, `discovery.mdns`,
  `discovery.broadcast`, `discovery.beacon_interval_secs`.

## D-Bus additions

- [ ] `ListCandidates() -> aa{sv}`.
- [ ] Signal `CandidateFound(a{sv})`.
- [ ] Signal `CandidateLost(s)` with fingerprint or node id.
- [ ] `GetDiscoveryStatus() -> a{sv}` with enabled backends and candidate count.

## CLI additions

- [ ] `ticonnect-cli discover list`.
- [ ] `ticonnect-cli discover watch` prints found/lost events.

## Tests

- [ ] Unit test: TXT record encode/decode round-trip.
- [ ] Unit test: beacon encode/decode round-trip.
- [ ] Unit test: malformed and truncated packets are rejected without panic.
- [ ] Unit test: self-candidate is ignored.
- [ ] Unit test: duplicate candidates from mDNS and broadcast deduplicate.
- [ ] Integration test: two discovery instances on loopback find each other via
  broadcast.
- [ ] Manual test: two machines on the same LAN find each other via mDNS.
- [ ] Manual test: disable mDNS and confirm broadcast fallback still works.

## Security

- [ ] Discovery data is untrusted input; validate every field.
- [ ] Cap candidate count and packet size to prevent memory abuse.
- [ ] Do not attempt any connection or data exchange from discovery alone.
- [ ] Do not log full fingerprints at info level by default.
- [ ] Document that discovery is not authentication.

## Docs and acceptance

- [ ] Document TXT keys and beacon format.
- [ ] Document ports and firewall expectations.
- [ ] Document behavior when mDNS is blocked or unavailable.

## Acceptance criteria

- Two nodes on the same LAN appear in each other's candidate list.
- Stopping one node removes it after expiry.
- Disabling mDNS still yields candidates through broadcast.
- No trust or transport activity occurs from discovery alone.

## Demo

1. Start daemon on two machines.
2. `ticonnect-cli discover watch` on both.
3. Stop one daemon; observe `CandidateLost`.
4. Disable mDNS; repeat with broadcast only.

## Risks and mitigations

- Risk: mDNS blocked by network or avahi conflicts. Mitigation: broadcast
  fallback and configurable backends.
- Risk: spoofed candidates. Mitigation: candidates are hints only; pairing and
  pinning decide trust.
- Risk: multicast storms on large LANs. Mitigation: beacon interval config and
  candidate caps.

## Works without later stages

Candidates are listed and expire with no pairing, transport, or UI. Nothing is
exchanged beyond discovery metadata.
