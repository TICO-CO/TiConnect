# Stage 8 — Overlay & Notifications

## Goal

Add the always-running corner overlay agent that shows toast cards for incoming
messages, incoming calls, and completed transfers. It must not depend on the UI
being open. When the overlay is disabled or unavailable, events fall back to
system notifications.

## Scope

- `ticonnect-ovl` GTK4 layer-shell agent on Wayland.
- X11 fallback: undecorated always-on-top window.
- Toast cards for `MessageReceived`, `IncomingCall`, `TransferDone`.
- Click-to-activate via `org.ticonnect.UI.Activate(view)`.
- Auto-dismiss and persistent incoming-call behavior.
- `notify-rust` fallback.
- Overlay enable/disable setting.

## Non-goals

- No message composition or history in the overlay.
- No call answering beyond activating the UI.
- No tray (already in Stage 6).
- No remote desktop UI.

## Prerequisites

Stages 1, 2, and 6 complete. Stage 7 is optional for click activation; without
the UI, clicks fall back to notifications or are ignored.

## Deliverables

- `crates/overlay/src/main.rs`
- `crates/overlay/src/card.rs`
- `crates/overlay/src/layer_shell.rs`
- `crates/overlay/src/x11_fallback.rs`
- `crates/overlay/src/notifications.rs`
- `crates/overlay/src/client.rs` — daemon signal subscription.
- Packaging autostart entry for the overlay.

## Wayland overlay

- [ ] Use `gtk4-layer-shell` with `Layer::Overlay`.
- [ ] Anchor bottom-right.
- [ ] Transparent window with rounded card and shadow.
- [ ] `KeyboardMode::None` so the overlay never steals focus.
- [ ] Only the card surface takes pointer input.
- [ ] Detect layer-shell availability at runtime; fall back cleanly.
- [ ] Respect scale factor and multi-monitor placement.
- [ ] Card slide-in animation with reduced-motion support.

## X11 fallback

- [ ] Undecorated GTK4 window with `keep_above` and `skip_taskbar_hint`.
- [ ] Position bottom-right of the primary monitor.
- [ ] No focus stealing.
- [ ] Same card widget and behavior as Wayland.
- [ ] Hide when no cards are visible.

## Card behavior

- [ ] Message card: sender name, preview text, timestamp.
- [ ] Incoming call card: caller name, accept and decline actions, persists until
  answered or declined.
- [ ] Transfer done card: file name and size.
- [ ] Non-call cards auto-dismiss after a configurable timeout (default 5 s).
- [ ] Hover pauses dismissal.
- [ ] Clicking a card calls `org.ticonnect.UI.Activate(view)`.
- [ ] If the UI bus name is absent, clicking opens the app if configured or does
  nothing and logs.
- [ ] Queue cards instead of overlapping; cap visible cards.
- [ ] Deduplicate repeated events for the same message id.
- [ ] Respect policy: mute and ignore suppress cards.
- [ ] Respect DND: suppress all non-call cards; calls follow config.

## Notifications fallback

- [ ] Use `notify-rust` when the overlay is disabled or fails to initialize.
- [ ] Notification actions map to activate or decline where supported.
- [ ] Do not send both overlay and system notification for the same event.
- [ ] Handle notification server absence without crashing.

## D-Bus and config

- [ ] Overlay subscribes directly to `org.ticonnect.Daemon` signals.
- [ ] Overlay does not depend on the UI process.
- [ ] Config key `overlay.enabled` read from daemon settings at start and on
  change.
- [ ] Autostart entry starts the overlay after login.
- [ ] Single-instance guard for the overlay.

## Tests

- [ ] Manual Wayland: card appears bottom-right, does not steal focus, dismisses.
- [ ] Manual X11: fallback window behaves the same.
- [ ] Manual: incoming call card persists until action.
- [ ] Manual: click activates UI on the correct view.
- [ ] Manual: UI closed; overlay still shows cards and does not crash.
- [ ] Manual: disable overlay; system notifications appear instead.
- [ ] Manual: DND suppresses cards.
- [ ] Manual: multi-monitor and HiDPI placement.
- [ ] Automated: card queue and dedup logic unit tests.

## Security

- [ ] Overlay has no network, storage, or key access.
- [ ] Only subscribes to the documented daemon signals.
- [ ] Message previews respect policy and DND.
- [ ] No focus stealing; keyboard input never routed to the overlay.
- [ ] No message bodies written to logs.

## Docs and acceptance

- [ ] Document overlay enable/disable and fallback behavior.
- [ ] Document focus and input guarantees.
- [ ] Document X11 vs Wayland differences.
- [ ] Document the idle memory cost and why it is accepted.

## Acceptance criteria

- Wayland overlay shows corner cards without stealing focus.
- X11 fallback shows equivalent cards.
- Overlay works with the UI closed.
- Clicking a card raises the UI on the relevant view.
- Incoming call cards persist until answered or declined.
- Disabling the overlay yields system notifications.
- DND and per-peer policy are honored.

## Demo

1. Start daemon and overlay with UI closed.
2. Send a message from another node; show the card.
3. Trigger an incoming call; show persistence and actions.
4. Click a card; show UI activation.
5. Disable overlay; repeat with system notifications.

## Risks and mitigations

- Risk: layer-shell not available on some compositors. Mitigation: runtime
  detection and X11 fallback window.
- Risk: overlay memory cost. Mitigation: single process, shared GTK stack,
  measured at about 40 MB and accepted.
- Risk: duplicate notifications. Mitigation: explicit routing rule and tests.
- Risk: card storms during bursts. Mitigation: queue, dedup, and visible card cap.

## Works without later stages

Overlay and notifications work for text messages and call/transfer events as
defined by earlier stages. No attachments, voice, or remote desktop code needed.
