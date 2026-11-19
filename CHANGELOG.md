# Changelog

## Unreleased

- Add group watch parties with host-authoritative sync, participant rosters,
  late invites, and per-guest leave.
- Make the watch party page a group composer: select people, paste a YouTube
  link, send the link, or start a party.
- Drive the YouTube player with real playback state and host-synced commands.
- Fix attachment uploads and downloads in the Vite lab with scoped bridge CORS.
- Preserve attachment filenames and browser MIME types across the bridge.
- Render image attachments inline and save other attachments without opening a
  new webview.
- Surface attachment transfer progress and failures.
- Allow blob audio playback in the webview CSP.
- Bundle GStreamer plugins for voice capture and media playback on Nix.
- Add voice-message recording and inline audio playback in the web UI.
- Add local history clearing for all conversations and per-user conversations.
- Report real local storage usage and identity creation time in settings.
- Make discovery controls editable and add system/light/dark appearance.

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
