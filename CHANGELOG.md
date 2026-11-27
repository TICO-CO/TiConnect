# Changelog

## 0.1.1 — 2026-11-25

- Ignore stale inherited Cargo rustflags in Makefile and Nix development
  builds.
- Find the Nix D-Bus session configuration when starting the private bus for
  `ticonnect-dev`.
- Install a separate **TiConnect Dev** app-menu launcher and icon during
  `make install-dev`.

## 0.1.0 — 2026-11-24

- Fix message edit/delete in the web UI: the bridge now forwards the
  `MessageEdited` and `MessageDeleted` daemon signals it was subscribed to
  but never listened for.
- Add message replies with quoted originals and jump-to-original, for text,
  image, audio, and file messages (message protocol v2, dev bump).
- Limit message edits to five minutes after sending; deletes stay as
  tombstones.
- Make edit/delete work in the web UI demo with local state updates.

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
- Add daemon-owned microphone and output mute for calls, with configurable
  shortcuts and synchronized UI/overlay state.
- Add daemon audio-device discovery and input/output selectors in Calls settings.
- Persist call history in SQLite and load it into the web UI.
- Allow hanging up an outgoing ringing call and notify peers when media setup
  fails; configured audio devices now fail explicitly instead of falling back.
- Fix call audio on stereo and non-F32 devices by downmixing capture to mono,
  expanding playback across device channels, and honoring supported sample
  formats.
- Accept signed D-Bus timestamps when parsing live call state, fixing the web
  bridge call view and incoming-call events.
- Remove ringtone playback and the `ringtone.path` setting; incoming calls are
  shown without audio. Route lab call audio through null devices by default to
  avoid same-host speaker/microphone feedback.
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
