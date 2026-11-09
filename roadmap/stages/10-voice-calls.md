# Stage 10 — Voice Calls

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

- [ ] States: `idle`, `ringing`, `in-call`, `ended`.
- [ ] Events: `start`, `incoming`, `accept`, `decline`, `hangup`, `timeout`,
  `error`.
- [ ] Only one active call per peer and one active call per node.
- [ ] Incoming call has a configurable timeout (default 30 s).
- [ ] Reject calls from untrusted peers before any media setup.
- [ ] Reject calls from `ignore` peers; `mute` still rings per config.
- [ ] DND suppresses ringtone but still reports the call.
- [ ] Emit state changes as D-Bus signals.
- [ ] Clean up audio devices and ringtone on every terminal transition.

## Ringtone

- [ ] `rodio` plays a configurable file from `ringtone.path`.
- [ ] Loop until answered, declined, or timeout.
- [ ] Fall back to a bundled tone when the file is missing or invalid.
- [ ] Run in the daemon, not the UI.
- [ ] Duck volume once call audio is connected.
- [ ] Stop immediately on state change.
- [ ] Respect mute/DND.
- [ ] Handle audio device absence without crashing.

## Audio pipeline

- [ ] Enumerate input and output devices with `cpal`.
- [ ] Capture mono at a supported rate; resample to Opus input rate if needed.
- [ ] Encode Opus at 20 ms frames with a fixed bitrate target.
- [ ] Send frames over QUIC datagrams with sequence and timestamp.
- [ ] Receive, reorder, and decode frames.
- [ ] Jitter buffer with target latency and adaptive depth.
- [ ] Packet-loss concealment on gaps.
- [ ] Playback through `cpal` with a bounded ring buffer.
- [ ] Handle device changes or unplugging without panic.
- [ ] Metrics: frames sent/received, loss, jitter, buffer depth.

## D-Bus additions

- [ ] `StartCall(s peer_id) -> s call_id`.
- [ ] `AcceptCall(s call_id)`.
- [ ] `DeclineCall(s call_id)`.
- [ ] `HangupCall(s call_id)`.
- [ ] `GetCallState() -> a{sv}`.
- [ ] Signals `IncomingCall(a{sv})`, `CallStateChanged(s, s)`,
  `CallEnded(s, s)`.

## UI

- [ ] Call view with peer name, state, duration, mute, and hangup.
- [ ] Incoming call prompt with accept and decline.
- [ ] Device selection in settings.
- [ ] Show call errors clearly.
- [ ] No audio handling in the UI process.

## Tests

- [ ] State machine transition tests for every event and invalid transition.
- [ ] Ringtone starts, loops, stops, and ducks using a fake sink.
- [ ] Opus encode/decode round-trip at 20 ms.
- [ ] Jitter buffer reorders packets and conceals loss.
- [ ] Two-node loopback call with synthetic audio; verify frames flow.
- [ ] Call timeout, decline, and hangup paths.
- [ ] Untrusted peer call is rejected.
- [ ] Device unplug simulation does not panic.
- [ ] Manual: real call on X11 and Wayland, GNOME and KDE.

## Security

- [ ] Calls only between trusted, pinned peers.
- [ ] Media flows only over the authenticated QUIC session.
- [ ] No microphone access before call acceptance.
- [ ] No call metadata in logs beyond state and peer id.
- [ ] Bounded packet and buffer sizes to prevent abuse.
- [ ] Reject unexpected datagrams outside an active call.

## Docs and acceptance

- [ ] Document call states and transitions.
- [ ] Document ringtone configuration and fallback.
- [ ] Document audio devices, latency target, and known limitations.
- [ ] Document that the daemon rings with the UI closed.

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
