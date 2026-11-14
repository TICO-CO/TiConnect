# Remote desktop

## Consent flow

1. The viewer calls `RequestRemoteSession(peer_id)`. The host creates a session
   in `requested` and emits `RemoteSessionRequested` with the viewer fingerprint
   and a fresh six-digit PIN.
2. The host shows the consent dialog and calls `AcceptRemoteSession(session_id, "")`
   to grant consent. No capture or input is opened before this step.
3. The viewer enters the PIN and calls `AcceptRemoteSession(session_id, pin)`.
   The host verifies it in constant time, bounded by `remote.pin_attempts`.
4. On success the host opens the portal grant, starts capture and input, and both
   sides transition to `active`. A visible indicator is emitted.
5. `EndRemoteSession(session_id)` is the kill switch. It stops capture, releases
   input, sends a kill frame, and tears down the route immediately.

Sessions are scoped to exactly one pinned peer. There is no remembered grant in
v1: every session repeats consent and PIN entry.

## Desktop support

| Desktop | Capture | Input |
|---|---|---|
| GNOME Wayland | XDG RemoteDesktop portal + PipeWire | libei through the portal EIS fd |
| KDE Wayland | XDG RemoteDesktop portal + PipeWire | libei through the portal EIS fd |
| X11 | XDG RemoteDesktop portal + PipeWire | XTEST through `x11rb` |

The portal controls the actual capture source. TiConnect requests a monitor
stream at up to 1080p; the portal may return a different size, which is used
as-is. 4K is out of scope. Software H.264 targets 1080p with frame dropping under
backpressure.

If the portal denies the request, the session ends without opening capture or
input. If libei is unavailable on Wayland, input injection is disabled and the
UI reports the error; X11 falls back to XTEST.

## Performance targets

- 1080p at the configured `video.fps` and `video.bitrate_kbps`.
- Dropped frames are preferred over unbounded queues.
- Keyframe requests recover from frame loss.
- No screen content or keystrokes appear in logs.
