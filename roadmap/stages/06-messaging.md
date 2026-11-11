# Stage 6 — Messaging

## Goal

Implement the local replica messaging model over established transport: signed
immutable records, per-sender sequence numbers, outbox with expiry, gap sync on
reconnect, read receipts, policy (accept/mute/ignore/DND), and a tray menu. This
is the first end-to-end usable milestone without a GUI.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-6).
Component graphs: [MSG](../../docs/systems/generated/components/msg.md), [POL](../../docs/systems/generated/components/pol.md), [TRAY](../../docs/systems/generated/components/tray.md).

## Scope

- Message record creation, signing, verification, and storage.
- Text messages only in this stage; attachments come in Stage 9.
- Outbox with retry and 7-day default expiry.
- Gap sync on reconnect using `peer_state`.
- `delivered` and `read` ack records with coalescing.
- Per-peer policy and global DND.
- Tray icon via `ksni`.
- CLI commands to send, list, and mark read.

## Non-goals

- No UI window, overlay, or notifications.
- No files, images, or audio.
- No edits, deletions, or group chats.
- No global ordering; clock skew is accepted.

## Prerequisites

Stages 1, 1.5, 3, and 5 complete.

## Deliverables

- `crates/core/src/message.rs` — record types, canonical encoding, signing.
- `crates/core/src/ack.rs`
- `crates/daemon/src/messaging/{mod.rs,store.rs,outbox.rs,sync.rs,receipts.rs,policy.rs}`
- `crates/daemon/src/tray.rs`
- `crates/cli` subcommands `send`, `messages`, `mark-read`, `policy`.
- D-Bus additions in `crates/daemon/src/service.rs`.

## Message records

- [x] Define `MessageKind`: `text`, `image`, `audio`, `file`, `ack`.
- [x] Define a canonical encoding for signing: conversation id, sender id,
  seq, sent_at, kind, target_id, body, attachment_hash.
- [x] Sender generates UUIDv7 message id.
- [x] Sender assigns monotonic `seq` per conversation per sender.
- [x] Sign canonical bytes with the Ed25519 identity key.
- [x] Receiver verifies signature and `seq` before insert.
- [x] Duplicate message ids and duplicate `(conversation, sender, seq)` are
  ignored, not errors.
- [x] Messages are immutable; no update path.
- [x] Store `received_at` locally.
- [x] Render order uses `sent_at` with `received_at` fallback when clocks skew.
- [x] Conversation id is deterministic for the peer pair; document derivation.

## Storage

- [x] Implement message insert, get by id, list by conversation with pagination.
- [x] Enforce `UNIQUE(conversation_id, sender_id, seq)`.
- [x] Store signature as `BLOB` and verify on insert.
- [x] Update `peer_state.last_recv_seq` transactionally with insert.
- [x] Store ack records with `target_id` and kind `ack`.
- [x] Add indexes needed for pagination and receipt lookup.

## Outbox

- [x] Queue outbound messages in `outbox` with `attempts`, `next_try`,
  `expires_at`.
- [x] Default TTL is 7 days; configurable via `outbox.ttl_days`.
- [x] Retry with exponential backoff on peer rediscovery and send failure.
- [x] Flush outbox for a peer after a successful connection and gap sync.
- [x] Expired rows are dropped and the message is surfaced as `failed` or
  `expired`.
- [x] Sending to a muted peer still queues and delivers, but silently.
- [x] Sending to an ignored peer is dropped at receive; sender sees `sent` once
  transport acknowledges, or `failed` if never connected. Document semantics.
- [x] Outbox depth exposed in `GetStatus`.

## Gap sync

- [x] On authenticated connection, exchange `peer_state`
  (`last_sent_seq`, `last_recv_seq`).
- [x] Each side computes missing ranges and requests them.
- [x] Stream missing records over a dedicated QUIC stream.
- [x] Verify and insert records; ignore duplicates.
- [x] Flush outbox after sync completes.
- [x] Sync is resumable and bounded in memory per batch.
- [x] Emit `SyncCompleted(peer_id, count)`.

## Read receipts

- [x] `delivered` ack created when the receiver stores the message.
- [x] `read` ack created when the conversation is visible in the UI and the
  message is scrolled into view; CLI `mark-read` simulates this before the UI.
- [x] Acks are signed records stored like messages.
- [x] Coalesce acks over a short interval (default 500 ms).
- [x] Suppress receipts for mute/ignore peers unless the message was read.
- [x] Global `read_receipts.enabled` disables sending but still honors inbound.
- [x] Message state exposed as `queued`, `sent`, `delivered`, `read`, `failed`,
  `expired`.

## Policy

- [x] Per-peer policy stored in `peers.policy`.
- [x] `accept`: store and notify.
- [x] `mute`: store silently, no notification.
- [x] `ignore`: drop inbound with no storage and no notification.
- [x] Global DND mode suppresses notifications, not storage.
- [x] Policy changes over D-Bus and tray menu.
- [x] Emit `PolicyChanged(peer_id, policy)`.

## Tray

- [x] `ksni` StatusNotifierItem with icon, title, and menu.
- [x] Menu: open app, DND toggle, per-peer policy submenu, quit.
- [x] Tray is optional; daemon runs when tray is unavailable.
- [x] No tray dependency in headless test runs.

## D-Bus additions

- [x] `SendMessage(s peer_id, s body) -> s message_id`.
- [x] `ListMessages(s conversation_id, u limit, u offset) -> aa{sv}`.
- [x] `MarkRead(s conversation_id, as message_ids)`.
- [x] `GetMessageState(s message_id) -> s`.
- [x] `SetPeerPolicy(s peer_id, s policy)`.
- [x] `SetDnd(b enabled)`.
- [x] Signals `MessageReceived(a{sv})`, `MessageStateChanged(s, s)`,
  `SyncCompleted(s, u)`, `PolicyChanged(s, s)`.

## Tests

- [x] Sign/verify round-trip and tamper detection.
- [x] Duplicate insert is ignored.
- [x] Out-of-order seq insertion is handled per sync rules.
- [x] Outbox retry and expiry with a controllable clock.
- [x] Gap sync between two stores with missing ranges on both sides.
- [x] Receipt coalescing sends one ack for multiple messages.
- [x] Mute stores silently; ignore drops.
- [x] DND suppresses notification but stores message.
- [x] Two-node integration: send, receive, delivered, read.
- [x] Reconnect after drop triggers gap sync and outbox flush.
- [x] Clock skew does not panic and falls back to `received_at`.

## Security

- [x] Verify signatures before any storage or ack.
- [x] Bound message body size; reject oversized records.
- [x] Reject seq values that overflow or are absurdly large.
- [x] Do not log message bodies by default.
- [x] Ignore policy is enforced before storage, not after.
- [x] Receipts cannot be forged for messages not stored.

## Docs and acceptance

- [x] Document message canonical encoding and signing.
- [x] Document outbox semantics, TTL, and expiry behavior.
- [x] Document gap sync algorithm and conflict handling.
- [x] Document receipt semantics and suppression rules.
- [x] Document policy semantics for each mode.

## Acceptance criteria

- Two paired nodes exchange text messages in both directions.
- Messages persist across daemon restarts.
- Offline messages queue and deliver on reconnect.
- Outbox expiry marks messages failed after the TTL.
- Delivered and read states appear for sent messages.
- Mute stores silently; ignore drops; DND suppresses notifications.
- Tray menu changes policy.

## Demo

1. Pair two nodes.
2. Send messages from CLI on both sides; list history.
3. Stop one daemon, send, restart, confirm gap sync and outbox flush.
4. Mark read and show state transitions.
5. Change policy to ignore and show inbound drop.
6. Show tray menu policy change.

## Risks and mitigations

- Risk: sync edge cases with gaps on both sides. Mitigation: property tests and
  explicit range computation tests.
- Risk: clock skew confusion. Mitigation: documented render order and stable
  local `received_at`.
- Risk: receipt storms. Mitigation: coalescing and suppression rules.

## Works without later stages

Full text messaging, history, receipts, policy, and tray work with no UI,
overlay, attachments, calls, or remote desktop.
