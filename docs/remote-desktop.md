# Remote desktop

## Consent flow

1. The viewer calls `RequestRemoteSession(user_id)`. The host creates a session
   in `requested` and emits `RemoteSessionRequested` with the viewer identity,
   including the verified BLAKE3 fingerprint.
2. The host reviews the request and calls `AcceptRemoteSession(session_id)`.
   This is the full consent grant; no PIN is generated, shown, or entered. The
   consent card shows the verified fingerprint so the host can compare it with
   the expected contact.
3. The host opens the portal grant, starts capture and input, sends a start
   frame, and both sides transition to `active`. A visible indicator is emitted.
4. `EndRemoteSession(session_id)` is the kill switch. It stops capture, releases
   input, sends a kill frame, and tears down the route immediately.

Sessions are scoped to exactly one trusted user. There is no remembered grant
in v1: every session repeats host consent.

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

Capture starts only after verifiable host consent and a portal grant. If the
portal denies the request, capture does not start. Input injection uses libei
through the portal EIS fd on Wayland; X11 uses XTEST. If libei is unavailable
on Wayland, input injection is disabled and the UI reports the error.

## Performance targets

- 1080p at the configured `video.fps` and `video.bitrate_kbps`.
- Dropped frames are preferred over unbounded queues.
- Keyframe requests recover from frame loss.
- No screen content or keystrokes appear in logs.
