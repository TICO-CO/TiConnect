# TiConnect Style Guide

**Direction: Quiet Grove.** A night-garden console: warm cream ink on deep teal, rounded surfaces, circular presence marks, soft green-tinted depth. Dark only for now. Every value in the interface comes from the token block below.

## Brief

| | |
|---|---|
| Purpose | Desktop client for a LAN-only messaging, voice-call, and remote-desktop daemon |
| Audience | Linux users on a trusted office or home LAN, window open all day |
| Tone | Quiet Grove: botanical field guide, night desk, soft utility |
| Reference | 1970s botanical field guide printed on dark stock; rounded instrument panels |
| Memorable | The selected peer card turns cream with a circular monogram and its presence dot goes quiet |
| Restraint | No chat bubbles, no photo avatars, no gradients, no sharp corners, no hairline grid, no patch-panel numbers |

The memorable moment is the cream selected peer card. Everything else stays quiet: type, spacing, and soft surfaces carry the hierarchy.

## Tokens

```css
:root {
  --font-display: "Konkhmer Sleokchher";
  --font-text:    "Istok Web";

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
  --base:      #182B2F;
  --surface:   #213639;
  --surface-2: #2A403C;
  --rail:      #1B3034;
  --backdrop:  #0F1F22;
  --ink:       #FFE5B0;
  --ink-2:     #EDB483;
  --ink-3:     #A9A78A;
  --line:      #2F4547;
  --line-strong: #3F5A52;
  --accent:    #EDB483;

  /* Semantic status */
  --error:     #E08A66;
  --success:   #7FAE63;
  --info:      #8AB4C4;
  --warning:   #D9B26A;
  --lamp:      #577E46;
  --fault:     var(--error);
  --warn:      var(--warning);

  /* Radius */
  --radius-s:    10px;   /* inputs, search, PIN slots, tags */
  --radius-m:    14px;   /* peer cards, composer, banners, panels */
  --radius-l:    20px;   /* window shell, dialogs, toasts */
  --radius-pill: 999px;  /* buttons, badges, toggles, monograms */

  /* Depth */
  --shadow-soft: 0 1px 2px rgba(6, 18, 18, 0.30),
                 0 14px 32px rgba(6, 18, 18, 0.28);

  /* Motion */
  --dur-fast: 120ms;
  --dur:      220ms;
  --ease-soft: cubic-bezier(0.22, 1, 0.36, 1);
}
```

### Token rules

- Every margin, padding, gap, width, and height uses a `--space-*` value.
- Every font size uses a `--step-*` value.
- Exactly one interactive accent, `--accent`, under 10% of surface: Send, Allow, Confirm, focus ring, unread pill.
- One radius scale only: 10 / 14 / 20 / pill / circle. A rectangle never uses an off-scale radius.
- `--shadow-soft` appears only on floating layers: dialog, toasts, and raised attachment cards. The app shell, rows, and messages have no shadow.
- The app shell fills the artboard. `--backdrop` is only used behind overlay and modal artboards.
- Status colors are dots, rules, and 13px labels only.

## Typography

Two faces only. Konkhmer Sleokchher is display only and ships a single 400 weight; Istok Web carries UI text and machine data at 400 and 700. No monospace face.

| Role | Family | Size | Weight | Tracking | Color |
|---|---|---|---|---|---|
| Wordmark | Konkhmer Sleokchher | `--step-1` | 400 | 0 | `--ink` |
| View / peer heading | Konkhmer Sleokchher | `--step-3` | 400 | 0 | `--ink` |
| Dialog heading | Konkhmer Sleokchher | `--step-3` | 400 | 0 | `--ink` |
| Overlay heading | Konkhmer Sleokchher | `--step-2` | 400 | 0 | `--ink` |
| Section label | Istok Web | 12px | 700 | 0.08em uppercase | `--ink-3` |
| Peer name | Istok Web | 15px | 700 / 400 | 0 | `--ink` |
| Message body | Istok Web | `--step-0` | 400 | 0 | `--ink` |
| Secondary body | Istok Web | `--step-xs` | 400 | 0 | `--ink-2` |
| Button label | Istok Web | `--step-xs` | 700 | 0.01em, sentence case | role-dependent |
| Machine data | Istok Web | 12px | 400 | 0.04em, tabular figures | `--ink-3` |
| Machine label | Istok Web | 12px | 700 | 0.06em uppercase | role-dependent |
| PIN value | Istok Web | 23px | 700 | 0.04em, tabular figures | `--ink` |

Rules:

- Weight budget: Istok Web 400 and 700 only; Konkhmer Sleokchher 400 only.
- Konkhmer has one weight. Do not request synthetic bold or add letter-spacing to large display text.
- Controls are sentence case. Uppercase is reserved for section labels and machine labels.
- No condensed headings.
- Body measure 60–75ch; message column max 640px.
- Line height: body 1.55, headings 1.15, display 1.1.
- Never below 12px; 12px is reserved for machine labels and metadata.
- Numerals in timestamps, hashes, transfer sizes, and PINs use tabular figures.

## Layout

Window: 1440 × 900, full-bleed shell. Minimum supported: 1024 × 720.

| Region | Size | Notes |
|---|---|---|
| Shell | 1440 × 900 | Fills the artboard edge to edge; no outer border, inset, or shadow |
| Top bar | 64px | Wordmark, LAN pill, daemon status, circle actions |
| Peer rail | 320px | `--rail` ground, 12px inset, 6px card gaps, no divider rule |
| Conversation header | 88px | 44px monogram, display name, two pill actions |
| Message stream | fluid | 28px side padding, 18px message gaps |
| Composer | auto (~100px) | Floating card, 20px margin, `--radius-m` |

Rules:

- Separation by tint and space, not rules. The rail sits on `--rail`; cards use `--surface` on hover; the selected card uses `--ink` with `--base` text, runs edge to edge across the rail, and is the one intentional zero-radius rectangle.
- Selection is a filled card, not a side tab or outline.
- Controls use a 2px `--accent` focus ring at 2px offset; peer cards use a 2px accent border. Never suppressed.
- Minimum hit target 32px; primary actions 36–40px; icon buttons 36px circles.
- Presence dot 7px circle; muted is hollow, connecting is 45% opacity, offline is `--line-strong`.

## Components and states

Every interactive element ships all states. Missing states are a defect.

### Primary button (Send, Allow, Confirm)

Pill, 40px tall, `--accent` fill, `--base` label.

| State | Treatment |
|---|---|
| rest | accent fill, base label |
| hover | `#F2C89E` fill |
| active | `#D9A06A` fill, `translateY(1px)` |
| focus-visible | accent fill, 2px accent outline at 2px offset |
| disabled | `--surface-2` fill, `--ink-3` label, `--line` border |
| loading | `#33433B` fill, accent label, 6px accent dot, layout reserved |

### Secondary button (Call, Remote desktop, Cancel, Retry)

Pill, 40px, transparent fill, 1px `--line`, `--ink` label.

States: rest, hover (`--surface`), active (`--surface-2`, 1px down), focus-visible, disabled (`--ink-3`), toggled (`--ink` fill, `--base` label). Destructive outline uses `--error` text and border, never a red fill.

### Icon button

36px circle, 1px `--line`, 16px icon. States: rest, hover (`--surface`), active (`--surface-2`, 1px down), focus-visible, disabled, recording (pill with 6px accent dot and timer).

### Peer card

68px tall, `--radius-m`, 32px monogram, name, preview, time, optional unread pill. 6px gap between cards.

| State | Treatment |
|---|---|
| rest | transparent fill, ink name, ink-2 preview |
| hover | `--surface` fill |
| selected | `--ink` fill, `--base` name, `rgba(24,43,47,.62)` preview, dark unread pill; spans the full rail width with square corners |
| focus-visible | transparent fill, 2px accent border |
| unread | name weight 600, accent unread pill |
| muted | hollow presence dot, ink-3 name and preview |
| blocked | ink-3 name with strike, no preview emphasis |
| offline | `--line-strong` dot, ink-3 text |
| no message | preview `No messages yet` in ink-3 |
| connecting | 45% success dot, `Discovering on …` preview |

Monogram: 32px circle, `--surface-2`, ink-2 initials; selected monogram uses `rgba(24,43,47,.12)` with `--base` initials. No photos.

### Presence dot / lamp

7px circle: online `--lamp`, connecting `--lamp` at 45%, away `--warning`, offline `--line-strong`, muted hollow with 1.5px `--ink-3`, sharing `--error`. Never the only signal; pair with a data label in detail views.

### Badge

Pill, 22px tall, 10px horizontal padding. Variants: unread (accent fill, base text), muted count (`--surface-2`, ink-3, line border), LAN-only and Pinned (dot plus data label, transparent), warning and info (dot plus data label). No radius exceptions.

### Composer

`--radius-m` card on `--surface`, 1px `--line`, 14px padding.

| State | Treatment |
|---|---|
| rest | line border, placeholder ink-3, disabled Send |
| focus | accent border, caret visible |
| filled | ink text, accent Send enabled |
| drag-over | dashed accent border, `--surface-2` fill, accent hint |
| error | error border, 12px error label |
| disabled | 50% opacity, `Daemon offline` label |

Enter sends; Shift+Enter inserts newline. Send stays 36px tall and reserved in every state.

### Attachment card

`--radius-m`, `--surface`, 1px `--line`, 12px padding, 34px icon tile with `--radius-s`. States: queued, uploading (3px accent progress), paused, complete (success dot plus hash), failed (error meta plus Retry pill).

### Voice note

`--radius-m` card, 34px circular play button, waveform bars are 3px pills, progress in `--accent`. States: idle, playing, paused, sending, failed (error label plus Retry).

### Call strip

`--radius-m` card, 60px tall. States: ringing (success dot, accept/decline circles), connecting, in-call (success dot plus timer), muted (error dot), ended (ink-3). Destructive End is an error-outline pill.

### Remote desktop panel

`--radius-m`, 1px `--line`, 16px padding. States: incoming request, PIN entry (10px slots), active session (error dot plus `Sharing screen`), declined. Consent copy stays visible.

### PIN slot

48 × 56, `--radius-s`, `--surface`, 23px bold numerals. States: empty, focused (2px accent border), filled, error (error border), success (success border), disabled (45% opacity).

### Toggle

40 × 22 pill track, 16px circle knob. States: off (`--line-strong` / ink-2), on (accent / base), hover, focus-visible, disabled. No square knobs.

### Toast card

420px wide, `--radius-l`, `--surface-2`, 1px `--line-strong`, `--shadow-soft`, 14–16px padding. States: incoming, hover-paused (close circle plus 38% progress), queued (55% opacity, queued note), dismissed (35% opacity). Max three visible; never takes keyboard focus.

### Banner

Full width, `--radius-m`, `--surface`, dot plus label and detail, trailing pill action. Variants: info, error, warning.

### Pairing dialog

520px, `--radius-l`, `--surface`, `--shadow-soft`, 28px padding, scrim `rgba(0,0,0,.55)`. States: scanning, peer found with code entry, mismatch error, success. PIN slots follow the PIN component. Buttons are pills.

## Semantic status palette

Four roles. Each is a 7px dot, a 1–3px rule, or a 13px data label. Never a large fill, never a button background, never the only signal.

| Role | Token | Value | Used for | Contrast on `--base` / `--surface` |
|---|---|---|---|---|
| Error | `--error` (`--fault`) | `#E08A66` | Failed transfer, wrong PIN, declined session, daemon unavailable | 5.6:1 / 4.8:1 |
| Success | `--success` | `#7FAE63` | Delivered, paired, in call, connected | 5.7:1 / 4.9:1 |
| Info | `--info` | `#8AB4C4` | Discovery active, informational banners | 6.6:1 / 5.7:1 |
| Warning | `--warning` (`--warn`) | `#D9B26A` | Key rotated, away, no peers | 7.4:1 / 6.4:1 |

- `--lamp` `#577E46` is the online dot only; success text uses `--success`.
- Success text on `--surface-2` drops to 4.3:1; use `--ink` there.
- `--info` is status, not interaction.
- Pair every status with a data label.

## Content voice

- Labels are plain and operational: `Pair a device`, `Remote desktop`, `Decline`, `Allow`, `Retry`.
- Sentence case for controls and prose; uppercase only for section and machine labels.
- Machine facts stay machine facts: fingerprint, pinned TLS, hash, transfer size.
- No exclamation marks. Success is a dot and a data label.
- Empty states explain the next action in one line.
- Realistic LAN names and message text, never lorem ipsum, never Figma or Sketch references.

## Accessibility

- Body text contrast: `--ink` on `--base` 12.0:1; `--ink` on `--surface` 10.4:1. `--ink-2` on `--base` 8.0:1. `--ink-3` on `--base` 6.0:1 and on `--surface` 5.2:1; 13px metadata only.
- Status text: error 5.6:1, success 5.7:1, info 6.6:1, warning 7.4:1 on `--base`.
- Focus is always visible: 2px `--accent`, 2px offset. Dialogs trap and restore focus.
- Icon-only buttons have accessible names. Status is never color-only.
- Keyboard: Enter sends, Shift+Enter newline, Ctrl+, settings, Ctrl+N pair, Ctrl+1 peers, Ctrl+2 conversation, Ctrl+Q quit, Escape closes dialog.
- Motion respects `prefers-reduced-motion`.
- Targets 32px minimum; 8px separation between adjacent actions.

## Anti-patterns

Never in this system:

- Sharp 0px corners, mixed off-scale radii, or square avatars.
- Monospace or typewriter faces.
- Chat bubbles, photo avatars, gradients, glass, glows.
- Patch-panel port numbers or switchboard metaphors.
- Display weights other than 400, or synthetic bold.
- Square status lamps; metallic or industrial detailing.
- Shadows on flat message rows or on the full-bleed shell; shadows only on floating layers.
- Emoji as icons.
- `currentColor` on SVG icons; Paper resolves it to black. Set `stroke` explicitly.
- More than one interactive accent hue.
- Color-only status.
- Arbitrary spacing, type, or radius values outside the tokens.

## Paper implementation

File: `TiConnect` in Paper. Tokens mirror the block above without namespaces: `--base`, `--space-s`, `--step-0`, `--radius-m`, `--font-display`, and so on. `--step-xs` is the 13.3px step (Paper token names cannot contain a double hyphen).

| Page | Artboard | Purpose |
|---|---|---|
| Screens | `01 — Desktop chat` (1440 × 900) | Floating shell, peer rail, conversation, composer |
| Screens | `02 — Overlay toasts` (520 × 640) | Toast stack, hover-paused and queued states |
| Screens | `03 — Pair a device` (1440 × 900) | Scrimmed window with 520px SPAKE2 dialog |
| Components | `Components — masters and states` | Every master and every state |

Paper MCP cannot create linked component instances. Screens reuse the same markup and tokens; local copies must be updated from the components page when a master changes.

## Review checklist

- [ ] Every spacing value comes from `--space-*`
- [ ] Every font size comes from `--step-*`
- [ ] Type scale is 1.200 throughout
- [ ] Konkhmer Sleokchher is display only, weight 400; no synthetic bold
- [ ] Radii use only 10 / 14 / 20 / pill / circle
- [ ] One accent, under 10% of surface
- [ ] Shadows only on floating layers
- [ ] Every component shows all required states
- [ ] Selected peer card is the loudest element
- [ ] No patch numbers, square lamps, or sharp corners
- [ ] Focus visible on every control
- [ ] Contrast checked for all text roles
- [ ] Verified at 1440px and 1024px width
