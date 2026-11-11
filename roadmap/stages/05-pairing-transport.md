# Stage 5 — Pairing & Transport

## Goal

Establish authenticated, encrypted QUIC connections between paired nodes. Pairing
uses SPAKE2 with a 6-digit PIN or a QR code, then pins the peer certificate to
its fingerprint. Unpaired peers can reach only the pairing endpoint.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-5).
Component graphs: [PAIR](../../docs/systems/generated/components/pair.md), [TX](../../docs/systems/generated/components/tx.md), [WIRE](../../docs/systems/generated/components/wire.md).

## Scope

- SPAKE2 pairing over a dedicated pairing path.
- PIN display/entry flow and QR payload.
- QUIC server and client with `quinn` over `rustls`.
- Self-signed certificates bound to the Ed25519 identity.
- Certificate pinning against the trusted-peer table.
- Connection/session management with reconnection.
- D-Bus pairing methods and signals.

## Non-goals

- No messages, files, or media yet.
- No UI; CLI handles PIN entry.
- No group pairing, no multi-device identity, no key rotation.

## Prerequisites

Stages 1, 1.5, 3, and 4 complete.

## Deliverables

- `crates/net/src/pairing/{mod.rs,spake2.rs,qr.rs}`
- `crates/net/src/tls.rs`
- `crates/net/src/transport/{mod.rs,server.rs,client.rs,session.rs}`
- `crates/net/src/trust.rs`
- `crates/daemon/src/pairing_service.rs`
- D-Bus additions in `crates/daemon/src/service.rs`.

## Pairing protocol

- [x] Pairing endpoint listens on a fixed or advertised port, separate from the
  post-pairing session handler.
- [x] SPAKE2 uses `spake2` with a 6-digit PIN encoded as the shared secret.
- [x] PIN is generated with a CSPRNG, displayed on the initiating side, and
  entered on the responding side.
- [x] QR payload contains node id, address, port, fingerprint, and a one-time
  pairing token with short TTL.
- [x] PIN flow has a bounded number of attempts and a timeout.
- [x] Pairing messages are length-prefixed and validated before processing.
- [x] On success, both sides derive and compare a confirmation value.
- [x] On success, insert the peer into the trusted-peer table with fingerprint,
  display name, and `trusted_at`.
- [x] Emit `PairingCompleted` with peer id and fingerprint.
- [x] Emit `PairingFailed` with a stable reason code.
- [x] Unpaired connections are allowed only to the pairing endpoint; any other
  application protocol is rejected before handler dispatch.

## TLS and QUIC

- [x] Generate a self-signed certificate whose key is bound to the node
  identity; document the binding mechanism.
- [x] ALPN identifier `ticonnect/1`.
- [x] TLS 1.3 only; no older protocol versions.
- [x] Server requires client certificates; client requires server certificates.
- [x] Custom certificate verifier pins the peer certificate fingerprint to the
  trusted-peer table.
- [x] Unknown or mismatched fingerprint aborts the handshake with a clear error.
- [x] Disable 0-RTT until replay handling is designed.
- [x] Configure keep-alive and idle timeout.
- [x] QUIC datagram support enabled for later audio use.

## Transport

- [x] `Transport` actor owns the QUIC endpoint and active sessions.
- [x] Accept loop spawns one task per connection.
- [x] Client connects to a candidate address and retries with backoff.
- [x] Session state: `connecting`, `connected`, `disconnected`, `failed`.
- [x] Emit `PeerConnected` and `PeerDisconnected` D-Bus signals.
- [x] Reconnect automatically when a trusted peer is rediscovered.
- [x] One active session per peer; a new connection replaces the old one.
- [x] Graceful close sends a close frame and releases resources.
- [x] Track connection metrics: handshakes, failures, active sessions.

## Trust store

- [x] Trusted peer lookup by fingerprint and by node id.
- [x] Revoke/forget peer removes trust and closes active session.
- [x] Pairing is the only way to add trust in v1.
- [x] Store certificate fingerprint separately from identity fingerprint only if
  they differ; otherwise document the single-fingerprint model.

## D-Bus additions

- [x] `StartPairing() -> a{sv}` returns PIN, QR payload, and expiry.
- [x] `PairWithPin(s node_id, s pin) -> b`.
- [x] `PairWithQr(s payload) -> b`.
- [x] `CancelPairing()`.
- [x] `ForgetPeer(s peer_id)`.
- [x] `ListTrustedPeers() -> aa{sv}`.
- [x] Signals `PairingStarted`, `PairingCompleted`, `PairingFailed`,
  `PeerConnected`, `PeerDisconnected`.

## Tests

- [x] SPAKE2 unit test: both sides derive the same key with the same PIN.
- [x] SPAKE2 unit test: wrong PIN fails with a typed error.
- [x] QR payload encode/decode round-trip and expiry test.
- [x] TLS test: pinned certificate succeeds; unknown certificate fails.
- [x] TLS test: client without certificate is rejected.
- [x] Transport test: two in-process nodes connect over loopback.
- [x] Test: unpaired peer cannot open a non-pairing stream.
- [x] Test: reconnection after transport drop.
- [x] Test: `ForgetPeer` closes an active session.
- [x] Fuzz test: pairing message decoder never panics.

## Security

- [x] Threat model written for pairing and transport before implementation.
- [x] PIN attempts limited; failures do not leak which step failed.
- [x] Pairing token is single-use and expires.
- [x] Constant-time comparison for confirmation values.
- [x] No key material in logs or D-Bus responses.
- [x] Certificate pinning fails closed.
- [x] No fallback to unauthenticated TLS.
- [x] Replay protection for pairing messages via nonce and transcript hash.

## Docs and acceptance

- [x] Document pairing flows (PIN and QR), timeouts, and failure modes.
- [x] Document trust model and how to forget a peer.
- [x] Document ALPN, ports, and certificate model.
- [x] Document that discovery is not trust.

## Acceptance criteria

- Two nodes pair with a PIN and with a QR payload.
- Wrong PIN fails and allows retry within limits.
- Paired nodes establish a pinned QUIC connection.
- An unpaired node cannot reach any non-pairing functionality.
- Forgetting a peer blocks future connections until re-pairing.

## Demo

1. Start two daemons.
2. Run `ticonnect-cli pair --pin` on both and complete pairing.
3. Show `PeerConnected` signals on both sides.
4. Forget the peer and show connection rejection.
5. Repeat with QR payload.

## Risks and mitigations

- Risk: cryptographic mistakes. Mitigation: use vetted crates, keep transcripts
  simple, add test vectors, and review before merge.
- Risk: certificate binding complexity. Mitigation: document one model and test
  it end to end.
- Risk: port conflicts. Mitigation: configurable port and clear startup error.

## Works without later stages

Pairing and a live pinned QUIC session work with no messaging, files, calls, or
UI. Streams beyond pairing are rejected.
