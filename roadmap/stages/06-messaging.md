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

- [ ] Define `MessageKind`: `text`, `image`, `audio`, `file`, `ack`.
- [ ] Define a canonical encoding for signing: conversation id, sender id,
  seq, sent_at, kind, target_id, body, attachment_hash.
- [ ] Sender generates UUIDv7 message id.
- [ ] Sender assigns monotonic `seq` per conversation per sender.
- [ ] Sign canonical bytes with the Ed25519 identity key.
- [ ] Receiver verifies signature and `seq` before insert.
- [ ] Duplicate message ids and duplicate `(conversation, sender, seq)` are
  ignored, not errors.
- [ ] Messages are immutable; no update path.
- [ ] Store `received_at` locally.
- [ ] Render order uses `sent_at` with `received_at` fallback when clocks skew.
- [ ] Conversation id is deterministic for the peer pair; document derivation.

## Storage

- [ ] Implement message insert, get by id, list by conversation with pagination.
- [ ] Enforce `UNIQUE(conversation_id, sender_id, seq)`.
- [ ] Store signature as `BLOB` and verify on insert.
- [ ] Update `peer_state.last_recv_seq` transactionally with insert.
- [ ] Store ack records with `target_id` and kind `ack`.
- [ ] Add indexes needed for pagination and receipt lookup.

## Outbox

- [ ] Queue outbound messages in `outbox` with `attempts`, `next_try`,
  `expires_at`.
- [ ] Default TTL is 7 days; configurable via `outbox.ttl_days`.
- [ ] Retry with exponential backoff on peer rediscovery and send failure.
- [ ] Flush outbox for a peer after a successful connection and gap sync.
- [ ] Expired rows are dropped and the message is surfaced as `failed` or
  `expired`.
- [ ] Sending to a muted peer still queues and delivers, but silently.
- [ ] Sending to an ignored peer is dropped at receive; sender sees `sent` once
  transport acknowledges, or `failed` if never connected. Document semantics.
- [ ] Outbox depth exposed in `GetStatus`.

## Gap sync

- [ ] On authenticated connection, exchange `peer_state`
  (`last_sent_seq`, `last_recv_seq`).
- [ ] Each side computes missing ranges and requests them.
- [ ] Stream missing records over a dedicated QUIC stream.
- [ ] Verify and insert records; ignore duplicates.
- [ ] Flush outbox after sync completes.
- [ ] Sync is resumable and bounded in memory per batch.
- [ ] Emit `SyncCompleted(peer_id, count)`.

## Read receipts

- [ ] `delivered` ack created when the receiver stores the message.
- [ ] `read` ack created when the conversation is visible in the UI and the
  message is scrolled into view; CLI `mark-read` simulates this before the UI.
- [ ] Acks are signed records stored like messages.
- [ ] Coalesce acks over a short interval (default 500 ms).
- [ ] Suppress receipts for mute/ignore peers unless the message was read.
- [ ] Global `read_receipts.enabled` disables sending but still honors inbound.
- [ ] Message state exposed as `queued`, `sent`, `delivered`, `read`, `failed`,
  `expired`.

## Policy

- [ ] Per-peer policy stored in `peers.policy`.
- [ ] `accept`: store and notify.
- [ ] `mute`: store silently, no notification.
- [ ] `ignore`: drop inbound with no storage and no notification.
- [ ] Global DND mode suppresses notifications, not storage.
- [ ] Policy changes over D-Bus and tray menu.
- [ ] Emit `PolicyChanged(peer_id, policy)`.

## Tray

- [ ] `ksni` StatusNotifierItem with icon, title, and menu.
- [ ] Menu: open app, DND toggle, per-peer policy submenu, quit.
- [ ] Tray is optional; daemon runs when tray is unavailable.
- [ ] No tray dependency in headless test runs.

## D-Bus additions

- [ ] `SendMessage(s peer_id, s body) -> s message_id`.
- [ ] `ListMessages(s conversation_id, u limit, u offset) -> aa{sv}`.
- [ ] `MarkRead(s conversation_id, as message_ids)`.
- [ ] `GetMessageState(s message_id) -> s`.
- [ ] `SetPeerPolicy(s peer_id, s policy)`.
- [ ] `SetDnd(b enabled)`.
- [ ] Signals `MessageReceived(a{sv})`, `MessageStateChanged(s, s)`,
  `SyncCompleted(s, u)`, `PolicyChanged(s, s)`.

## Tests

- [ ] Sign/verify round-trip and tamper detection.
- [ ] Duplicate insert is ignored.
- [ ] Out-of-order seq insertion is handled per sync rules.
- [ ] Outbox retry and expiry with a controllable clock.
- [ ] Gap sync between two stores with missing ranges on both sides.
- [ ] Receipt coalescing sends one ack for multiple messages.
- [ ] Mute stores silently; ignore drops.
- [ ] DND suppresses notification but stores message.
- [ ] Two-node integration: send, receive, delivered, read.
- [ ] Reconnect after drop triggers gap sync and outbox flush.
- [ ] Clock skew does not panic and falls back to `received_at`.

## Security

- [ ] Verify signatures before any storage or ack.
- [ ] Bound message body size; reject oversized records.
- [ ] Reject seq values that overflow or are absurdly large.
- [ ] Do not log message bodies by default.
- [ ] Ignore policy is enforced before storage, not after.
- [ ] Receipts cannot be forged for messages not stored.

## Docs and acceptance

- [ ] Document message canonical encoding and signing.
- [ ] Document outbox semantics, TTL, and expiry behavior.
- [ ] Document gap sync algorithm and conflict handling.
- [ ] Document receipt semantics and suppression rules.
- [ ] Document policy semantics for each mode.

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
