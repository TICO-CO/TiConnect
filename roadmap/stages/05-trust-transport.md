# Stage 5 — Automatic trust & Transport

## Goal

Establish authenticated, encrypted QUIC connections between paired nodes. Automatic trust
uses SPAKE2 with a 6-digit PIN or a QR code, then pins the user certificate to
its fingerprint. Unpaired users can reach only the trust endpoint.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-5).
Component graphs: [TRUST](../../docs/systems/generated/components/pair.md), [TX](../../docs/systems/generated/components/tx.md), [WIRE](../../docs/systems/generated/components/wire.md).

## Scope

- SPAKE2 trust over a dedicated trust path.
- PIN display/entry flow and QR payload.
- QUIC server and client with `quinn` over `rustls`.
- Self-signed certificates bound to the Ed25519 identity.
- Certificate pinning against the trusted-user table.
- Connection/session management with reconnection.
- D-Bus trust methods and signals.

## Non-goals

- No messages, files, or media yet.
- No UI; CLI handles PIN entry.
- No group trust, no multi-device identity, no key rotation.

## Prerequisites

Stages 1, 1.5, 3, and 4 complete.

## Deliverables

- `crates/net/src/trust/{mod.rs,spake2.rs,qr.rs}`
- `crates/net/src/tls.rs`
- `crates/net/src/transport/{mod.rs,server.rs,client.rs,session.rs}`
- `crates/net/src/trust.rs`
- `crates/daemon/src/trust_service.rs`
- D-Bus additions in `crates/daemon/src/service.rs`.

## Automatic trust protocol

- [x] Automatic trust endpoint listens on a fixed or advertised port, separate from the
  post-trust session handler.
- [x] SPAKE2 uses `spake2` with a 6-digit PIN encoded as the shared secret.
- [x] PIN is generated with a CSPRNG, displayed on the initiating side, and
  entered on the responding side.
- [x] QR payload contains node id, address, port, fingerprint, and a one-time
  trust token with short TTL.
- [x] PIN flow has a bounded number of attempts and a timeout.
- [x] Automatic trust messages are length-prefixed and validated before processing.
- [x] On success, both sides derive and compare a confirmation value.
- [x] On success, insert the user into the trusted-user table with fingerprint,
  display name, and `trusted_at`.
- [x] Emit `Automatic trustCompleted` with user id and fingerprint.
- [x] Emit `Automatic trustFailed` with a stable reason code.
- [x] Unpaired connections are allowed only to the trust endpoint; any other
  application protocol is rejected before handler dispatch.

## TLS and QUIC

- [x] Generate a self-signed certificate whose key is bound to the node
  identity; document the binding mechanism.
- [x] ALPN identifier `ticonnect/1`.
- [x] TLS 1.3 only; no older protocol versions.
- [x] Server requires client certificates; client requires server certificates.
- [x] Custom certificate verifier pins the user certificate fingerprint to the
  trusted-user table.
- [x] Unknown or mismatched fingerprint aborts the handshake with a clear error.
- [x] Disable 0-RTT until replay handling is designed.
- [x] Configure keep-alive and idle timeout.
- [x] QUIC datagram support enabled for later audio use.

## Transport

- [x] `Transport` actor owns the QUIC endpoint and active sessions.
- [x] Accept loop spawns one task per connection.
- [x] Client connects to a candidate address and retries with backoff.
- [x] Session state: `connecting`, `connected`, `disconnected`, `failed`.
- [x] Emit `UserConnected` and `UserDisconnected` D-Bus signals.
- [x] Reconnect automatically when a trusted user is rediscovered.
- [x] One active session per user; a new connection replaces the old one.
- [x] Graceful close sends a close frame and releases resources.
- [x] Track connection metrics: handshakes, failures, active sessions.

## Trust store

- [x] Trusted user lookup by fingerprint and by node id.
- [x] Revoke/forget user removes trust and closes active session.
- [x] Automatic trust is the only way to add trust in v1.
- [x] Store certificate fingerprint separately from identity fingerprint only if
  they differ; otherwise document the single-fingerprint model.

## D-Bus additions

- [x] `StartAutomatic trust() -> a{sv}` returns PIN, QR payload, and expiry.
- [x] `PairWithPin(s node_id, s pin) -> b`.
- [x] `PairWithQr(s payload) -> b`.
- [x] `CancelAutomatic trust()`.
- [x] `ForgetUser(s user_id)`.
- [x] `ListTrustedUsers() -> aa{sv}`.
- [x] Signals `Automatic trustStarted`, `Automatic trustCompleted`, `Automatic trustFailed`,
  `UserConnected`, `UserDisconnected`.

## Tests

- [x] SPAKE2 unit test: both sides derive the same key with the same PIN.
- [x] SPAKE2 unit test: wrong PIN fails with a typed error.
- [x] QR payload encode/decode round-trip and expiry test.
- [x] TLS test: pinned certificate succeeds; unknown certificate fails.
- [x] TLS test: client without certificate is rejected.
- [x] Transport test: two in-process nodes connect over loopback.
- [x] Test: unpaired user cannot open a non-trust stream.
- [x] Test: reconnection after transport drop.
- [x] Test: `ForgetUser` closes an active session.
- [x] Fuzz test: trust message decoder never panics.

## Security

- [x] Threat model written for trust and transport before implementation.
- [x] PIN attempts limited; failures do not leak which step failed.
- [x] Automatic trust token is single-use and expires.
- [x] Constant-time comparison for confirmation values.
- [x] No key material in logs or D-Bus responses.
- [x] Certificate pinning fails closed.
- [x] No fallback to unauthenticated TLS.
- [x] Replay protection for trust messages via nonce and transcript hash.

## Docs and acceptance

- [x] Document trust flows (PIN and QR), timeouts, and failure modes.
- [x] Document trust model and how to forget a user.
- [x] Document ALPN, ports, and certificate model.
- [x] Document that discovery is not trust.

## Acceptance criteria

- Two nodes trust each other automatically after discovery.
- Wrong PIN fails and allows retry within limits.
- Paired nodes establish a pinned QUIC connection.
- An unpaired node cannot reach any non-trust functionality.
- Forgetting a user blocks future connections until re-trust.

## Demo

1. Start two daemons.
2. Wait for discovery and automatic trust to complete.
3. Show `UserConnected` signals on both sides.
4. Forget the user and show connection rejection.
5. Repeat with QR payload.

## Risks and mitigations

- Risk: cryptographic mistakes. Mitigation: use vetted crates, keep transcripts
  simple, add test vectors, and review before merge.
- Risk: certificate binding complexity. Mitigation: document one model and test
  it end to end.
- Risk: port conflicts. Mitigation: configurable port and clear startup error.

## Works without later stages

Automatic trust and a live pinned QUIC session work with no messaging, files, calls, or
UI. Streams beyond trust are rejected.
