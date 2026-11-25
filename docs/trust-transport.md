# Automatic trust and transport

TiConnect trusts users automatically on the LAN. There is no manual PIN or QR
pairing flow. Transport carries all features over one QUIC endpoint.

## Ports and protocols

| Purpose | Protocol | Default |
|---|---|---|
| mDNS | UDP multicast `5353` | enabled |
| Broadcast discovery | UDP `45870` | enabled |
| QUIC session and trust | UDP | `transport.port`, default `45871` |

`transport.port` is advertised in mDNS TXT records and UDP beacons. Discovery
candidates carry this port.

ALPNs:

- `ticonnect/1` — authenticated sessions.
- `ticonnect-trust/1` — automatic first-time trust.
- `ticonnect-profile/1` — bounded public profile fetch.

TLS is 1.3 only. 0-RTT is disabled. QUIC datagrams are used for audio and input
frames.

## Certificate model

The self-signed certificate reuses the Ed25519 identity key. The identity
fingerprint and certificate fingerprint are the same value: BLAKE3 over the
Ed25519 public key. Trust rows store that single fingerprint.

The client pins the expected candidate fingerprint. The server requires a valid
self-signed Ed25519 identity certificate and then checks that the fingerprint is
trusted or still unknown and unblocked before accepting `ticonnect/1` streams.

## Automatic trust flow

1. Discovery yields a candidate with node id, fingerprint, and address.
2. Both sides derive a LAN token from both fingerprints.
3. Both sides complete a SPAKE2 exchange over the trust ALPN.
4. Confirmation values are compared in constant time.
5. On success the user is inserted into the user table with its fingerprint,
   node id, and `trusted_at`.

Limits:

- Trust exchange timeout: 120 seconds.
- Trust frames are bounded.
- Blocked fingerprints are rejected before the exchange.

## Transcript

The SPAKE2 transcript binds protocol version, both node ids, both identity
fingerprints, both certificate fingerprints, and the LAN token. Confirmation
values are derived with BLAKE3 over the shared secret and transcript hash.

## Trust store

The daemon loads trusted fingerprints into memory at startup and writes user rows
to SQLite on trust. `ForgetUser` deletes the user row, local conversation
history, and queued outbox rows, removes the in-memory trust entry, and closes
any active session for that user. Sequence high-water marks are retained so a
returning user cannot reuse old sequence numbers.

Reconnects happen only for trusted fingerprints. A trusted user is redialed when
discovered or when its session drops, subject to bounded backoff.

## Firewall

Allow UDP `5353` for mDNS, UDP `45870` for broadcast discovery, and the configured
`transport.port` for QUIC.

## Discovery is not identity verification

Discovery data is untrusted. Candidates are hints only. The LAN token exchange
and TLS certificate pinning authenticate the session. Any device on the same LAN
can appear as a user unless it is blocked.
