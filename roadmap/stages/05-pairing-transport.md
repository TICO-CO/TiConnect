# Stage 5 — Pairing & Transport

## Goal

Establish authenticated, encrypted QUIC connections between paired nodes. Pairing
uses SPAKE2 with a 6-digit PIN or a QR code, then pins the peer certificate to
its fingerprint. Unpaired peers can reach only the pairing endpoint.

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

Stages 1, 3, and 4 complete.

## Deliverables

- `crates/net/src/pairing/{mod.rs,spake2.rs,qr.rs}`
- `crates/net/src/tls.rs`
- `crates/net/src/transport/{mod.rs,server.rs,client.rs,session.rs}`
- `crates/net/src/trust.rs`
- `crates/daemon/src/pairing_service.rs`
- D-Bus additions in `crates/daemon/src/service.rs`.

## Pairing protocol

- [ ] Pairing endpoint listens on a fixed or advertised port, separate from the
  post-pairing session handler.
- [ ] SPAKE2 uses `spake2` with a 6-digit PIN encoded as the shared secret.
- [ ] PIN is generated with a CSPRNG, displayed on the initiating side, and
  entered on the responding side.
- [ ] QR payload contains node id, address, port, fingerprint, and a one-time
  pairing token with short TTL.
- [ ] PIN flow has a bounded number of attempts and a timeout.
- [ ] Pairing messages are length-prefixed and validated before processing.
- [ ] On success, both sides derive and compare a confirmation value.
- [ ] On success, insert the peer into the trusted-peer table with fingerprint,
  display name, and `trusted_at`.
- [ ] Emit `PairingCompleted` with peer id and fingerprint.
- [ ] Emit `PairingFailed` with a stable reason code.
- [ ] Unpaired connections are allowed only to the pairing endpoint; any other
  application protocol is rejected before handler dispatch.

## TLS and QUIC

- [ ] Generate a self-signed certificate whose key is bound to the node
  identity; document the binding mechanism.
- [ ] ALPN identifier `ticonnect/1`.
- [ ] TLS 1.3 only; no older protocol versions.
- [ ] Server requires client certificates; client requires server certificates.
- [ ] Custom certificate verifier pins the peer certificate fingerprint to the
  trusted-peer table.
- [ ] Unknown or mismatched fingerprint aborts the handshake with a clear error.
- [ ] Disable 0-RTT until replay handling is designed.
- [ ] Configure keep-alive and idle timeout.
- [ ] QUIC datagram support enabled for later audio use.

## Transport

- [ ] `Transport` actor owns the QUIC endpoint and active sessions.
- [ ] Accept loop spawns one task per connection.
- [ ] Client connects to a candidate address and retries with backoff.
- [ ] Session state: `connecting`, `connected`, `disconnected`, `failed`.
- [ ] Emit `PeerConnected` and `PeerDisconnected` D-Bus signals.
- [ ] Reconnect automatically when a trusted peer is rediscovered.
- [ ] One active session per peer; a new connection replaces the old one.
- [ ] Graceful close sends a close frame and releases resources.
- [ ] Track connection metrics: handshakes, failures, active sessions.

## Trust store

- [ ] Trusted peer lookup by fingerprint and by node id.
- [ ] Revoke/forget peer removes trust and closes active session.
- [ ] Pairing is the only way to add trust in v1.
- [ ] Store certificate fingerprint separately from identity fingerprint only if
  they differ; otherwise document the single-fingerprint model.

## D-Bus additions

- [ ] `StartPairing() -> a{sv}` returns PIN, QR payload, and expiry.
- [ ] `PairWithPin(s node_id, s pin) -> b`.
- [ ] `PairWithQr(s payload) -> b`.
- [ ] `CancelPairing()`.
- [ ] `ForgetPeer(s peer_id)`.
- [ ] `ListTrustedPeers() -> aa{sv}`.
- [ ] Signals `PairingStarted`, `PairingCompleted`, `PairingFailed`,
  `PeerConnected`, `PeerDisconnected`.

## Tests

- [ ] SPAKE2 unit test: both sides derive the same key with the same PIN.
- [ ] SPAKE2 unit test: wrong PIN fails with a typed error.
- [ ] QR payload encode/decode round-trip and expiry test.
- [ ] TLS test: pinned certificate succeeds; unknown certificate fails.
- [ ] TLS test: client without certificate is rejected.
- [ ] Transport test: two in-process nodes connect over loopback.
- [ ] Test: unpaired peer cannot open a non-pairing stream.
- [ ] Test: reconnection after transport drop.
- [ ] Test: `ForgetPeer` closes an active session.
- [ ] Fuzz test: pairing message decoder never panics.

## Security

- [ ] Threat model written for pairing and transport before implementation.
- [ ] PIN attempts limited; failures do not leak which step failed.
- [ ] Pairing token is single-use and expires.
- [ ] Constant-time comparison for confirmation values.
- [ ] No key material in logs or D-Bus responses.
- [ ] Certificate pinning fails closed.
- [ ] No fallback to unauthenticated TLS.
- [ ] Replay protection for pairing messages via nonce and transcript hash.

## Docs and acceptance

- [ ] Document pairing flows (PIN and QR), timeouts, and failure modes.
- [ ] Document trust model and how to forget a peer.
- [ ] Document ALPN, ports, and certificate model.
- [ ] Document that discovery is not trust.

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
