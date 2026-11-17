# Stage 7 — GTK4 UI

## Goal

Build the GTK4 main window as a separate process that talks to the daemon only
over D-Bus. Provide users, chat threads, message state, settings, and the trust
dialog. The daemon remains fully functional when the UI is closed.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-7).
Component graph: [UI](../../docs/systems/generated/components/ui.md).

## Scope

- `ticonnect-ui` binary with `gtk4`, `libadwaita`, and `relm4`.
- User list, conversation view, message composer.
- Message state indicators and policy controls.
- Settings pages.
- Automatic trust dialog (PIN and QR).
- D-Bus client with signal subscriptions.

## Non-goals

- No overlay toasts (Stage 8).
- No file or audio attachments (Stage 9).
- No call or remote desktop views (Stages 10–11).
- No direct database or network access from the UI.

## Prerequisites

Stages 1, 1.5, 2, and 6 complete.

## Deliverables

- `crates/ui/src/main.rs` — replace the Stage 1 stub with GTK application
  startup and `anyhow` error reporting.
- `crates/ui/src/app.rs` — relm4 application and components.
- `crates/ui/src/client.rs` — D-Bus client wrapper.
- `crates/ui/src/views/{users.rs,chat.rs,settings.rs,trust.rs}`
- `crates/ui/resources/` — desktop file, icons, CSS.
- D-Bus name `org.ticonnect.UI` with `Activate(view)`.

## Application shell

- [x] `AdwApplication` with application id `org.ticonnect.UI`.
- [x] Single main window with adaptive layout.
- [x] Sidebar: users and presence.
- [x] Content: selected conversation.
- [x] Header bar: window title, connection state, menu.
- [x] Graceful behavior when the daemon is absent: show a clear banner and retry.
- [x] Reconnect to daemon signals automatically on daemon restart.
- [x] Own bus name `org.ticonnect.UI` and implement `Activate(view)`.

## Users and chat

- [x] List trusted users with display name, fingerprint, policy, and presence.
- [x] Select user opens or creates the conversation.
- [x] Chat view renders text messages in send order.
- [x] Show message state: queued, sent, delivered, read, failed, expired.
- [x] Composer sends via `SendMessage`.
- [x] Enter sends; Shift+Enter inserts a newline.
- [x] Mark visible messages read when scrolled into view.
- [x] Scroll pagination for history.
- [x] Empty, loading, and error states for users and messages.
- [x] Policy control per user (accept/mute/ignore).

## Settings

- [x] Display name.
- [x] Per-user policy list.
- [x] Ringtone path (placeholder until Stage 10).
- [x] Discovery enable/disable and backend toggles.
- [x] Autostart enable/disable via XDG autostart file.
- [x] Overlay enable/disable (placeholder until Stage 8).
- [x] Read receipts toggle.
- [x] DND toggle.
- [x] Outbox TTL days.
- [x] Settings apply immediately and persist directly to `config.toml` through
  `ticonnect-settings`.
- [x] Refresh displayed settings on `SettingsChanged`; no `SetSetting` D-Bus
  call.

## Automatic trust dialog

- [x] Start trust and show 6-digit PIN with expiry countdown.
- [x] Enter a user PIN to join a trust session.
- [x] Show QR payload as a QR image.
- [x] Accept QR payload from clipboard or file for testing.
- [x] Show success and failure states with reason.
- [x] Cancel trust cleanly.

## D-Bus client

- [x] Typed wrapper over `zbus` proxy for `org.ticonnect.Daemon`.
- [x] Subscribe to `MessageReceived`, `MessageStateChanged`,
  `UserConnected`, `UserDisconnected`, `Automatic trustCompleted`, `SettingsChanged`.
- [x] Dispatch signals to the GTK main loop safely.
- [x] Handle daemon disappearance and reappearance.
- [x] No blocking calls on the GTK main thread.

## Tests

- [x] Unit test: D-Bus client maps daemon errors to UI errors.
- [x] Widget test or headless test for user list model updates.
- [ ] Manual: send and receive messages with the UI open.
- [ ] Manual: close UI, send from CLI, reopen, confirm history and states.
- [ ] Manual: daemon restart while UI is open recovers.
- [ ] Manual: trust flow with PIN and QR.
- [ ] Manual: settings changes persist across restart and the daemon reflects
  them without restart.
- [ ] Accessibility check: keyboard navigation, labels, focus order.

## Security

- [x] UI never reads keys or the database directly.
- [x] UI only uses the documented D-Bus interface.
- [x] Fingerprints shown exactly as the daemon reports them.
- [x] No message body logging.
- [x] Automatic trust PIN not persisted in logs or config.

## Docs and acceptance

- [x] Document UI/daemon split and why the UI can be closed.
- [x] Document `org.ticonnect.UI.Activate(view)` contract.
- [x] Document keyboard shortcuts.
- [x] Add screenshots or a short usage guide.

## Acceptance criteria

- UI launches, lists users, and shows conversations.
- Sending and receiving works with live updates.
- Message states match daemon state.
- Automatic trust succeeds with PIN and QR.
- Settings persist and apply.
- Closing the UI does not affect daemon message reception.
- `Activate(view)` raises the window on the correct view.

## Demo

1. Start daemon and UI.
2. Pair with another node via PIN.
3. Exchange messages and show state changes.
4. Change a setting and restart UI.
5. Call `Activate("chat")` over D-Bus and show the window raised.

## Risks and mitigations

- Risk: UI logic creeping into the daemon. Mitigation: strict D-Bus boundary and
  review checklist.
- Risk: GTK main-loop blocking. Mitigation: async D-Bus client and no sync I/O
  on the main thread.
- Risk: accessibility gaps. Mitigation: test keyboard-only and screen reader
  labels before stage close.
- Risk: signal storms from high message volume. Mitigation: batch UI updates and
  coalesce refresh events.

## Works without later stages

The full text messaging UI works with no overlay, attachments, calls, or remote
desktop features present.
