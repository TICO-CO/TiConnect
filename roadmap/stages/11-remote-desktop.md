# Stage 11 — Remote Desktop

> Automation complete in code. Manual portal, capture, input, and indicator checks on GNOME/KDE Wayland and X11 remain pending.

## Goal

Allow a trusted peer to view and control the host desktop. Capture and input go
through a single RemoteDesktop portal grant. Consent is explicit, an indicator is
always visible, and a kill switch ends the session immediately. Sessions are
PIN-gated and scoped to one trusted peer.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-11).
Component graphs: [DT](../../docs/systems/generated/components/dt.md), [VID](../../docs/systems/generated/components/vid.md), [RD](../../docs/systems/generated/components/rd.md).

## Scope

- `ashpd` RemoteDesktop portal session.
- PipeWire capture stream.
- `openh264` software encoding at 1080p.
- Viewer decode and GTK4 rendering.
- Input injection with `reis` (libei) on Wayland and `x11rb` XTEST on X11.
- Consent dialog, persistent indicator, and kill switch.
- Per-session PIN.

## Non-goals

- No 4K, no hardware encoding, no multi-monitor selection beyond portal options.
- No file transfer during a session beyond Stage 9.
- No unattended or silent access.
- No audio streaming from host.
- No Windows or macOS.

## Prerequisites

Stages 1, 1.5, 3, 5, and 7 complete.

## Deliverables

- `crates/desktop/src/portal.rs` — `ashpd` session setup.
- `crates/desktop/src/capture.rs` — PipeWire stream.
- `crates/desktop/src/input/{mod.rs,libei.rs,xtest.rs}`
- `crates/media/src/h264/{mod.rs,encoder.rs,decoder.rs}`
- `crates/daemon/src/remote/{mod.rs,session.rs,consent.rs}`
- `crates/ui/src/views/remote.rs`
- D-Bus additions in `crates/daemon/src/service.rs`.

## Consent and session flow

- [x] Viewer requests a session with the host peer.
- [x] Host daemon shows a consent dialog with viewer identity and fingerprint.
- [x] Host generates a per-session PIN shown locally and entered by the viewer.
- [x] Session is scoped to exactly one trusted peer.
- [x] Consent is required for every session; no remembered grant in v1.
- [x] Host shows a persistent "remote session active" indicator while active.
- [x] Kill switch ends capture, input, and transport immediately.
- [x] Session ends on transport loss, peer disconnect, or explicit hangup.
- [x] Deny path leaves no capture or input resources open.
- [x] Session timeout configurable; default off with explicit hangup.

## Capture

- [x] Open one RemoteDesktop portal session via `ashpd`.
- [x] Obtain the PipeWire capture stream and EIS file descriptor in one grant.
- [x] Negotiate 1080p target; respect portal-provided size.
- [x] Consume frames without unbounded buffering.
- [x] Drop frames under backpressure rather than queueing without limit.
- [x] Stop capture on kill switch and release the portal session.

## Encoding and transport

- [x] `openh264` software encoder at 1080p target.
- [x] Configure bitrate, keyframe interval, and low-latency mode.
- [x] Send encoded frames over QUIC streams or datagrams with sequencing.
- [x] Handle frame loss with keyframe requests.
- [x] Viewer decodes with `openh264`.
- [x] Render decoded frames in the GTK4 remote view.
- [x] Track FPS, bitrate, and dropped frames.

## Input injection

- [x] Use `reis` over the portal EIS fd on Wayland.
- [x] Use `x11rb` XTEST on X11 when libei is unavailable.
- [x] Send pointer motion, buttons, wheel, and keyboard events.
- [x] Bound input rate and coalesce motion events.
- [x] Validate event types and ranges before injection.
- [x] Reject input when no active session or when the viewer is not the granted
  peer.
- [x] Release input devices on session end.

## D-Bus additions

- [x] `RequestRemoteSession(s peer_id) -> a{sv}` with session id and PIN.
- [x] `AcceptRemoteSession(s session_id, s pin)`.
- [x] `DenyRemoteSession(s session_id)`.
- [x] `EndRemoteSession(s session_id)`.
- [x] `GetRemoteSession() -> a{sv}`.
- [x] Signals `RemoteSessionRequested(a{sv})`, `RemoteSessionStarted(s)`,
  `RemoteSessionEnded(s, s)`.

## UI

- [x] Remote view with live frame rendering and connection state.
- [x] Host consent dialog with viewer identity and fingerprint.
- [x] Persistent indicator window or header badge while active.
- [x] Kill switch accessible from UI and tray.
- [x] PIN entry for the viewer.
- [x] Clear errors for portal denial, unsupported desktop, and capture failure.

## Tests

- [x] Portal session mock: consent accept and deny paths.
- [x] Encoder/decoder round-trip for a synthetic frame sequence.
- [x] Frame loss triggers keyframe request.
- [x] Input event validation and rate limiting.
- [x] Session end releases capture, input, and transport resources.
- [x] Untrusted peer cannot request or join a session.
- [x] Wrong PIN fails with bounded attempts.
- [x] Kill switch stops the session within a bounded time.
- [ ] Manual: GNOME Wayland and KDE Wayland capture plus input.
- [ ] Manual: X11 capture plus XTEST input.

## Security

- [x] Per-session consent, no silent or unattended access.
- [x] Per-session PIN with attempt limits and expiry.
- [x] Session scoped to one pinned peer.
- [x] Persistent visible indicator, not dismissible while active.
- [x] Hard kill switch always reachable.
- [x] Input injection only while the session is active.
- [x] No capture before consent.
- [x] Threat model updated for remote desktop.
- [x] Logs contain no screen content or keystrokes.

## Docs and acceptance

- [x] Document consent flow, PIN, indicator, and kill switch.
- [x] Document portal requirements and desktop support matrix.
- [x] Document Wayland libei vs X11 XTEST behavior.
- [x] Document performance target and 4K exclusion.
- [x] Document how to revoke a peer and end sessions.

## Acceptance criteria

- A trusted peer views the host desktop after explicit consent.
- Input injection works on Wayland and X11.
- The indicator is visible for the whole session.
- The kill switch ends the session immediately.
- Wrong PIN and untrusted peers are rejected.
- Session cleanup leaves no capture or input resources open.

## Demo

1. Pair two nodes.
2. Request a remote session; show host consent and PIN.
3. Connect, show live view and control.
4. Trigger the kill switch; show immediate teardown.
5. Repeat on X11 and on Wayland.

## Risks and mitigations

- Risk: portal and libei variance across desktops. Mitigation: test GNOME and KDE
  first; feature-detect and report unsupported environments clearly.
- Risk: CPU cost of software H.264. Mitigation: 1080p cap, bitrate tuning, and
  frame dropping under load.
- Risk: security exposure. Mitigation: mandatory consent, PIN, indicator, kill
  switch, and security review before merge.
- Risk: capture pipeline backpressure. Mitigation: bounded queues and drop frames
  instead of growing memory.

## Works without later stages

Remote desktop works with no attachments, voice calls, or overlay dependency.
Only earlier pairing, transport, and UI pieces are required.
