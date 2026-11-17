# TiConnect Style Guide

**Direction: Industrial Signal Desk.** Switch-room faceplate meets Swiss instrument labeling. Dark only for now. Every value in the interface comes from the token block below.

## Brief

| | |
|---|---|
| Purpose | Desktop client for a LAN-only messaging, voice-call, and remote-desktop daemon |
| Audience | Linux users on a trusted office or home LAN, window open all day |
| Tone | Industrial Signal Desk: patch panel, instrument faceplate, no decoration |
| Reference | 1970s patch panels, Braun/Telefunken control faces, network switch faceplates |
| Memorable | Peer list drawn as a patch panel: numbered ports, square status lamps, selected port inverts to bone with an orange tab |
| Restraint | No bubbles, no avatars, no gradients, no rounded corners, no shadows inside the app window |

The single memorable moment is the patch-panel peer rail. Do not compete with it: everything else is typography, rules, and space.

## Tokens

```css
:root {
  --font-display: "Archivo";
  --font-text:    "Archivo";
  --font-mono:    "IBM Plex Mono";

  /* Type scale, 1.200 minor third, 16px base */
  --step-xs: 0.833rem;
  --step-0:  1rem;
  --step-1:  1.2rem;
  --step-2:  1.44rem;
  --step-3:  1.728rem;
  --step-4:  2.074rem;
  --step-5:  2.488rem;

  /* Space scale, 4px unit */
  --space-3xs: 4px;
  --space-2xs: 8px;
  --space-xs:  12px;
  --space-s:   16px;
  --space-m:   24px;
  --space-l:   32px;
  --space-xl:  48px;
  --space-2xl: 64px;
  --space-3xl: 96px;

  /* Color */
  --base:      #14161A;
  --surface:   #1D2024;
  --surface-2: #24282D;
  --ink:       #F1EFE9;
  --ink-2:     #A9ADB2;
  --ink-3:     #868B91;
  --line:      #2E3237;
  --line-strong: #3C4147;
  --accent:    #E4572E;
  --lamp:      #4FAE75;
  --warn:      #D9A441;
  --fault:     #C4453B;

  /* Form */
  --radius: 0;
  --border: 1px solid var(--line);

  /* Motion */
  --dur-fast: 120ms;
  --dur:      220ms;
  --ease:     cubic-bezier(0.2, 0, 0, 1);
}
```

### Token rules

- Every margin, padding, gap, width, and height uses a `--space-*` value. No `14px`, no `0.85rem`.
- Every font size uses a `--step-*` value. Type is chosen by role, not by eye.
- One accent (`--accent`). Under 10% of visible surface: send button, unread count, active port tab, focus ring, recording timer, destructive confirmation only.
- `--lamp`, `--warn`, `--fault` are semantic status only, never decoration. Lamps are 6px squares; status text stays ink.
- One radius: 0. No pills, no 4px softening. Mixed radii is forbidden.
- No shadows inside the app window. Only floating overlay toast cards may use two shadows:
  `0 1px 2px rgb(0 0 0 / 0.35)` and `0 16px 40px rgb(0 0 0 / 0.45)`.
- Motion: 120ms for hover/press, 220ms for panels, `--ease` only. Respect `prefers-reduced-motion`: substitute instant state changes.

## Typography

Two families only. Archivo carries display and UI text; IBM Plex Mono carries machine data.

| Role | Family | Size | Weight | Width | Tracking | Color |
|---|---|---|---|---|---|---|
| Wordmark | Archivo | `--step-1` | 800 | 75 | 0.08em, uppercase | `--ink` |
| Artboard / view title | Archivo | `--step-3` | 800 | 75 | -0.01em | `--ink` |
| Peer name (selected header) | Archivo | `--step-3` | 800 | 75 | -0.01em | `--ink` |
| Section label | Archivo | `--step-xs` | 500 | 100 | 0.10em, uppercase | `--ink-3` |
| Message body | Archivo | `--step-0` | 400 | 100 | 0 | `--ink` |
| Peer preview | Archivo | `--step-0` | 400 | 100 | 0 | `--ink-2` |
| Inline emphasis | Archivo | `--step-0` | 600 | 100 | 0 | `--ink` |
| Button label | Archivo | `--step-xs` | 600 | 100 | 0.04em, uppercase | role-dependent |
| Port number | IBM Plex Mono | `--step-xs` | 500 | — | 0 | `--ink-3` |
| Timestamp | IBM Plex Mono | `--step-xs` | 400 | — | 0 | `--ink-3` |
| Fingerprint / hash | IBM Plex Mono | `--step-xs` | 400 | — | 0.02em | `--ink-2` |
| Status / count | IBM Plex Mono | `--step-xs` | 500 | — | 0.06em, uppercase | role-dependent |

Rules:

- Weight budget: 400, 500, 600, 800. No other weights.
- Body measure 60–75ch. Message column max 68ch.
- Line height: body 1.55, headings 1.15, display 1.05.
- Never center more than two lines.
- Never below 13px. Small text at 13px must be `--ink-3` or brighter.
- IBM Plex Mono is for ASCII machine data only: ports, times, hashes, counts, PINs, versions. Never for prose.

## Layout

Window: 1440 × 900. Minimum supported: 1024 × 720.

| Region | Size | Notes |
|---|---|---|
| Top bar | 56px tall | Wordmark, LAN-ONLY badge, node fingerprint, daemon status, window actions |
| Peer rail | 320px fixed | Patch panel. Border-right `--line`. Never collapsible below 280px |
| Conversation | fluid | Header 96px, message stream scrolls, composer 112px |
| Composer | 112px | Attach rail 40px on top, input row below |

Grid rules:

- All separation by rules and space. A panel border is 1px `--line`; a raised surface is `--surface` or `--surface-2`.
- Two separation tools only: doubled gap (`--space-m` between groups, `--space-xs` within) and 1px rules. No cards.
- Hover changes background to `--surface`; selection inverts to `--ink` background with `--base` text.
- Focus ring: 2px `--accent` outline, 2px offset. Never `outline: none`.
- Minimum hit target 32px; primary actions 40px; inline icon buttons 32px with 8px separation.

## Components and states

Every interactive element ships all states. Missing states are a defect, not an omission.

### Primary button (Send)

| State | Treatment |
|---|---|
| rest | `--accent` fill, `--base` label |
| hover | fill lightened 6%, 120ms |
| active | `translateY(1px)`, fill darkened 6% |
| focus-visible | 2px `--accent` outline, 2px offset |
| disabled | `--surface-2` fill, `--ink-3` label, no pointer events |
| loading (`aria-busy`) | label hidden, mono counter slot reserved, spinner 16px |

### Secondary button / header action (Call, Remote desktop, Mute)

| State | Treatment |
|---|---|
| rest | transparent, 1px `--line`, `--ink` label |
| hover | `--surface` fill |
| active | `translateY(1px)` |
| focus-visible | accent outline |
| disabled | `--ink-3` label, `--line` border, no hover |
| toggled on | `--ink` fill, `--base` label, orange 2px left tab |

### Icon button (Attach, Mic)

States: rest, hover, active, focus-visible, disabled, active-with-count (recording: orange 6px dot plus mono timer). Icons are 16px stroke 1.5 SVG. No emoji.

### Peer row (patch port)

| State | Treatment |
|---|---|
| rest | port number, 6px lamp, name, preview, timestamp |
| hover | `--surface` fill |
| focus-visible | inset accent outline |
| selected | `--ink` fill, `--base` text, 3px `--accent` port tab on left edge |
| unread | name weight 600, orange mono count right-aligned |
| muted | lamp hollow, `--ink-3` name, count in `--ink-3` |
| blocked | `--ink-3` name with strike, no preview |
| offline | lamp `--line-strong`, `--ink-3` preview, no timestamp |
| no message | preview reads `No messages yet` in `--ink-3` |

Lanes: port number fixed 24px, lamp slot fixed 16px, timestamp right fixed 52px, count fixed 24px. `flexShrink: 0` on every slot. Gap alone is never used to align columns.

### Lamps

6px square, no radius: online `--lamp`; connecting `--lamp` at 40% with 1.5s breathing (static in export); away `--warn`; offline `--line-strong`.

### Composer

States: rest, focus (2px orange rule under input), filled, drag-over (dashed 1px `--accent`, `--surface` fill), error (1px `--fault` rule plus mono error text), disabled daemon-offline (mono caption `Daemon offline`, input inert).

Placeholder: `Message <peer> · Shift+Enter for newline`. Enter sends.

### Attachment row

States: queued, uploading (mono percentage plus 1px progress rule), paused, complete (hash + size in mono), failed (fault-colored retry action). File name in `--ink`, metadata in `--ink-3`.

### Voice note row

States: idle, playing (waveform in `--ink`, progress in `--accent`), paused, sending, failed. Duration mono.

### Remote desktop panel

States: incoming request (peer name, fingerprint, accept/decline), PIN entry (six slots), active session (red dot plus `Sharing screen` mono label and `End session` ghost), declined, wrong-PIN error. Session state occupies full width of the message column with 1px `--line-strong` frame.

### Call strip

States: ringing, connecting, in-call (mono timer), muted (orange dot), ended. Strip is 56px, `--surface` background, 1px top rule.

### PIN slot

States: empty, focused (orange underline), filled, error (fault underline), success (lamp underline), disabled. Slot is 48 × 56, mono `--step-3`, 1px `--line`, no radius.

### Toggle

States: off (`--line-strong` track, `--ink-2` knob), on (`--accent` track, `--base` knob), hover, focus-visible, disabled. Track 36 × 20, knob 14px square. No pill radius.

### Badge

Variants: unread count (orange fill, `--base` text), muted count (`--surface-2`, `--ink-3`), LAN-ONLY (transparent, 1px `--line-strong`, mono, `--ink-2`), pinned-TLS verified (`--lamp` dot plus mono `PINNED`), warning (`--warn` dot plus mono label). No radius.

### Toast card (overlay)

States: incoming, hover-paused (progress rule halts, close affordance appears), queued (stack indicator `+2`), dismissed. 420px wide, `--surface-2` fill, 1px `--line-strong`, 3px orange left edge, two shadows allowed. Max three visible. Never takes keyboard focus.

### Banner

Variants: daemon unavailable (fault left rule, mono reason, `Retry` action), no peers discovered (neutral, mono guidance, `Pair a device` action). Full window width under top bar, 40px tall.

### Pairing dialog

States: idle, scanning (mono progress), peer found (name, fingerprint, confirm), code entry (six PIN slots), mismatch error, success. Width 520px, `--surface`, 1px `--line-strong`, no radius, scrim `rgb(0 0 0 / 0.6)`.

## Content voice

- Labels are plain and operational: `Pair a device`, `Remote desktop`, `Mute`, `Retry`.
- Sentence case for prose, uppercase only for mono machine labels and button labels.
- Machine facts are always shown as machine facts: node fingerprint, pinned TLS, message hash, transfer size.
- No exclamation marks. No exclamation of success beyond a lamp and a mono status.
- Empty states explain the next action in one line.
- Never expose internal IDs as decoration; peer fingerprints are legitimate identity data and stay.
- Placeholder content uses realistic LAN names and message text, never lorem ipsum, never Figma or Sketch references.

## Accessibility

- Body text contrast: `--ink` on `--base` is 15.9:1, `--ink` on `--surface` is 14.2:1. `--ink-2` on `--base` is 7.4:1. `--ink-3` on `--base` is 5.3:1 and on `--surface` is 4.7:1; still limited to 13px secondary metadata, never body copy.
- `--base` text on `--accent` fill is 5.5:1; never use `--ink` on `--accent` for small text.
- Focus is always visible and never suppressed. Dialogs trap focus and restore it on close.
- Every icon-only button has an accessible name. Status is never conveyed by color alone: lamps pair with mono text in detail views.
- Keyboard: Enter sends, Shift+Enter newline, Ctrl+, settings, Ctrl+N pair, Ctrl+1 peers, Ctrl+2 conversation, Ctrl+Q quit, Escape closes dialog.
- Motion respects `prefers-reduced-motion`.
- Touch targets >= 32px; spacing between adjacent actions >= 8px.

## Anti-patterns

Never in this system:

- Chat bubbles, avatars, gradients, glass, glows, rounded cards, pill buttons.
- Emoji as icons.
- More than one accent hue for decoration.
- Shadows inside the app window.
- Centered marketing-style layouts, hero sections, feature card grids.
- Low-contrast gray body text.
- Arbitrary spacing or type values outside the token block.
- Mixed radii or mixed border widths.
- Color-only status.

## Paper implementation

File: `TiConnect` in Paper. Design tokens mirror the block above, named without extra namespaces: `--base`, `--space-s`, `--step-0`, `--step-xs`, `--font-display`, and so on. `--step-xs` is the 13.3px step (Paper token names cannot contain a double hyphen).

| Page | Artboard | Purpose |
|---|---|---|
| Page 1 | `01 — Desktop chat` (1440 x 900) | Main window: top bar, patch-panel peer rail, conversation, composer |
| Page 1 | `02 — Overlay toasts` (520 x 640) | Bottom-right toast stack, hover-paused and queued states |
| Page 1 | `03 — Pair a device` (1440 x 900) | Scrimmed window with 520px SPAKE2 pairing dialog |
| Components | `Components — masters and states` | Single source for every component and its states |

The components page is the master reference. Paper MCP cannot create linked component instances, so screens reuse the same markup and tokens; local copies must be updated from the components page when a component changes.

## Review checklist

- [ ] Every spacing value comes from `--space-*`
- [ ] Every font size comes from `--step-*`
- [ ] Type scale is 1.200 throughout
- [ ] One accent, under 10% of surface
- [ ] Radius 0 everywhere
- [ ] Every interactive component shows all required states
- [ ] Peer rail lanes align across all rows
- [ ] The patch-panel rail is still the loudest element
- [ ] Restraint line respected: no bubbles, avatars, gradients, shadows in-window
- [ ] Focus visible on every control
- [ ] Contrast checked for all text roles
- [ ] Verified at 1440px and 1024px width
