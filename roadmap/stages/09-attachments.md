# Stage 9 — Attachments

> Automation complete. Hardware-independent store, chunk, resume, and two-node transfer tests pass.

## Goal

Send and receive images, audio clips, and arbitrary files as attachments over
QUIC streams. Use a BLAKE3 content-addressed store with chunked, resumable
transfers. Messages reference the attachment hash, so duplicates dedupe.

## Systems and components

Component IDs and relations: [stage component map](../../docs/systems/generated/stage-map.md#stage-9).
Component graphs: [ATT](../../docs/systems/generated/components/att.md), [WIRE](../../docs/systems/generated/components/wire.md).

## Scope

- Content-addressed attachment store.
- Chunked upload/download over dedicated QUIC streams.
- Resumable partial state persisted in SQLite.
- Size and MIME validation.
- UI rendering for images, audio clips, and generic files.
- CLI send/receive for testing.

## Non-goals

- No streaming playback of partially downloaded audio.
- No transcoding, thumbnailing pipeline, or image resizing beyond display.
- No group attachments.
- No remote desktop video (Stage 11).

## Prerequisites

Stages 1, 1.5, 3, 5, 6, and 7 complete.

## Deliverables

- `crates/core/src/attachment.rs`
- `crates/net/src/transfer/{mod.rs,chunk.rs,resume.rs}`
- `crates/daemon/src/attachments/{mod.rs,store.rs,transfer.rs}`
- `crates/ui` attachment widgets.
- D-Bus additions in `crates/daemon/src/service.rs`.
- CLI subcommands `send-file`, `download`.

## Content-addressed store

- [x] Hash file content with BLAKE3; hash is the storage key.
- [x] Store files under `$XDG_DATA_HOME/ticonnect/attachments/<first2>/<hash>`.
- [x] Verify hash on write; reject mismatch.
- [x] Deduplicate identical content across messages and peers.
- [x] Store metadata in `attachments`: hash, size, path, mime.
- [x] Detect MIME from extension and optional content sniffing.
- [x] Enforce max size (configurable, default 2 GiB) before accepting.
- [x] Never execute or open attachments automatically.
- [x] Clean up orphaned partial files on startup with a documented policy.

## Transfer protocol

- [x] Message references `attachment_hash` and metadata (name, size, mime).
- [x] Sender opens a QUIC stream per attachment transfer.
- [x] Chunk size configurable; default 64 KiB.
- [x] Each chunk frame includes hash, offset, length, and payload.
- [x] Receiver writes to a temp file and verifies each chunk offset.
- [x] Persist partial state: hash, bytes received, temp path, peer id.
- [x] Resume requests missing ranges from the sender.
- [x] Support cancellation and timeout.
- [x] Bound concurrent transfers per peer and globally.
- [x] Emit progress signals with bytes and total.
- [x] Emit `TransferDone` or `TransferFailed` with reason.

## Receiving

- [x] On message with attachment hash, check store first.
- [x] If present, skip transfer and link the message immediately.
- [x] If absent, start or resume transfer.
- [x] Verify final hash before publishing the file into the store.
- [x] Insert the `attachments` row only after verification.
- [x] Ignore attachments from `ignore` peers; store silently for `mute`.

## UI

- [x] Composer attach button opens a file chooser.
- [x] Show transfer progress inline in the conversation.
- [x] Render images inline with a size cap and click-to-open.
- [x] Render audio clips with play/pause using `rodio` or GTK media.
- [x] Render generic files as a card with name, size, and save action.
- [x] Save attachment to a user-chosen path.
- [x] Handle missing or failed attachments with a clear state.
- [x] Do not block the UI during transfer.

## D-Bus additions

- [x] `SendAttachment(s peer_id, s path, s mime) -> s message_id`.
- [x] `GetTransferState(s hash) -> a{sv}`.
- [x] `CancelTransfer(s hash)`.
- [x] `SaveAttachment(s hash, s destination) -> b`.
- [x] Signals `TransferProgress(s, t, t)`, `TransferDone(s)`,
  `TransferFailed(s, s)`.

## Tests

- [x] Hash and store round-trip; duplicate content dedupes.
- [x] Chunk encode/decode and offset validation.
- [x] Resume after simulated disconnect at several offsets.
- [x] Hash mismatch rejects the file and removes temp data.
- [x] Size limit rejects oversized files before transfer.
- [x] Concurrent transfers respect limits.
- [x] Two-node integration: image, audio, and generic file transfer.
- [x] Store-first path skips transfer when content already exists.
- [x] Fuzz test: chunk decoder never panics.

## Security

- [x] Verify BLAKE3 hash before publishing.
- [x] Validate MIME and size; reject path traversal in file names.
- [x] Temp files created with restrictive permissions.
- [x] Do not follow symlinks when writing.
- [x] No automatic execution or opening.
- [x] Disk quota or free-space check before accepting large transfers.
- [x] Ignore policy prevents transfer initiation.

## Docs and acceptance

- [x] Document store layout and dedupe behavior.
- [x] Document transfer protocol, chunk format, and resume.
- [x] Document size limits and configuration.
- [x] Document how to save attachments and where they live.

## Acceptance criteria

- Send and receive an image, an audio clip, and a generic file.
- Interrupt a transfer and resume it without corruption.
- Duplicate attachment does not consume extra disk.
- Hash mismatch is rejected.
- UI shows progress and renders each attachment type.
- Ignore policy blocks inbound attachments.

## Demo

1. Send a large file between two nodes; interrupt and resume.
2. Send the same file again; show dedupe.
3. Send an image and an audio clip; show inline rendering.
4. Send a corrupt chunk in a test harness; show rejection.

## Risks and mitigations

- Risk: partial-state corruption. Mitigation: temp files plus final hash check;
  resume only after validating existing prefix.
- Risk: disk exhaustion. Mitigation: size limits, free-space checks, and
  configurable quota.
- Risk: unsafe file handling. Mitigation: no auto-open, path sanitization, and
  restrictive permissions.

## Works without later stages

Attachments transfer and render with no voice calls, remote desktop, or overlay
dependency.
