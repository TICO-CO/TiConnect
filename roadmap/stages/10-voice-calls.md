# Stage 10 — Voice Calls

> Automation complete. Manual real-device calls on X11/Wayland and GNOME/KDE remain pending.

## Goal

Add one-to-one voice calls over QUIC datagrams. The daemon owns the call state
machine and ringtone so calls ring with the UI closed. Audio uses `cpal` and
Opus at 20 ms frames with a jitter buffer and packet-loss concealment.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-10).
Component graphs: [AUD](../../docs/systems/generated/components/aud.md), [CALL](../../docs/systems/generated/components/call.md).

## Scope

- Call state machine in the daemon.
- Ringtone playback with `rodio`, looping until answered or declined.
- `cpal` capture and playback.
- Opus encode/decode, 20 ms frames.
- QUIC datagrams for audio.
- Jitter buffer and packet-loss concealment.
- Ringtone ducking under call audio.
- D-Bus call methods and signals.
- Call controls in the UI.

## Non-goals

- No group calls.
- No video.
- No call recording.
- No echo cancellation beyond what `cpal`/platform provides; document limits.
- No PSTN or relay.

## Prerequisites

Stages 1, 1.5, 3, 5, and 6 complete. Stage 7 is optional; CLI and D-Bus signals
are enough to demo calls.

## Deliverables

- `crates/media/src/audio/{mod.rs,capture.rs,playback.rs,device.rs}`
- `crates/media/src/opus_codec.rs`
- `crates/media/src/jitter.rs`
- `crates/media/src/ringtone.rs`
- `crates/daemon/src/calls/{mod.rs,state.rs,session.rs}`
- `crates/ui/src/views/call.rs`
- D-Bus additions in `crates/daemon/src/service.rs`.

## Call state machine

- [x] States: `idle`, `ringing`, `in-call`, `ended`.
- [x] Events: `start`, `incoming`, `accept`, `decline`, `hangup`, `timeout`,
  `error`.
- [x] Only one active call per peer and one active call per node.
- [x] Incoming call has a configurable timeout (default 30 s).
- [x] Reject calls from untrusted peers before any media setup.
- [x] Reject calls from `ignore` peers; `mute` still rings per config.
- [x] DND suppresses ringtone but still reports the call.
- [x] Emit state changes as D-Bus signals.
- [x] Clean up audio devices and ringtone on every terminal transition.

## Ringtone

- [x] `rodio` plays a configurable file from `ringtone.path`.
- [x] Loop until answered, declined, or timeout.
- [x] Fall back to a bundled tone when the file is missing or invalid.
- [x] Run in the daemon, not the UI.
- [x] Duck volume once call audio is connected.
- [x] Stop immediately on state change.
- [x] Respect mute/DND.
- [x] Handle audio device absence without crashing.

## Audio pipeline

- [x] Enumerate input and output devices with `cpal`.
- [x] Capture mono at a supported rate; resample to Opus input rate if needed.
- [x] Encode Opus at 20 ms frames with a fixed bitrate target.
- [x] Send frames over QUIC datagrams with sequence and timestamp.
- [x] Receive, reorder, and decode frames.
- [x] Jitter buffer with target latency and adaptive depth.
- [x] Packet-loss concealment on gaps.
- [x] Playback through `cpal` with a bounded ring buffer.
- [x] Handle device changes or unplugging without panic.
- [x] Metrics: frames sent/received, loss, jitter, buffer depth.

## D-Bus additions

- [x] `StartCall(s peer_id) -> s call_id`.
- [x] `AcceptCall(s call_id)`.
- [x] `DeclineCall(s call_id)`.
- [x] `HangupCall(s call_id)`.
- [x] `GetCallState() -> a{sv}`.
- [x] Signals `IncomingCall(a{sv})`, `CallStateChanged(s, s)`,
  `CallEnded(s, s)`.

## UI

- [x] Call view with peer name, state, duration, mute, and hangup.
- [x] Incoming call prompt with accept and decline.
- [x] Device selection in settings.
- [x] Show call errors clearly.
- [x] No audio handling in the UI process.

## Tests

- [x] State machine transition tests for every event and invalid transition.
- [x] Ringtone starts, loops, stops, and ducks using a fake sink.
- [x] Opus encode/decode round-trip at 20 ms.
- [x] Jitter buffer reorders packets and conceals loss.
- [x] Two-node loopback call with synthetic audio; verify frames flow.
- [x] Call timeout, decline, and hangup paths.
- [x] Untrusted peer call is rejected.
- [x] Device unplug simulation does not panic.
- [ ] Manual: real call on X11 and Wayland, GNOME and KDE.

## Security

- [x] Calls only between trusted, pinned peers.
- [x] Media flows only over the authenticated QUIC session.
- [x] No microphone access before call acceptance.
- [x] No call metadata in logs beyond state and peer id.
- [x] Bounded packet and buffer sizes to prevent abuse.
- [x] Reject unexpected datagrams outside an active call.

## Docs and acceptance

- [x] Document call states and transitions.
- [x] Document ringtone configuration and fallback.
- [x] Document audio devices, latency target, and known limitations.
- [x] Document that the daemon rings with the UI closed.

## Acceptance criteria

- Two paired nodes place and answer a voice call.
- Ringtone plays in the daemon with the UI closed.
- Audio flows both directions with acceptable latency on LAN.
- Hangup and decline clean up devices and state.
- DND and ignore behavior match policy.
- Call view controls work.

## Demo

1. Start daemons with UI closed on both nodes.
2. Start a call; show ringtone on the callee.
3. Answer; show two-way audio and call state signals.
4. Hang up; show cleanup.
5. Repeat with DND and ignore policy.

## Risks and mitigations

- Risk: device quirks and sample-rate mismatches. Mitigation: resampling layer
  and device abstraction with tests.
- Risk: latency and jitter on Wi-Fi. Mitigation: adaptive jitter buffer and
  metrics; document LAN expectations.
- Risk: echo. Mitigation: document platform limits; no claim of AEC in v1.

## Works without later stages

Voice calls work with no remote desktop, attachments, or overlay dependency.
The UI call view is optional; CLI and signals suffice.
