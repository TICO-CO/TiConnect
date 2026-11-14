# Pairing and transport

Pairing authenticates a first-time peer. Transport then carries all later
features over one QUIC endpoint.

## Ports and protocols

| Purpose | Protocol | Default |
|---|---|---|
| mDNS | UDP multicast `5353` | enabled |
| Broadcast discovery | UDP `45870` | enabled |
| QUIC session and pairing | UDP | `transport.port`, default `45871` |

`transport.port` is advertised in mDNS TXT records and UDP beacons. Discovery
candidates carry this port.

ALPNs:

- `ticonnect/1` — post-pairing sessions.
- `ticonnect-pair/1` — first-time pairing.

TLS is 1.3 only. 0-RTT is disabled. QUIC datagrams are enabled for later audio
and input stages.

## Certificate model

The self-signed certificate reuses the Ed25519 identity key. The identity
fingerprint and certificate fingerprint are the same value: BLAKE3 over the
Ed25519 public key. Trust rows store that single fingerprint.

The client pins the expected candidate or QR fingerprint. The server requires a
valid self-signed Ed25519 identity certificate and then checks that the
fingerprint is trusted before accepting `ticonnect/1` streams.

## PIN flow

1. The displaying node calls `StartPairing` and receives a six-digit PIN, a QR
   payload, and an expiry time.
2. The user runs `ticonnect cli pair with-pin <node-id> <pin>` on the other node.
3. Both sides complete a SPAKE2 exchange over the pairing ALPN.
4. Both sides compute a confirmation value over the transcript and compare it in
   constant time.
5. On success the peer is inserted into the trust table with its fingerprint,
   node id, and `trusted_at`.

Limits:

- PIN lifetime: 120 seconds.
- Failures per pairing session: 3.
- Pairing exchange timeout: 120 seconds.

## QR flow

The QR payload format is:

```text
ticonnect://pair/v1?node_id=<uuid>&fp=<64 hex>&addr=<ip>&port=<port>&token=<64 hex>&exp=<unix seconds>
```

The token is 32 random bytes and is the SPAKE2 secret. It expires after 120
seconds and is single-use.

```sh
ticonnect cli pair start
ticonnect-cli pair with-qr '<payload>'
```

## Transcript

The SPAKE2 transcript binds:

- protocol version
- pairing mode
- both node ids
- both identity fingerprints
- both certificate fingerprints
- the QR token when present

Confirmation values are derived with BLAKE3 over the shared secret and the
transcript hash.

## Trust store

The daemon loads trusted fingerprints into memory at startup and writes peer
rows to SQLite on pairing. `ForgetPeer` deletes the peer row, local conversation
history, and queued outbox rows, removes the in-memory trust entry, and closes
any active session for that peer. Sequence high-water marks are retained so a
re-paired peer cannot reuse old sequence numbers.

Reconnects happen only for trusted fingerprints. A trusted peer is redialed when
it is discovered or when its session drops, subject to bounded backoff.

## Firewall

Allow UDP `5353` for mDNS, UDP `45870` for broadcast discovery, and the configured
`transport.port` for QUIC. Discovery is optional; pairing with a QR payload does
not require discovery because the payload carries the address and port.

## Discovery is not trust

Discovery data is untrusted. Candidates are hints only. Nothing is reachable
before pairing and certificate pinning succeeds.
