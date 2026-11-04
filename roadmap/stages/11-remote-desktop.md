# Stage 11 — Remote Desktop

## Goal

Allow a trusted peer to view and control the host desktop. Capture and input go
through a single RemoteDesktop portal grant. Consent is explicit, an indicator is
always visible, and a kill switch ends the session immediately. Sessions are
PIN-gated and scoped to one trusted peer.

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

Stages 1, 3, 5, and 7 complete.

## Deliverables

- `crates/desktop/src/portal.rs` — `ashpd` session setup.
- `crates/desktop/src/capture.rs` — PipeWire stream.
- `crates/desktop/src/input/{mod.rs,libei.rs,xtest.rs}`
- `crates/media/src/h264/{mod.rs,encoder.rs,decoder.rs}`
- `crates/daemon/src/remote/{mod.rs,session.rs,consent.rs}`
- `crates/ui/src/views/remote.rs`
- D-Bus additions in `crates/daemon/src/service.rs`.

## Consent and session flow

- [ ] Viewer requests a session with the host peer.
- [ ] Host daemon shows a consent dialog with viewer identity and fingerprint.
- [ ] Host generates a per-session PIN shown locally and entered by the viewer.
- [ ] Session is scoped to exactly one trusted peer.
- [ ] Consent is required for every session; no remembered grant in v1.
- [ ] Host shows a persistent "remote session active" indicator while active.
- [ ] Kill switch ends capture, input, and transport immediately.
- [ ] Session ends on transport loss, peer disconnect, or explicit hangup.
- [ ] Deny path leaves no capture or input resources open.
- [ ] Session timeout configurable; default off with explicit hangup.

## Capture

- [ ] Open one RemoteDesktop portal session via `ashpd`.
- [ ] Obtain the PipeWire capture stream and EIS file descriptor in one grant.
- [ ] Negotiate 1080p target; respect portal-provided size.
- [ ] Consume frames without unbounded buffering.
- [ ] Drop frames under backpressure rather than queueing without limit.
- [ ] Stop capture on kill switch and release the portal session.

## Encoding and transport

- [ ] `openh264` software encoder at 1080p target.
- [ ] Configure bitrate, keyframe interval, and low-latency mode.
- [ ] Send encoded frames over QUIC streams or datagrams with sequencing.
- [ ] Handle frame loss with keyframe requests.
- [ ] Viewer decodes with `openh264`.
- [ ] Render decoded frames in the GTK4 remote view.
- [ ] Track FPS, bitrate, and dropped frames.

## Input injection

- [ ] Use `reis` over the portal EIS fd on Wayland.
- [ ] Use `x11rb` XTEST on X11 when libei is unavailable.
- [ ] Send pointer motion, buttons, wheel, and keyboard events.
- [ ] Bound input rate and coalesce motion events.
- [ ] Validate event types and ranges before injection.
- [ ] Reject input when no active session or when the viewer is not the granted
  peer.
- [ ] Release input devices on session end.

## D-Bus additions

- [ ] `RequestRemoteSession(s peer_id) -> a{sv}` with session id and PIN.
- [ ] `AcceptRemoteSession(s session_id, s pin)`.
- [ ] `DenyRemoteSession(s session_id)`.
- [ ] `EndRemoteSession(s session_id)`.
- [ ] `GetRemoteSession() -> a{sv}`.
- [ ] Signals `RemoteSessionRequested(a{sv})`, `RemoteSessionStarted(s)`,
  `RemoteSessionEnded(s, s)`.

## UI

- [ ] Remote view with live frame rendering and connection state.
- [ ] Host consent dialog with viewer identity and fingerprint.
- [ ] Persistent indicator window or header badge while active.
- [ ] Kill switch accessible from UI and tray.
- [ ] PIN entry for the viewer.
- [ ] Clear errors for portal denial, unsupported desktop, and capture failure.

## Tests

- [ ] Portal session mock: consent accept and deny paths.
- [ ] Encoder/decoder round-trip for a synthetic frame sequence.
- [ ] Frame loss triggers keyframe request.
- [ ] Input event validation and rate limiting.
- [ ] Session end releases capture, input, and transport resources.
- [ ] Untrusted peer cannot request or join a session.
- [ ] Wrong PIN fails with bounded attempts.
- [ ] Kill switch stops the session within a bounded time.
- [ ] Manual: GNOME Wayland and KDE Wayland capture plus input.
- [ ] Manual: X11 capture plus XTEST input.

## Security

- [ ] Per-session consent, no silent or unattended access.
- [ ] Per-session PIN with attempt limits and expiry.
- [ ] Session scoped to one pinned peer.
- [ ] Persistent visible indicator, not dismissible while active.
- [ ] Hard kill switch always reachable.
- [ ] Input injection only while the session is active.
- [ ] No capture before consent.
- [ ] Threat model updated for remote desktop.
- [ ] Logs contain no screen content or keystrokes.

## Docs and acceptance

- [ ] Document consent flow, PIN, indicator, and kill switch.
- [ ] Document portal requirements and desktop support matrix.
- [ ] Document Wayland libei vs X11 XTEST behavior.
- [ ] Document performance target and 4K exclusion.
- [ ] Document how to revoke a peer and end sessions.

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
