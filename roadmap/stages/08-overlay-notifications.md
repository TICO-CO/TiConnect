# Stage 8 — Overlay & Notifications

## Goal

Add the always-running corner overlay agent that shows toast cards for incoming
messages, incoming calls, and completed transfers. It must not depend on the UI
being open. When the overlay is disabled or unavailable, events fall back to
system notifications.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-8).
Component graph: [OVL](../../docs/systems/generated/components/ovl.md).

Call cards and transfer cards are extension points in Stages 10 and 9. Stage 8
ships message cards only.

## Scope

- `ticonnect-ovl` GTK4 layer-shell agent on Wayland.
- X11 fallback: undecorated always-on-top window.
- Toast cards for `MessageReceived`.
- Extension hooks for `IncomingCall` cards in Stage 10 and `TransferDone` cards
  in Stage 9.
- Click-to-activate via `org.ticonnect.UI.Activate(view)`.
- Auto-dismiss behavior for message cards.
- `notify-rust` fallback.
- Overlay enable/disable setting.

## Non-goals

- No message composition or history in the overlay.
- No call answering beyond activating the UI.
- No tray (already in Stage 6).
- No remote desktop UI.

## Prerequisites

Stages 1, 1.5, 2, and 6 complete. Stage 7 is optional for click activation; without
the UI, clicks fall back to notifications or are ignored.

## Deliverables

- `crates/overlay/src/main.rs` — replace the Stage 1 stub with overlay startup
  and `anyhow` error reporting.
- `crates/overlay/src/card.rs`
- `crates/overlay/src/layer_shell.rs`
- `crates/overlay/src/x11_fallback.rs`
- `crates/overlay/src/notifications.rs`
- `crates/overlay/src/client.rs` — daemon signal subscription.
- Packaging autostart entry for the overlay.

## Wayland overlay

- [x] Use `gtk4-layer-shell` with `Layer::Overlay`.
- [x] Anchor bottom-right.
- [x] Transparent window with rounded card and shadow.
- [x] `KeyboardMode::None` so the overlay never steals focus.
- [x] Only the card surface takes pointer input.
- [x] Detect layer-shell availability at runtime; fall back cleanly.
- [x] Respect scale factor and multi-monitor placement.
- [x] Card slide-in animation with reduced-motion support.

## X11 fallback

- [x] Undecorated GTK4 window with `keep_above` and `skip_taskbar_hint`.
- [x] Position bottom-right of the primary monitor.
- [x] No focus stealing.
- [x] Same card widget and behavior as Wayland.
- [x] Hide when no cards are visible.

## Card behavior

- [x] Message card: sender name, preview text, timestamp.
- [ ] Incoming call card extension point: caller name, accept and decline
  actions, persists until answered or declined. Enabled in Stage 10.
- [ ] Transfer done card extension point: file name and size. Enabled in Stage 9.
- [x] Message cards auto-dismiss after a configurable timeout (default 5 s).
- [x] Hover pauses dismissal.
- [x] Clicking a card calls `org.ticonnect.UI.Activate(view)`.
- [x] If the UI bus name is absent, clicking opens the app if configured or does
  nothing and logs.
- [x] Queue cards instead of overlapping; cap visible cards.
- [x] Deduplicate repeated events for the same message id.
- [x] Respect policy: mute and ignore suppress cards.
- [x] Respect DND: suppress all non-call cards; calls follow config.

## Notifications fallback

- [x] Use `notify-rust` when the overlay is disabled or fails to initialize.
- [x] Notification actions map to activate or decline where supported.
- [x] Do not send both overlay and system notification for the same event.
- [x] Handle notification server absence without crashing.

## D-Bus and config

- [x] Overlay subscribes directly to `org.ticonnect.Daemon` signals.
- [x] Overlay does not depend on the UI process.
- [x] Config key `overlay.enabled` read from TOML settings at start and after
  config reload changes.
- [x] Autostart entry starts the overlay after login.
- [x] Single-instance guard for the overlay.

## Tests

- [ ] Manual Wayland: card appears bottom-right, does not steal focus, dismisses.
- [ ] Manual X11: fallback window behaves the same.
- [ ] Manual: incoming call card persists until action.
- [ ] Manual: click activates UI on the correct view.
- [ ] Manual: UI closed; overlay still shows cards and does not crash.
- [ ] Manual: disable overlay; system notifications appear instead.
- [ ] Manual: DND suppresses cards.
- [ ] Manual: multi-monitor and HiDPI placement.
- [x] Automated: card queue and dedup logic unit tests.

## Security

- [x] Overlay has no network, storage, or key access.
- [x] Only subscribes to the documented daemon signals.
- [x] Message previews respect policy and DND.
- [x] No focus stealing; keyboard input never routed to the overlay.
- [x] No message bodies written to logs.

## Docs and acceptance

- [x] Document overlay enable/disable and fallback behavior.
- [x] Document focus and input guarantees.
- [x] Document X11 vs Wayland differences.
- [x] Document the idle memory cost and why it is accepted.

## Acceptance criteria

- Wayland overlay shows corner cards without stealing focus.
- X11 fallback shows equivalent cards.
- Overlay works with the UI closed.
- Clicking a card raises the UI on the relevant view.
- Incoming call and transfer cards appear when Stages 10 and 9 are present.
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
