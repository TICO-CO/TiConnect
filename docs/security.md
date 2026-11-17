# Security model

This document covers identity, discovery, automatic trust, transport, and text
messaging. Files, calls, and remote desktop add their own gates in later stages.

## Assets

- Ed25519 identity private key in `identity.key`.
- Trusted user table and user signing keys in SQLite.
- Message bodies, signatures, and receipt state.
- LAN trust tokens and SPAKE2 secrets.
- QUIC session keys and application frames.

## Trust boundaries

- Discovery is untrusted input. A candidate is a hint only and never grants
  trust.
- The trust endpoint is reachable before trust. It exposes no feature other
  than the automatic trust exchange.
- The profile endpoint (`ticonnect-profile/1`) is reachable before trust. It
  serves only the local display name and avatar, bounded to 160 KiB. Profile
  data is public LAN data and never grants trust.
- Sessions are reachable only after the user certificate fingerprint matches a
  trusted user.
- The D-Bus session bus is the local control boundary. Any local process on the
  session bus can call daemon methods; this is the same boundary used by the UI
  and CLI.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| Spoofed mDNS or broadcast candidate | Candidates are validated and only used as addresses. Trust still requires the LAN token exchange and certificate pinning. |
| Man-in-the-middle during trust | SPAKE2 binds both identities, both fingerprints, the certificate fingerprints, protocol version, and LAN token in the transcript. Confirmation values are compared in constant time. |
| Trust token replay | The LAN token is derived from both fingerprints and is bound into the SPAKE2 transcript. Trust frames are single-use per exchange. |
| Certificate substitution | The client pins the expected BLAKE3 fingerprint before the handshake completes. The server requires the certificate fingerprint to be in the trust store before accepting session streams. |
| Unknown user access | Only the `ticonnect-trust/1` and `ticonnect-profile/1` ALPNs are reachable before trust. `ticonnect/1` closes the connection when the user fingerprint is blocked. |
| Spoofed profile | The profile endpoint is self-asserted. Clients verify the signed profile against the certificate key and advertised avatar hash, but profile data never grants trust. Session trust remains authoritative. |
| Key substitution | The self-signed certificate uses the Ed25519 identity key. TLS proves possession of that key, and the fingerprint is the BLAKE3 hash of the public key. |
| Replay of trust messages | The transcript includes both identities, the token, and a protocol version. Trust messages are length-prefixed and bounded. 0-RTT is disabled. |
| Oversized or malformed frames | Every decoder enforces fixed maximum sizes and returns typed errors. Fuzz targets cover wire and trust decoders. |
| Resource exhaustion | Candidate caps, packet size limits, bounded trust frames, connection timeouts, and one active session per user limit memory and connection abuse. |
| Secret leakage | Private keys, trust tokens, and session keys are never logged or exposed over D-Bus. Message bodies are never logged. Errors are stable reason codes. |
| Forged or tampered message | Records are signed with the sender's Ed25519 identity key. The receiver verifies the signature against the public key bound to the pinned fingerprint before storage, acknowledgement, or notification. |
| Message replay or duplication | Message IDs are UUIDv7 and signed. Duplicate IDs and duplicate `(conversation, sender, seq)` rows are ignored. Sequence numbers are bounded and monotonic. |
| Forged receipt | Ack records are signed and may target only a locally stored outbound message. Acks for unknown or inbound targets are rejected. |
| Oversized message | Bodies are limited to 32 KiB and canonical records to 64 KiB. Decoders reject malformed or oversized records. |
| Ignore policy bypass | Ignore is enforced before verification, storage, receipt creation, or notification. |

## Certificate model

There is one fingerprint model in v1:

1. `identity.key` is a 32-byte Ed25519 seed.
2. The identity fingerprint is BLAKE3 of the Ed25519 public key.
3. The self-signed TLS certificate uses the same Ed25519 key.
4. The certificate fingerprint is therefore the identity fingerprint.
5. Trust rows store that single fingerprint and the user's Ed25519 public key.
6. The public key is extracted from the user certificate during trust or the
   first authenticated session and checked against the pinned fingerprint.

Certificate validity and self-signature are checked. Pinning fails closed on
unknown, mismatched, or malformed certificates.

## Automatic LAN trust

Every discovered user is automatically trusted using a deterministic LAN token
derived from both fingerprints. This removes the user identity-verification step:
any device on the same LAN can appear in the user list and send messages or
calls.

Controls:

- `mute` keeps the user connected but suppresses notifications.
- `ignore` drops inbound messages and calls silently.
- blocking removes trust, closes the session, and adds the fingerprint to a
  persistent blocklist so automatic trust does not re-add it.
- `ticonnect users unblock <fingerprint>` clears a block.

Remote desktop remains gated by explicit host consent.

## Trust endpoint

The trust endpoint shares the QUIC listener and port with session transport but
uses the `ticonnect-trust/1` ALPN. ALPN routing happens before any application
handler is dispatched. Session ALPN connections check the trust table after the
TLS handshake and before accepting the handshake stream.

## Out of scope

- Compromised local accounts or root on either host.
- WAN discovery or relays.
- Key rotation, multi-device identities, and group trust in v1.

## Attachments

- Content is addressed by BLAKE3 and verified before publication into the store.
- Files are written to private temporary paths and renamed only after the final
  hash matches.
- Names reject path separators and control characters; exports never follow
  symlinks and never overwrite an existing path.
- Size is bounded by `attachments.max_size_mib` (default 2048). MIME type and
  extension are validated; nothing is opened or executed automatically.
- User `ignore` policy blocks inbound transfer initiation. `mute` stores content
  silently.
- Transfer frames, chunk sizes, concurrent transfers per user, and total active
  transfers are bounded.

## Voice calls

- Calls are offered only to trusted, pinned users over the authenticated QUIC
  session.
- The microphone is opened only after the local call is accepted.
- Audio datagrams are accepted only for the active call and the matching user.
- Packet size, jitter depth, and playback buffers are bounded.
- Ignore policy drops calls before any media setup; DND suppresses the ringtone
  but still reports the incoming call.
- Logs contain call state and user id, not audio contents.

## Remote desktop

- Every session requires explicit host consent with a session PIN.
- A session is scoped to exactly one pinned user. Untrusted users cannot request
  or join a session.
- Capture and input use the desktop portal grant and begin only after consent.
- A visible indicator is shown for the whole active session. The kill switch
  tears down capture, input, and transport immediately.
- Input events are validated and rate limited before injection. No screen
  content, keystrokes, or PINs are logged.

## Threat model review status

Automatic trust, pinning, key storage, message policy, attachments, calls, and remote
consent are covered by unit and integration tests. Manual GNOME/KDE Wayland and
X11 verification of portals, input injection, and the indicator remains a
release gate.
