# Baibo Frontend UI Constraints

## 1. Status and intent

This document is the normative visual and interaction contract for Baibo's
frontend. New UI work **must** follow the requirements marked MUST or MUST NOT.
Existing checkpoint prototypes may be migrated incrementally, but a touched
component must not introduce a new violation.

The target is a desktop developer tool whose main content is a real terminal.
The visual keywords are:

- **Terminal-inspired**: the application chrome shares the terminal's density,
  typography, status language, and keyboard-first behavior.
- **TUI-like**: regions have explicit boundaries, titles, focus, and state. The
  UI should feel operable without a pointer.
- **ASCII art**: text graphics are a restrained identity device, not a substitute
  for semantic controls or data.
- **Retro**: the product borrows the economy and tactility of older terminals,
  not their display defects.

The result should read as a capable contemporary tool with a terminal lineage,
not an arcade game, cyberpunk poster, or simulated CRT.

## 2. Design principles

### 2.1 Terminal is content, not decoration

- The PTY viewport MUST receive the largest and clearest content region.
- Terminal output MUST be rendered by a terminal emulator. It MUST NOT be
  restyled as arbitrary HTML or placed behind decorative overlays.
- Application chrome MUST remain visually quieter than ANSI output. One
  application accent color is the default; additional colors are reserved for
  semantic states.
- Scanlines, noise, bloom, chromatic aberration, perspective distortion, and
  always-on flicker MUST NOT cover terminal content.

### 2.2 Structure before ornament

- Every screen MUST make the active workspace, active session, process state,
  focused region, and available primary action discoverable.
- Borders, spacing, labels, and alignment SHOULD create hierarchy before
  shadows, gradients, or color blocks.
- Repeated regions SHOULD use a TUI anatomy: optional eyebrow, concise title,
  body, and optional status/action line.
- A panel boundary MUST communicate grouping or focus. Decorative nested boxes
  are prohibited.

### 2.3 Keyboard first, pointer complete

- Every action available to the pointer MUST also be reachable by keyboard.
- Tab order MUST follow the visible reading order. Arrow-key navigation SHOULD
  be used inside composite widgets such as tabs, menus, lists, and trees.
- Shortcuts MUST NOT override established terminal or operating-system
  shortcuts while the terminal has focus. Global application shortcuts MUST be
  documented and expose their key hint near the action or in a shortcut view.
- Focus changes MUST be visible and MUST NOT unexpectedly execute an action or
  alter a value.

### 2.4 Retro through constraints

Retro character comes from a limited palette, monospaced labels, crisp
one-pixel rules, compact spacing, direct status copy, and immediate feedback.
It MUST NOT depend on illegible pixel fonts, fake phosphor glow, or continuous
animation.

## 3. Visual system

### 3.1 Color tokens

Components MUST consume semantic tokens rather than raw color literals. These
values define the default dark theme and may later be remapped by a theme:

```css
:root {
	--ui-canvas: #0c0f14;
	--ui-surface: #11151b;
	--ui-surface-raised: #171d25;
	--ui-border: #2f3947;
	--ui-border-strong: #5d6879;

	--ui-text: #e8edf5;
	--ui-text-secondary: #a7b0bd;
	--ui-text-muted: #7f8b9d;

	--ui-accent: #80aee8;
	--ui-success: #6fd3a7;
	--ui-warning: #f0b866;
	--ui-danger: #f07f78;

	--ui-focus: #80aee8;
	--ui-selection: #253d5a;
}
```

Rules:

- Normal text MUST meet a contrast ratio of at least 4.5:1. Large text and
  meaningful UI boundaries MUST meet at least 3:1.
- `--ui-border` is for passive separation only. Interactive boundaries and
  focus indicators MUST use `--ui-border-strong` or a higher-contrast token.
- Color MUST NOT be the only state signal. Pair it with text, an icon, a border
  pattern, or a position change. Examples: `[RUNNING]`, `! FAILED`, `● online`.
- Accent color is for selection, focus, and primary action. Success green MUST
  not be used as the general brand/action color.
- ANSI colors inside the terminal belong to the terminal theme and MUST remain
  separate from application UI tokens.

### 3.2 Typography

- Application chrome MUST use a legible monospace stack:
  `ui-monospace, "SFMono-Regular", Menlo, Monaco, Consolas, monospace`.
- Terminal font settings MUST be independent from application typography and
  MUST preserve character-cell alignment and Powerline/box-drawing glyphs.
- Chinese and mixed-language text MUST have a tested fallback. Do not assume a
  Latin-only pixel font can render product copy.
- Pixel/display fonts MAY appear in a wordmark or short marketing heading only.
  They MUST NOT be used for terminal output, body copy, controls, tables, or
  status text.
- Default application text SHOULD be 13–14 px with a line height of 1.4–1.6.
  Terminal font size MUST be user-configurable and MUST NOT default below 12 px.
- Labels SHOULD use sentence case. Uppercase is reserved for short machine-like
  state labels and eyebrows, with tracking no greater than `0.08em`.
- Lining/tabular numerals SHOULD be used for durations, counts, line numbers,
  and resource usage.

### 3.3 Grid, spacing, and shape

- Layout MUST use a 4 px base grid. Preferred spacing steps are 4, 8, 12, 16,
  24, and 32 px.
- Dense controls SHOULD be 28–32 px high. Primary controls MUST provide at
  least a 36 px pointer target; tightly packed smaller targets need an expanded
  hit area that does not overlap adjacent actions.
- Panel padding SHOULD be 12 or 16 px. Repeated list rows SHOULD use 8–12 px
  vertical padding.
- Application chrome SHOULD use 0–4 px corner radii. The terminal viewport
  SHOULD be square within its panel. Pill shapes are reserved for filters or
  compact statuses, not general buttons.
- Shadows SHOULD be absent. A raised transient layer MAY use one restrained,
  hard-edged shadow plus a strong border.
- Desktop layouts MUST work without clipped primary actions at 960×640,
  1280×720, and 1440×900. Below the comfortable width, secondary panels SHOULD
  collapse, become tabs, or open as overlays; the terminal MUST NOT be squeezed
  below a usable width merely to preserve a multi-column layout.

### 3.4 Borders and ASCII language

- CSS borders are preferred for layout because they resize and expose semantics
  correctly. Use a consistent one-pixel solid rule.
- Box-drawing characters (`─ │ ┌ ┐ └ ┘ ├ ┤`) MAY appear in fixed, decorative
  text compositions but MUST NOT be manually repeated to form responsive panel
  borders.
- ASCII art is allowed for the brand mark, an empty state, onboarding, or an
  easter egg. It MUST be short, optional, and no more visually prominent than
  the task at hand.
- Decorative ASCII MUST use `aria-hidden="true"` and have adjacent semantic
  text. Information-bearing diagrams MUST have a plain-text equivalent.
- ASCII symbols MUST NOT replace familiar control labels or accessible names.
  For example, `[x]` may reinforce “Stop session” but cannot be the button's
  only label.
- Use one symbol vocabulary consistently:
  `>` prompt/action, `+` create, `×` close, `…` pending, `!` warning,
  `●` live, and `○` inactive. Do not use emoji as interface icons.

## 4. Components and states

### 4.1 Panels and navigation

- The focused panel MUST have a stronger boundary or title treatment that
  remains visible without color perception.
- The selected navigation item MUST differ by at least two cues, such as
  background plus a leading marker.
- Panel titles MUST remain plain text in the accessibility tree even when
  visually wrapped in TUI punctuation such as `┤ SESSIONS ├`.
- Resizable split panes MUST expose a keyboard-operable separator and sensible
  minimum sizes.

### 4.2 Buttons, links, and menus

- Use semantic `button`, `a`, `input`, `select`, and dialog elements rather than
  clickable `div` elements.
- Every interactive state MUST be designed: default, hover, active, focus,
  disabled, loading, and destructive where applicable.
- Hover feedback MUST NOT move layout. Transitions SHOULD last 120–200 ms.
- Destructive actions MUST include a word label and confirmation proportional
  to reversibility. Red alone is insufficient.
- Icon-only buttons MUST have an accessible name and visible tooltip.

### 4.3 Status and feedback

- Session and process status MUST be sourced from structured application state,
  never inferred solely from rendered terminal text.
- Status labels SHOULD use concise present-tense copy such as `STARTING`,
  `RUNNING`, `EXITED 0`, `FAILED`, and `STOPPED`.
- Dynamic status updates outside the terminal SHOULD use an appropriate
  `aria-live` region. Routine streaming terminal output SHOULD NOT trigger
  duplicate application announcements.
- Operations longer than 300 ms SHOULD acknowledge input. Operations longer
  than 1 s SHOULD show a determinate value when known or a restrained
  indeterminate state.
- Error messages MUST state what failed and the next available action. Preserve
  relevant command, exit-code, or log evidence when safe.

### 4.4 Terminal integration

- Terminal focus MUST be visually distinct from application chrome focus.
- App shortcuts that capture keystrokes while terminal focus is active MUST be
  limited, documented, and tested against shells, editors, tmux, and supported
  agent TUIs.
- Selection, copy, paste, context menu, scrollback, search, resize, and screen
  reader behavior MUST be tested in the embedded terminal.
- Terminal implementation SHOULD expose a user-controlled screen-reader mode
  and minimum contrast behavior when supported by the emulator.
- Terminal overlays MUST not intercept selection, mouse reporting, or IME input.
- Resizing MUST update PTY columns and rows from the actual character-cell
  geometry, not from a CSS breakpoint estimate.

## 5. Motion and effects

- Motion MUST explain a state change, focus change, panel transition, or
  progress. Decorative ambient animation is not allowed in the workspace UI.
- At most one repeating animation SHOULD be visible in a region. A terminal
  cursor owned by the emulator does not need a second simulated cursor.
- Blink frequency MUST remain conservative, and rapidly flashing content is
  prohibited.
- `prefers-reduced-motion: reduce` MUST disable non-essential motion and replace
  it with an immediate state change.
- Glows MAY be used as a subtle reinforcement for a live/focused state, but the
  underlying state MUST remain clear when shadows are disabled.

## 6. Accessibility and internationalization

- Target WCAG 2.2 AA for application chrome.
- Keyboard focus MUST always be visible. The preferred focus treatment is a
  two-pixel outline using `--ui-focus` with an offset that does not alter layout.
- UI text and meaningful controls MUST remain usable at 200% text zoom.
- Controls MUST have programmatic names, form fields MUST have labels, and
  validation messages MUST be associated with their fields.
- Do not encode meaning only through glyph choice: provide a text label for
  status, severity, provider, and branch/worktree state.
- Copy MUST tolerate expansion and mixed Chinese/English paths without
  overlapping controls. Truncation requires a way to reveal and copy the full
  value.
- Respect operating-system settings for reduced motion, contrast where
  available, and input modality.

## 7. Implementation contract

- Global tokens and reset rules belong in `src/app.css`; page-specific files
  MUST not redefine the global palette.
- Reusable interactive patterns belong in `src/lib/components/` once a second
  use appears. Variants SHOULD be explicit props or data attributes rather than
  selector overrides tied to one page.
- Svelte templates MUST use semantic HTML. Dynamic status exposed outside the
  terminal MUST include appropriate ARIA state or live-region behavior.
- Raw values are allowed only when defining tokens, terminal-owned themes, or a
  documented one-off data visualization.
- Native title-bar and traffic-light safe areas MUST be tested on macOS before a
  custom window chrome treatment is accepted.

## 8. Review checklist

A frontend change is ready only when the relevant answers are yes:

- Does the terminal remain the dominant, unobstructed content surface?
- Can every action be completed with a keyboard, with visible focus throughout?
- Are active workspace, session, process state, and focused region unambiguous?
- Are colors tokenized, contrast-safe, and reinforced by a non-color cue?
- Does typography preserve terminal cell alignment and mixed-language legibility?
- Is ASCII art semantic-safe, restrained, and absent from control-only labels?
- Are loading, empty, error, disabled, focus, and destructive states covered?
- Does the layout remain usable at the three target window sizes?
- Does reduced-motion mode remove non-essential animation?
- Have terminal selection, paste, IME, resize, and shortcut conflicts been tested?

## 9. Research basis

These constraints synthesize the project's product requirements with:

- [W3C WCAG 2.2: Focus Appearance](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html)
  for visible focus size and contrast.
- [IBM Carbon: Accessibility and color](https://carbondesignsystem.com/guidelines/accessibility/color/)
  for role-based color, text contrast, component-boundary contrast, and
  non-color state cues.
- [IBM Carbon: Typography](https://carbondesignsystem.com/elements/typography/overview/)
  for neutral running text and purposeful use of color and monospace.
- [Microsoft keyboard UI design guidance](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/dnacc/guidelines-for-keyboard-user-interface-design)
  for complete keyboard access, logical navigation order, documented shortcuts,
  and predictable focus behavior.
- [xterm.js terminal options](https://xtermjs.org/docs/api/terminal/interfaces/iterminaloptions/)
  for terminal-specific screen-reader and minimum-contrast capabilities.
