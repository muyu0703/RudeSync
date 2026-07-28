# Contributing to RudeSync

Thanks for taking a look. RudeSync is a single-user, offline-first desktop
tracker, and that shapes almost every decision in it — please read the
constraints below before proposing a change.

## Ground rules

**Offline-first is not negotiable.** Every feature must work with no network
access. Do not add a runtime dependency on a remote service, CDN, telemetry
endpoint, or font host. Assets are bundled; fonts are self-hosted.

**Money is stored as integer minor units.** Never use floating point for money.
Totals are grouped per currency and are never summed across currencies — a
`USD` balance and a `EUR` balance are two numbers, not one.

**SQLite is the source of truth.** All persisted rows carry a UUID primary key,
`created_at`, `updated_at`, and a nullable `deleted_at`. Deletion is soft; there
is no `DELETE FROM` in application code.

**Deleting must not destroy financial history.** A delete that would remove
records someone might need for accounting is refused with an explanation, not
silently cascaded.

## Development

Prerequisites, install steps and the full command list are in the
[README](README.md#development).

Before opening a pull request, all of these must pass:

```powershell
npm.cmd run check                                   # 0 errors
npm.cmd test                                        # frontend tests
cargo test --manifest-path .\src-tauri\Cargo.toml   # Rust tests
npm.cmd run build                                   # frontend build
```

`npm run check` currently reports four accessibility warnings in
`src/widget/TaskWidget.svelte` (an `autofocus`, and three resize grips that are
pointer-only by design). Those are known and expected. **Zero errors** is the
bar; do not add new warnings.

## Design system

The visual language lives in one place: the `:root` block of
[`src/app.css`](src/app.css). See [Customising](README.md#customising) for what
you can change and what will break if you change it carelessly.

Rules the codebase enforces:

- Type sizes come from the scale (11, 12, 13, 15, 17, 20, 24, 28 px). No
  half-pixels, no invented sizes.
- Font weights are 400, 500, 600 or 700. Nothing between.
- Radii are `--radius-control` (6px), `--radius-panel` (10px), `--radius-sheet`
  (14px), or `--radius-pill`.
- **Colour never carries meaning alone.** Every status colour is paired with a
  text label, so the interface still works for someone who cannot distinguish
  them.
- Screens are composed from the shared primitives in `src/lib/components/`
  (`Card`, `StatCard`, `StatRow`, `SectionHeader`, `MeterBar`). Reach for those
  before writing new layout CSS.

## Charts

Charts are built from `BarChart` plus a pure series function in
`src/lib/features/dashboard/chartSeries.ts`. Four rules hold:

- **One hue, never a palette.** Charts use the emphasis form — a single fill
  against a neutral track. Adding a second series colour means re-validating
  the pair for colourblind separation, not picking something that looks nice.
  See [Customising](README.md#customising) for the measured numbers.
- **Every value is reachable without hover.** The plot itself is
  `aria-hidden`; the `<details>` table below it is the accessible
  representation. A tooltip may enhance, never gate.
- **Bucket with the same resolver as the surrounding view.** `completionsByDay`
  takes the day resolver as an argument because Today and Review bucket by
  *local* day. Pass the wrong one and, east of UTC, a task finished at 07:00
  lands on the previous bar and the chart contradicts the meter beside it.
- **Never sum across currencies.** A money chart plots exactly one currency and
  says which one in its subtitle.

Series functions must stay free of Svelte and of `Date.now()` so they can be
tested directly under `tests/dashboard/`.

## Motion

Animations use Svelte's built-in transitions, and **every duration must be
routed through `motionDuration()`** from [`src/lib/motion.ts`](src/lib/motion.ts):

```svelte
<div transition:fade={{ duration: motionDuration(180) }}>
```

This matters more than it looks. Svelte 5 implements transitions through the
Web Animations API, so the CSS `@media (prefers-reduced-motion: reduce)` rule
does **not** reach them. A raw numeric duration silently ignores a user who has
asked their operating system for less motion. Settings also exposes a per-device
override (Follow system / Always on / Off).

## Accessibility

- Interactive controls need an accessible name — icon-only buttons take an
  `aria-label`.
- Dialogs use the native `<dialog>` element with `showModal()`, which supplies
  focus trapping and restoration.
- Body text should meet WCAG AA contrast (4.5:1). `--text-secondary` passes on
  the card surface; `--text-tertiary` does not, so it is reserved for small
  supporting metadata.

## Pull requests

Keep changes focused — one concern per pull request. Explain the user-visible
behaviour that changes, not only the implementation. If a change affects data
integrity, money handling, or deletion, say so explicitly in the description.
