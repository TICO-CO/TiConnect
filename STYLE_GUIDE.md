# TiConnect Style Guide

**Direction: Quiet Grove.** A botanical console with two lighting conditions: cream ink on deep teal at night, deep pine ink on bone paper by day. Rounded surfaces, circular presence marks, soft tinted depth. Dark is the default; light keeps the same semantic roles. Every value comes from the token block.

## Brief

| | |
|---|---|
| Purpose | Desktop client for a LAN-only messaging, voice, video, watch-party, and remote-desktop daemon |
| Audience | Linux users on a trusted office or home LAN, window open all day |
| Tone | Quiet Grove: botanical field guide, night desk, soft utility |
| Reference | 1970s botanical field guide on dark stock; rounded instrument panels |
| Memorable | The selected user card inverts to the strongest ink/base pair — cream on pine in dark, pine on bone in light — square-edged, with a circular monogram and a quiet presence dot |
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

:root[data-theme='light'] {
  --base: #F4EFE2; --surface: #FBF7EC; --surface-2: #EEE6D4;
  --rail: #E9E1CF; --backdrop: #D8CFBB;
  --ink: #24352F; --ink-2: #4F6256; --ink-3: #526458;
  --line: #D3C9B5; --line-strong: #B9AD95;
  --accent: #7A4A24;

  --error: #96402A; --success: #3F6A31; --info: #356474; --warning: #755612;
  --lamp: #658743; --fault: var(--error); --warn: var(--warning);

  --shadow-soft: 0 1px 2px rgba(73,59,35,0.12), 0 14px 32px rgba(73,59,35,0.14);
}
```

Rules:

- One interactive accent; under 10% of surface.
- Radii only 10 / 14 / 20 / pill / circle. Selected user card is the one intentional square, edge-to-edge.
- Shadows only on floating layers: request cards, active cards, player, toasts, dialogs.
- Status colors are dots, rules, and 13px labels only.
- All numbers in data labels use tabular figures.

## Appearance

`appearance.color_scheme` accepts `system`, `light`, or `dark`.

- `system` removes `data-theme` and follows `prefers-color-scheme`.
- `light` and `dark` set `data-theme` and pin the theme immediately.
- Both themes use the same semantic token names. Only the values change.
- The browser `theme-color` must follow the resolved theme, including live OS changes while `system` is selected.
- The Appearance control lives in Settings → User, above Presence.

## Typography

Konkhmer Sleokchher is display only, weight 400. Istok Web carries UI and data at 400/700. No monospace face.

| Role | Family | Size | Weight |
|---|---|---|---|
| Wordmark | Konkhmer | `--step-1` | 400 |
| View / user heading | Konkhmer | 23–27px | 400 |
| Settings / overlay heading | Konkhmer | `--step-2` | 400 |
| Section label | Istok Web | 12px uppercase, 0.08em | 700 |
| User name (rail) | Istok Web | 14px | 700 |
| Message body | Istok Web | `--step-0` | 400 |
| Body secondary | Istok Web | `--step-xs` | 400 |
| Button | Istok Web | `--step-xs` sentence case | 700 |
| Data label | Istok Web | 12px, tabular | 400/700 |
| Tabular numerals | Istok Web | 12–23px tabular | 700 |

Never request synthetic bold. Controls are sentence case; uppercase only for section and data labels.

## Layout

Window 1440 × 900, full-bleed. Minimum 1024 × 720.

| Region | Size | Notes |
|---|---|---|
| Users rail | 320px, collapsible to 72px | `--rail`, 12px inset, 6px gaps, no divider rule |
| Right pane | fluid | Conversation or user profile; both occupy the same 1120 × 836 area |
| Top bar | 64px | Logo, LAN pill, daemon status, circle actions |
| Conversation header | 88px | Monogram, display name, presence, actions only |
| Profile header | 60px back row + identity block | Back arrow returns to conversation |
| Composer | auto | Rounded card, 20px margin |
| Requests | 360px cards | Top-right, stacked, in-app or desktop overlay |
| Call view | fluid right pane | Replaces the conversation; monogram stage, timer, controls |
| Sharing host | 720px preview + 300px card | Monitor preview and session controls |
| Sharing viewer | fluid canvas | Shared desktop with floating controls |
| Player | 480px | Bottom-right, above message toasts |
| Message toasts | 420px | Bottom-right, max three |

Profile replaces the right pane, not the whole window. Users rail and top bar stay. Clicking the conversation header opens profile; back returns to chat; clicking another user switches conversation.

The users rail collapses from 320px to 72px with a chevron control at its top. Expanded: search, full user cards, and a square, edge-to-edge footer pinned to the bottom of the rail holding only the 34px logo and the no-background settings button. Collapsed: monogram-only tiles with presence dots, selected user inverted to the `--ink`/`--base` pair, logo and settings at the bottom, chat pane expands to fill the space.

## Users

### Rail user card

64px, `--radius-m`, 32px monogram, 14px name, preview, time, optional unread pill. Presence dot follows the name. Address changes are stated in the preview line, never as a second dot.

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

- Header shows name and presence plus Call / Remote / Watch party only. No settings button.
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

## Calls

Dedicated right-pane view replacing the conversation. Rail and top bar stay.

- Header: monogram, name, presence, call status, tabular timer.
- Stage: large monogram in quiet concentric rings, name, timer, connection line.
- Controls: Mute, Output, End call. End call uses the error outline. Active mute uses the accent.
- States: ringing, active, muted, reconnecting, ended. Reconnecting uses the warning dot; ended uses a system line and Call back.
- No video surface in v1.

## Sharing

Host and viewer both use the right pane; the rail stays visible. Consent is the full grant; no PIN is shown or entered.

- Host: 16:9 monitor preview, viewer identity, input permission, session log, Stop sharing. The indicator stays visible while sharing.
- Viewer: shared desktop canvas on `--backdrop`, live indicator, quality readout, pointer/keyboard/fullscreen controls, End session.
- Host states: starting, active, reconnecting, ended. Viewer states: waiting for consent, reconnecting, session ended, input unavailable.
- Capture and input start only after the host accepts. End session is a hard kill switch.

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

Sections: User, Notifications, Calls, Remote desktop, Media, Storage, About. No pairing or trust section.
User contains Profile card, Appearance, Presence, and Identity key.
Media contains autoplay, queue limit, player position, ignore incoming videos, dismiss behavior, watch-party control policy and invite acceptance.

## Message toasts

Bottom-right, max three, stack above the video player. Call, remote, and party request cards stay top-right so surfaces never collide.

## Accessibility

- Dark: ink on base 12.0:1; ink on surface 10.4:1; ink-2 8.0:1; ink-3 6.0:1 base and 5.2:1 surface.
- Light: ink on base 11.3:1; ink on surface 12.1:1; ink-2 5.7:1; ink-3 5.5:1 base and 5.9:1 surface.
- Status text: dark error 5.6:1, success 5.7:1, info 6.6:1, warning 7.4:1; light error 6.0:1, success 5.5:1, info 5.7:1, warning 5.9:1.
- Focus: 2px accent ring at 2px offset; user cards use a 2px accent border.
- Scrollbars are custom: 4px pill thumb, transparent track, `--line-strong` idle, `--ink-2` hover, `--accent` while dragging. Used on the message stream and profile pane; never the native scrollbar.
- Autoplay: video starts muted if audio autoplay is not permitted; provide visible unmute.
- Ignore-incoming toggle must be reachable from the player and Settings.
- Requests always name the action; never color-only.
- Targets 32px minimum; 8px between adjacent actions.
- Motion respects `prefers-reduced-motion`.

## Anti-patterns

Never in this system:

- “User”, “pair”, “PIN”, “SPAKE2”, “trusted”, “pinned”, “Open LAN”.
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
| Screens | `01 — Chat`, `01b — Chat · rail collapsed`, `02 — User profile`, `03 — Settings`, `03b — Settings · user`, `04 — Watch party`, `05 — Voice call`, `05b — Call states`, `06 — Sharing · host`, `06b — Sharing states`, `07 — Sharing · viewer` |
| Light mode | Complete light copies of every Screens and Overlays artboard |
| Overlays | `05 — In-app requests`, `06 — In-app active`, `07 — Desktop requests`, `08 — YouTube player`, `09 — Message toasts`, `10 — Desktop active`, `11 — Desktop active · states` |
| Components | Masters and every state; dark + light palette and token mapping |

Paper MCP has no linked instances; local copies must follow the Components page.

## Review checklist

- [ ] No pairing vocabulary anywhere
- [ ] No user details outside the profile pane
- [ ] Header opens profile; back returns; rail stays visible
- [ ] No call/remote cards as messages; system lines only for missed/declined calls
- [ ] Selected user card uses the inverted `--ink`/`--base` pair, square, full rail width
- [ ] Light mode has the same screens, overlays, states, and focus treatment as dark
- [ ] Appearance offers System / Light / Dark and System follows the OS live
- [ ] Requests top-right; player and toasts bottom-right
- [ ] YouTube dismiss and ignore states present
- [ ] Watch party shared control and sync states present
- [ ] Call and sharing screens include reconnecting, ended, and muted/error states
- [ ] Sharing never shows a PIN; accepting is the full consent
- [ ] All component states exist
- [ ] Radii, spacing, type, and color come from tokens
- [ ] Contrast and focus verified
