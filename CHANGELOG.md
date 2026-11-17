# Changelog

## Unreleased

- Replace the GTK4/libadwaita/relm4 interface with a WebKitGTK 6.0 host and a
  Svelte frontend.
- Add the loopback HTTP + WebSocket bridge with token, origin, and host checks.
- Remove the old GTK views, UI stylesheet, and relm4/libadwaita/qrcodegen
  dependencies.

## 0.0.6 — 2026-11-14

- LAN discovery, SPAKE2 pairing, pinned QUIC transport, and signed messaging.
- Replica history, outbox expiry, gap sync, receipts, policy, tray, GTK4 UI.
- Corner overlay with X11 fallback and system notification fallback.
- BLAKE3 content-addressed attachments with chunked resumable transfers.
- Opus voice calls over QUIC datagrams with jitter buffer and daemon ringtone.
- Consensual remote desktop: portal capture, H.264, input injection, PIN,
  indicator, and kill switch.
- Flatpak manifest, hardened systemd units, benchmarks, and documentation.
