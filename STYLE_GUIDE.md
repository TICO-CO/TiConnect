# TiConnect Style Guide

**Direction: Quiet Grove.** A night-garden console: cream ink on deep teal, rounded surfaces, circular presence marks, soft green-tinted depth. Dark only. Every value comes from the token block.

## Brief

| | |
|---|---|
| Purpose | Desktop client for a LAN-only messaging, voice, video, watch-party, and remote-desktop daemon |
| Audience | Linux users on a trusted office or home LAN, window open all day |
| Tone | Quiet Grove: botanical field guide, night desk, soft utility |
| Reference | 1970s botanical field guide on dark stock; rounded instrument panels |
| Memorable | The selected user card turns cream, square-edged, with a circular monogram and a quiet presence dot |
| Restraint | No chat bubbles, photo avatars, gradients, sharp corners, hairline grids, or pairing wizard |

## Tokens

```css
:root {
  --font-display: "Konkhmer Sleokchher";
  --font-text:    "Istok Web";

  --step-xs: 0.833rem;  --step-0: 1rem;     --step-1: 1.2rem;
  --step-2: 1.44rem;   --step-3: 1.728rem; --step-4: 2.074rem;
  --step-5: 2.488rem;

  --space-3xs: 4px; --space-2xs: 8px; --space-xs: 12px; --space-s: 16px;
  --space-m: 24px;  --space-l: 32px;  --space-xl: 48px; --space-2xl: 64px;
  --space-3xl: 96px;

  --base: #182B2F; --surface: #213639; --surface-2: #2A403C;
  --rail: #1B3034; --backdrop: #0F1F22;
  --ink: #FFE5B0; --ink-2: #EDB483; --ink-3: #A9A78A;
  --line: #2F4547; --line-strong: #3F5A52;
  --accent: #EDB483;

  --error: #E08A66; --success: #7FAE63; --info: #8AB4C4; --warning: #D9B26A;
  --lamp: #577E46; --fault: var(--error); --warn: var(--warning);

  --radius-s: 10px; --radius-m: 14px; --radius-l: 20px; --radius-pill: 999px;
  --shadow-soft: 0 1px 2px rgba(6,18,18,0.30), 0 14px 32px rgba(6,18,18,0.28);
  --ease-soft: cubic-bezier(0.22, 1, 0.36, 1);
}
```

Rules:

- One interactive accent; under 10% of surface.
- Radii only 10 / 14 / 20 / pill / circle. Selected user card is the one intentional square, edge-to-edge.
- Shadows only on floating layers: request cards, active cards, player, toasts, dialogs.
- Status colors are dots, rules, and 13px labels only.
- All numbers in data labels use tabular figures.

## Typography

Konkhmer Sleokchher is display only, weight 400. Istok Web carries UI and data at 400/700. No monospace face.

| Role | Family | Size | Weight |
|---|---|---|---|
| Wordmark | Konkhmer | `--step-1` | 400 |
| View / user heading | Konkhmer | `--step-3` | 400 |
| Settings / overlay heading | Konkhmer | `--step-2` | 400 |
| Section label | Istok Web | 12px uppercase, 0.08em | 700 |
| User name | Istok Web | 15px | 700 |
| Message body | Istok Web | `--step-0` | 400 |
| Body secondary | Istok Web | `--step-xs` | 400 |
| Button | Istok Web | `--step-xs` sentence case | 700 |
| Data label | Istok Web | 12px, tabular | 400/700 |
| PIN-free numerals | Istok Web | 12–23px tabular | 700 |

Never request synthetic bold. Controls are sentence case; uppercase only for section and data labels.

## Layout

Window 1440 × 900, full-bleed. Minimum 1024 × 720.

| Region | Size | Notes |
|---|---|---|
| Users rail | 320px | `--rail`, 12px inset, 6px gaps, no divider rule |
| Right pane | fluid | Conversation or user profile; both occupy the same 1120 × 836 area |
| Top bar | 64px | Logo, LAN pill, daemon status, circle actions |
| Conversation header | 88px | Monogram, display name, presence, actions only |
| Profile header | 60px back row + identity block | Back arrow returns to conversation |
| Composer | auto | Rounded card, 20px margin |
| Requests | 360px cards | Top-right, stacked, in-app or desktop overlay |
| Player | 480px | Bottom-right, above message toasts |
| Message toasts | 420px | Bottom-right, max three |

Profile replaces the right pane, not the whole window. Users rail and top bar stay. Clicking the conversation header opens profile; back returns to chat; clicking another user switches conversation.

## Users

### Rail user card

64px, `--radius-m`, 32px monogram, name, preview, time, optional unread pill, 6–7px presence dot.

States: rest, hover (`--surface`), selected (`--ink`, `--base`, square full-width), focus, unread, muted (hollow dot), blocked (strike), offline, address changed (amber dot after name), connecting (45% dot).

No IP, MAC, OS, display server, or fingerprint in the rail.

### Profile pane

Only location for user details. Single scrollable content, two-column cards after the identity block.

- Header: monogram 64, display name, presence, “Stable identity · B3:71F0…9A2C”, actions Message / Call / Remote / Watch party.
- Identity: fingerprint, first seen, “Stable across IP changes”.
- Connection: status, current IP, previous IP with warning “Address changed 2 h ago”, MAC or “Not exposed”, connected since, last seen, transport encrypted.
- Device: OS, display server, app and protocol versions.
- Activity: active call/remote/party, missed and declined calls, “None” state.
- Policies: Mute, Ignore, Block toggles; Clear local history; Block user danger action.

MAC is shown only when available; never infer or fabricate it. Address change is a warning, not a trust failure.

## Chat

- Header shows name and presence only.
- Messages: text, file, voice, YouTube card. No call or remote panels.
- 1-to-1 messages have no avatar or sender name. Incoming blocks align left; outgoing blocks align right.
- Block order: message text first, then attachments/media (file, voice, YouTube card, watch party), then a bottom meta row with time. Outgoing adds the receipt after the time.
- Bottom meta is the only place for time and receipts; never put them inside attachment or video cards.
- Read receipts sit in the bottom meta row, outgoing only: single check = sent, double check = delivered, accent double check = read. Failed shows an error glyph with tap-to-retry; no status text labels.
- System lines only for missed or declined calls: centered between two rules, non-interactive, 12px muted, phone glyph.
- Composer detects YouTube URLs pre-send with a removable chip.
- Empty conversation explains the next action.

## Requests and active sessions

Floating cards, top-right, 360px, `--radius-l`, `--shadow-soft`. No user details beyond name and monogram.

- Incoming call: name, “Incoming call”, Decline / Accept.
- Remote desktop: name, “Wants to view and control this screen”, Decline / Accept. No PIN. Accept/decline is the full consent.
- Watch party invite: name, video title, Decline / Join.
- DND: suppressed card, no interruption.
- Active call: timer, Mute, End; muted and reconnecting states.
- Active remote: red dot, “Sharing screen”, timer, End session; indicator stays visible.
- Party minimized: pill with play glyph, “Watch party · 3 watching”, Open.

Desktop context when the window is closed uses the same cards top-right over `--backdrop`. In-app cards never dim the chat.

## YouTube

- Link message: thumbnail block, title, channel, duration, “Playing in overlay”, queue count, Watch party action.
- Player: bottom-right 480px, autoplay, queue, dismiss, ignore-incoming toggle, play/pause, mute, seek, title/channel.
- Dismiss is one-off. Ignore incoming persists until reactivated and is mirrored in Settings > Media.
- States: loading, playing, paused, buffering, queued, unavailable, embed blocked, ignored-all, minimized.
- No video is inserted into the message stream as an embed.

## Watch party

Dedicated in-app view: video stage left, participants and queue right. Anyone can play, pause, and seek; current controller is labeled.

States: invite, participating, controlling, buffering, guest left, host left, sync ok, resyncing, ended. Leave party is an error-outline action. Closing the view minimizes to the active-session stack.

## Settings

Sections: Notifications, Calls, Remote desktop, Media, Storage, About. No pairing or trust section.
Media contains autoplay, queue limit, player position, ignore incoming videos, dismiss behavior, watch-party control policy and invite acceptance.

## Message toasts

Bottom-right, max three, stack above the video player. Call, remote, and party request cards stay top-right so surfaces never collide.

## Accessibility

- Ink on base 12.0:1; ink on surface 10.4:1; ink-2 8.0:1; ink-3 6.0:1 base and 5.2:1 surface.
- Status text: error 5.6:1, success 5.7:1, info 6.6:1, warning 7.4:1.
- Focus: 2px accent ring at 2px offset; user cards use a 2px accent border.
- Autoplay: video starts muted if audio autoplay is not permitted; provide visible unmute.
- Ignore-incoming toggle must be reachable from the player and Settings.
- Requests always name the action; never color-only.
- Targets 32px minimum; 8px between adjacent actions.
- Motion respects `prefers-reduced-motion`.

## Anti-patterns

Never in this system:

- “Peer”, “pair”, “PIN”, “SPAKE2”, “trusted”, “pinned”, “Open LAN”.
- IP, MAC, OS, display server, or fingerprint outside the profile pane.
- Call or remote cards inside the message stream.
- PIN slots or pairing dialogs.
- Sharp corners outside the selected user card, mixed off-scale radii, square avatars.
- Chat bubbles, photo avatars, gradients, glass, glows.
- Monospace fonts; `currentColor` icons; emoji icons.
- Shadows on flat rows or the full-bleed shell.
- Color-only status.
- Text labels for delivery receipts (`Delivered`, `Read`, `Not delivered · Retry`).

## Paper implementation

| Page | Artboards |
|---|---|
| Screens | `01 — Chat`, `02 — User profile`, `03 — Settings`, `04 — Watch party` |
| Overlays | `05 — In-app requests`, `06 — In-app active`, `07 — Desktop requests`, `08 — YouTube player`, `09 — Message toasts` |
| Components | Masters and every state |

Paper MCP has no linked instances; local copies must follow the Components page.

## Review checklist

- [ ] No pairing vocabulary anywhere
- [ ] No user details outside the profile pane
- [ ] Header opens profile; back returns; rail stays visible
- [ ] No call/remote cards as messages; system lines only for missed/declined calls
- [ ] Selected user card is cream, square, full rail width
- [ ] Requests top-right; player and toasts bottom-right
- [ ] YouTube dismiss and ignore states present
- [ ] Watch party shared control and sync states present
- [ ] All component states exist
- [ ] Radii, spacing, type, and color come from tokens
- [ ] Contrast and focus verified
