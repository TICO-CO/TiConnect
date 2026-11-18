# Web UI migration

Migration of the TiConnect desktop interface from GTK4/libadwaita/relm4 widgets
to a WebKitGTK webview with a Svelte frontend.

Status: in progress. The Rust host, bridge, and old-GTK removal are landed; the
Svelte frontend is in progress. Check items off as they land and keep the file
current.

Progress:

- [x] Core serde derives and JSON shape tests.
- [x] `LinkHandle` moved to tokio tasks; relm4 removed from the UI crate.
- [x] Loopback HTTP + WebSocket bridge with token, origin, and host checks.
- [x] JSON-RPC dispatch, event fanout, upload and ticket download endpoints.
- [x] GTK4 WebKitGTK host, activation service, and single-instance forwarding.
- [x] Old GTK views, libadwaita, relm4, qrcodegen, and UI stylesheet removed.
- [x] Svelte 5 + TypeScript + Vite scaffold with typed bridge client.
- [x] Svelte routes, stores, and components (`crates/ui/web`).
- [x] Committed Vite dist and CI freshness gate.
- [x] Quiet Grove screens: chat, collapsed rail, profile, settings, watch party, call, sharing.
- [x] In-app request/active cards, message toasts and YouTube player.
- [x] Remote desktop consent without a PIN.
- [x] Native overlay request and toast surfaces restyled; active session cards added.
- [x] Frontend tests; manual desktop matrix remains pending.

## Decision

- Keep the existing GTK4 process, window, `org.ticonnect.UI` activation service,
  single-instance forwarding, settings/autostart handling, and runtime
  bootstrap.
- Replace the relm4/libadwaita views with one WebKitGTK 6.0 webview
  (`webkit6` crate).
- Frontend: Svelte 5 + TypeScript + Vite.
- Rust/JS boundary: a loopback HTTP + WebSocket bridge that wraps the existing
  `LinkHandle` D-Bus client. JavaScript never talks to D-Bus directly.
- The old GTK views are deleted in the same pass. No fallback UI.

Why not wry/tao: stable wry is GTK3 + webkit2gtk 4.1 and cannot share a process
with the GTK4/libadwaita shell. The GTK4/WebKit6 wry port is still an open,
unreleased pull request. Direct `webkit6` bindings are stable and match the
workspace's `gtk4` 0.11 line.

## Architecture

```
ticonnect-ui process
  GTK4 Application/Window
    ├─ WebKitGTK 6.0 WebView (assets served from embedded dist)
    └─ activation D-Bus org.ticonnect.UI (unchanged)
  tokio runtime
    ├─ client.rs LinkHandle (D-Bus daemon client, mostly unchanged)
    ├─ bridge: axum on 127.0.0.1:<ephemeral> + token
    │    ├─ GET /            embedded Svelte dist
    │    ├─ GET /assets/*    embedded assets
    │    ├─ GET /ws           bidirectional JSON-RPC + server events
    │    └─ GET /attachment/{hash}?ticket=  ticketed download endpoint
    └─ event fanout: DaemonEvent → WS; ActivationEvent → WS + window raise
```

The daemon, overlay, tray, CLI, and process model are unchanged. The UI remains
a separate process that never reads keys, SQLite, or the network directly.

## Bridge protocol

Request:

```json
{"id": 1, "method": "send_message", "params": {"user_id": "...", "body": "hi"}}
```

Reply:

```json
{"id": 1, "ok": true, "result": "message-id"}
{"id": 1, "ok": false, "error": {"code": "daemon_unavailable", "message": "..."}}
```

Server event:

```json
{"type": "message_received", "message": {"id": "..."}}
```

Methods mirror `LinkHandle` plus host-only operations:

- every current `crates/ui/src/client.rs` command
- `settings_save`, `autostart_get`, `autostart_set`
- `POST /attachment?user_id=` uploads browser `<input type=file>` bytes and
  returns the message id
- `POST /avatar` uploads avatar bytes
- `GET /attachment/{hash}?ticket=` and `GET /avatar/{fingerprint}?ticket=`
  download bytes; tickets are minted over JSON-RPC
- `bootstrap` (identity, settings, appearance, version, connection state)
- `activate` event push (`users|chat|settings|remote|chat:<node-id>`)

Serialization: derive `serde::Serialize` and `serde::Deserialize` on the data
types in `crates/core/src/ipc/events.rs`. Map `DaemonEvent` to an explicit
`{type, data}` representation in the bridge so that adding a variant without a
mapping is a compile error.

## Files

Add:

- `crates/ui/src/bridge/mod.rs` — server lifecycle, auth and origin checks
- `crates/ui/src/bridge/protocol.rs` — JSON-RPC types, method dispatch
- `crates/ui/src/bridge/events.rs` — `DaemonEvent`/`ActivationEvent` to WS messages
- `crates/ui/src/bridge/assets.rs` — `include_dir!` embedded dist and CSP headers
- `crates/ui/src/bridge/security.rs` — token, origin allowlist, dev-mode guard
- `crates/ui/web/` — Svelte app (`src/main.ts`, `App.svelte`, `lib/bridge.ts`,
  `lib/types.ts`, stores, components, `vite.config.ts`, `package.json`, lockfile)
- `crates/ui/assets/` — committed Vite build output
- `crates/ui/tests/bridge.rs` — protocol and auth tests

Rewrite/remove:

- `crates/ui/src/app.rs` — plain `gtk4::Application` with `ApplicationWindow`
  and `webkit6::WebView`; preserve the activation service and duplicate-instance
  forwarding currently near lines 1830–1930.
- `crates/ui/src/client.rs` — replace `relm4::spawn` with a tokio `Handle`; keep
  the command set and reconnect/backoff behavior.
- `crates/ui/src/model.rs` — keep `SettingsForm`, `BlockedUser`, and `Backoff`;
  move presentation, sorting, and label logic to TypeScript.
- Delete `crates/ui/src/views/*`, `crates/ui/resources/`, and the GTK texture
  parts of `avatar.rs` (keep PNG normalization).
- `crates/ui/Cargo.toml` — add `webkit6 = "0.6"`,
  `axum = { version = "0.8", features = ["ws"] }`, `serde_json`, and
  `include_dir`; remove `relm4`, `libadwaita`, and `qrcodegen`. Verify that
  `gtk4` 0.11 stays compatible with `webkit6` 0.6.

## Frontend structure

Routes/stores: `shell`, `users`, `chat`, `user`, `settings`, `call`, `remote`, `watch-party`.

Port in this order:

1. shell: header, connection banner, toasts, appearance
2. users: list, presence, unread count, mute badge, policy menu
3. chat: messages, state badges, composer (Enter sends, Shift+Enter newline),
   scroll pagination, mark-read, typing indicator
4. avatars and attachments (upload/download, progress)
5. user page: fingerprint, current addresses, optional MAC, OS, desktop,
   display server, app version, policy, block/forget
6. settings: every current field, blocked users, DND, outbox TTL, autostart,
   read receipts, discovery, ringtone path, appearance
7. call/remote/watch-party: overlay notifications and YouTube player

Webview hardening:

- ephemeral `WebKitNetworkSession`; no persistent web data
- `WebKitSettings`: disable automatic window opening, disable file URLs, enable
  developer extras only in dev
- deny all permission requests
- `decide-policy`: cancel navigation outside the app origin
- CSP: `default-src 'self'; connect-src 'self' ws://127.0.0.1:*; img-src 'self'
  data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self'`
- no CDN; all assets bundled for offline use

Security warning: the bridge binds loopback only. Production requires a random
128-bit token passed through the URL fragment, sent in the first WebSocket
message, plus strict `Origin` and `Host` validation. Tokenless dev mode is
allowed only when `TICONNECT_DEV_URL` is set and the origin is
`http://127.0.0.1:5173`. Never enable dev mode on a shared machine.

## Build and packaging

- Build output from `crates/ui/web` is committed under `crates/ui/assets`.
  `build.rs` does not invoke Node, so plain `cargo build` works without a JS
  toolchain.
- Make targets: `web-install`, `web-dev`, `web-build`, `web-check`.
- CI:
  - `bun install --frozen-lockfile`, `bun run check`, `bun run test`,
    `bun run build`
  - `git diff --exit-code crates/ui/assets`
  - add `libwebkitgtk-6.0-dev` to the apt install steps
  - headless runs set `WEBKIT_DISABLE_DMABUF_RENDERER=1`
- Nix flake: add `pkgs.webkitgtk_6_0` and `pkgs.nodejs` for the web build.
- Flatpak: verify `org.gnome.Platform` ships WebKitGTK 6.0; add a module if not.
- AppImage/release: verify WebKitGTK 6.0 bundling assumptions; GTK4 currently
  comes from the host.
- Docs: update `architecture.md` (stack table and section 8 UI), `README.md`
  (build deps, UI section, appearance), and `docs/systems/20-ui.md` through
  `docs/systems/graph/components-app.toml`; regenerate with
  `cargo xtask systems`.
- Note the webview memory cost (~100–200 MB web process) in
  `docs/performance.md`; it conflicts with the current idle-memory claims.

## Tests

Rust:

- bridge auth: missing/wrong token, bad origin, dev-mode guard
- JSON-RPC dispatch mapping, unknown method, daemon-unavailable error
- attachment temp-file cleanup
- retain existing settings/autostart tests

Frontend:

- Vitest: bridge reconnect/backoff, message ordering and state, settings
  parsing, composer behavior
- axe-core accessibility smoke
- optional Playwright smoke against a mock bridge

Integration:

- update `tests/local/run.sh scenario_ui` for webview startup and
  `Activate` raising the window
- manual matrix: X11, Weston, KWin, Hyprland, HiDPI, dark/light, IME,
  keyboard-only, automatic trust, attachments, daemon restart

Verification:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo xtask systems --check
cd crates/ui/web && bun run check && bun run test && bun run build
tests/local/run.sh x11 ui
```

## Execution order

1. Spike: `webkit6` + GTK4 window + embedded hello page. Verify build and run
   on X11 and Wayland. De-risks crate versions and packaging.
2. Bridge server + auth + WebSocket + event fanout over the existing
   `LinkHandle`; add Rust tests.
3. Core serde derives, bridge command parity, attachments, settings, activation.
4. Svelte scaffold: tokens from `style.css`, typed bridge client,
   reconnect/backoff.
5. Port views: shell → users → chat → avatars/attachments → user page → settings
   → call/remote.
6. Rewrite `app.rs` host, harden the WebView, preserve activation and
   single-instance behavior, delete GTK views.
7. Build integration: committed dist, Makefile, CI, Nix, Flatpak, docs and graph
   regeneration.
8. Test pass, manual matrix, remove dead dependencies and tests.

## Risks

- WebKitGTK install size and RAM exceed the lightweight-idle goal. Accepted by
  choosing a webview.
- Flatpak/AppImage/Nix must guarantee WebKitGTK 6.0 at runtime; highest
  packaging risk.
- Remote-desktop video has no frame path today. A future path needs `<canvas>`
  plus WebCodecs/MSE over binary WebSocket frames. Design the bridge so binary
  frames can be added without breaking the JSON protocol.
- JavaScript cannot use local file paths. All attachment and file operations go
  through bridge temp files or HTTP endpoints.
- Source/build drift: committed `dist` plus a CI freshness check.
- X11/Wayland WebKit rendering bugs: mitigate with
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` in CI and the manual matrix before merge.
