# Stage 9 — Attachments

## Goal

Send and receive images, audio clips, and arbitrary files as attachments over
QUIC streams. Use a BLAKE3 content-addressed store with chunked, resumable
transfers. Messages reference the attachment hash, so duplicates dedupe.

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

Stages 1, 3, 5, and 6 complete.

## Deliverables

- `crates/core/src/attachment.rs`
- `crates/net/src/transfer/{mod.rs,chunk.rs,resume.rs}`
- `crates/daemon/src/attachments/{mod.rs,store.rs,transfer.rs}`
- `crates/ui` attachment widgets.
- D-Bus additions in `crates/daemon/src/service.rs`.
- CLI subcommands `send-file`, `download`.

## Content-addressed store

- [ ] Hash file content with BLAKE3; hash is the storage key.
- [ ] Store files under `$XDG_DATA_HOME/ticonnect/attachments/<first2>/<hash>`.
- [ ] Verify hash on write; reject mismatch.
- [ ] Deduplicate identical content across messages and peers.
- [ ] Store metadata in `attachments`: hash, size, path, mime.
- [ ] Detect MIME from extension and optional content sniffing.
- [ ] Enforce max size (configurable, default 2 GiB) before accepting.
- [ ] Never execute or open attachments automatically.
- [ ] Clean up orphaned partial files on startup with a documented policy.

## Transfer protocol

- [ ] Message references `attachment_hash` and metadata (name, size, mime).
- [ ] Sender opens a QUIC stream per attachment transfer.
- [ ] Chunk size configurable; default 64 KiB.
- [ ] Each chunk frame includes hash, offset, length, and payload.
- [ ] Receiver writes to a temp file and verifies each chunk offset.
- [ ] Persist partial state: hash, bytes received, temp path, peer id.
- [ ] Resume requests missing ranges from the sender.
- [ ] Support cancellation and timeout.
- [ ] Bound concurrent transfers per peer and globally.
- [ ] Emit progress signals with bytes and total.
- [ ] Emit `TransferDone` or `TransferFailed` with reason.

## Receiving

- [ ] On message with attachment hash, check store first.
- [ ] If present, skip transfer and link the message immediately.
- [ ] If absent, start or resume transfer.
- [ ] Verify final hash before publishing the file into the store.
- [ ] Insert the `attachments` row only after verification.
- [ ] Ignore attachments from `ignore` peers; store silently for `mute`.

## UI

- [ ] Composer attach button opens a file chooser.
- [ ] Show transfer progress inline in the conversation.
- [ ] Render images inline with a size cap and click-to-open.
- [ ] Render audio clips with play/pause using `rodio` or GTK media.
- [ ] Render generic files as a card with name, size, and save action.
- [ ] Save attachment to a user-chosen path.
- [ ] Handle missing or failed attachments with a clear state.
- [ ] Do not block the UI during transfer.

## D-Bus additions

- [ ] `SendAttachment(s peer_id, s path, s mime) -> s message_id`.
- [ ] `GetTransferState(s hash) -> a{sv}`.
- [ ] `CancelTransfer(s hash)`.
- [ ] `SaveAttachment(s hash, s destination) -> b`.
- [ ] Signals `TransferProgress(s, t, t)`, `TransferDone(s)`,
  `TransferFailed(s, s)`.

## Tests

- [ ] Hash and store round-trip; duplicate content dedupes.
- [ ] Chunk encode/decode and offset validation.
- [ ] Resume after simulated disconnect at several offsets.
- [ ] Hash mismatch rejects the file and removes temp data.
- [ ] Size limit rejects oversized files before transfer.
- [ ] Concurrent transfers respect limits.
- [ ] Two-node integration: image, audio, and generic file transfer.
- [ ] Store-first path skips transfer when content already exists.
- [ ] Fuzz test: chunk decoder never panics.

## Security

- [ ] Verify BLAKE3 hash before publishing.
- [ ] Validate MIME and size; reject path traversal in file names.
- [ ] Temp files created with restrictive permissions.
- [ ] Do not follow symlinks when writing.
- [ ] No automatic execution or opening.
- [ ] Disk quota or free-space check before accepting large transfers.
- [ ] Ignore policy prevents transfer initiation.

## Docs and acceptance

- [ ] Document store layout and dedupe behavior.
- [ ] Document transfer protocol, chunk format, and resume.
- [ ] Document size limits and configuration.
- [ ] Document how to save attachments and where they live.

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
