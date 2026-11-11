# Security model

This document covers identity, discovery, pairing, and transport. Messaging,
files, calls, and remote desktop add their own gates in later stages.

## Assets

- Ed25519 identity private key in `identity.key`.
- Trusted peer table in SQLite.
- Pairing PINs, QR tokens, and SPAKE2 secrets.
- Post-pairing QUIC session keys and application frames.

## Trust boundaries

- Discovery is untrusted input. A candidate is a hint only and never grants
  trust.
- The pairing endpoint is reachable before trust. It exposes no feature other
  than the pairing exchange.
- Post-pairing sessions are reachable only after the peer certificate
  fingerprint matches a trusted peer.
- The D-Bus session bus is the local control boundary. Any local process on the
  session bus can call daemon methods; this is the same boundary used by the UI
  and CLI.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| Spoofed mDNS or broadcast candidate | Candidates are validated and only used as addresses. Pairing still requires the PIN or QR token. |
| Man-in-the-middle during pairing | SPAKE2 binds both identities, both fingerprints, the certificate fingerprints, protocol version, and pairing mode in the transcript. Confirmation values are compared in constant time. |
| PIN guessing | PINs are six digits, expire after 120 seconds, and allow three failures per session. A new PIN is required after exhaustion. |
| QR replay | QR tokens are 32 random bytes, expire after 120 seconds, and are single-use. The listener clears the active session after success. |
| Certificate substitution | The client pins the expected BLAKE3 fingerprint before the handshake completes. The server requires the certificate fingerprint to be in the trust store before accepting session streams. |
| Unpaired access | Only the `ticonnect-pair/1` ALPN is reachable before trust. `ticonnect/1` closes the connection when the peer fingerprint is not trusted. |
| Key substitution | The self-signed certificate uses the Ed25519 identity key. TLS proves possession of that key, and the fingerprint is the BLAKE3 hash of the public key. |
| Replay of pairing messages | The transcript includes both identities, the token, and a protocol version. Pairing messages are length-prefixed and bounded. 0-RTT is disabled. |
| Oversized or malformed frames | Every decoder enforces fixed maximum sizes and returns typed errors. Fuzz targets cover wire and pairing decoders. |
| Resource exhaustion | Candidate caps, packet size limits, bounded pairing frames, connection timeouts, and one active session per peer limit memory and connection abuse. |
| Secret leakage | Private keys, PINs, tokens, and session keys are never logged or exposed over D-Bus. Errors are stable reason codes. |

## Certificate model

There is one fingerprint model in v1:

1. `identity.key` is a 32-byte Ed25519 seed.
2. The identity fingerprint is BLAKE3 of the Ed25519 public key.
3. The self-signed TLS certificate uses the same Ed25519 key.
4. The certificate fingerprint is therefore the identity fingerprint.
5. Trust rows store that single fingerprint.

Certificate validity and self-signature are checked. Pinning fails closed on
unknown, mismatched, or malformed certificates.

## Pairing endpoint

The pairing endpoint shares the QUIC listener and port with session transport but
uses the `ticonnect-pair/1` ALPN. ALPN routing happens before any application
handler is dispatched. Session ALPN connections check the trust table after the
TLS handshake and before accepting the handshake stream.

## Out of scope

- Compromised local accounts or root on either host.
- WAN discovery or relays.
- Key rotation, multi-device identities, and group pairing in v1.
