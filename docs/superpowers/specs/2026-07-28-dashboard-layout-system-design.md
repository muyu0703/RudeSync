# Dashboard Layout System — Design (Cycle 1 of 2)

Status: Approved scope, ready for implementation plan
Date: 2026-07-28
Feature area: UI layer (all screens)
Platform: Windows 11 desktop (Tauri 2). Keep platform-specific code isolated.

## 1. Goal

Rebuild RudeSync's UI as a card-grid dashboard, modelled on a reference analytics
dashboard the user supplied, while keeping the app's own emerald-on-dark
identity. Replace ad-hoc per-screen styling with a small set of shared
primitives so every screen is consistent by construction rather than by
discipline.

This is **Cycle 1 of 2**. Cycle 1 delivers the layout system and applies it to
every screen. Cycle 2 (a separate spec) adds data visualization into slots this
cycle reserves.

## 2. Confirmed decisions (fixed inputs)

- **Two cycles.** Layout system first, charts second, so the direction can be
  corrected cheaply.
- **Palette: "Deep forest."** Ground `#070d0a`, cards `#0d1712`, raised
  `#142019`. Emerald accent `#3ddc84` unchanged.
  Rationale: the ground must read green (brand identity) but stay dark enough
  that the emerald accent still functions as a *signal* rather than blending in.
- **Today stat row (4 cards):** Tasks (open + done today), Overdue, Outstanding
  money, Received this month.
- **Stat rows also on Work, Money and Review.** Tasks and Settings get the card
  grid without a stat row.
- **Sidebar stays navigation-only.** No contextual client/project list.
- **Chart slots are reserved but rendered empty** this cycle, so Cycle 2 drops
  in without re-layout.
- **Typography foundation already landed** (commits `5a488bd`, `00afaa9`,
  `7ec7679`) and is retained: SF Pro on Apple platforms / self-hosted Inter
  elsewhere, whole-pixel type scale, four weights, four-value radius scale.
- **Window translucency is abandoned.** Mica requires a transparent window to
  show through WebView2, which produced visual artifacts on this machine. Depth
  comes from surface layering and elevation instead.

## 3. What this replaces

Roughly 3,500 lines of per-component scoped CSS across 11 `.svelte` files
(`WorkView` 1213, `SettingsView` 697, `ProjectForm` 305, `TaskDialog` 226,
`MoneyView` 178, and smaller forms/dialogs). Most of it is deleted rather than
rewritten: screens compose shared primitives instead of restyling the same
patterns locally.

## 4. Design tokens (extend the existing `:root` in `src/app.css`)

Surfaces move to the Deep forest ramp; everything else stays as landed.

- `--surface-window: #070d0a` (app ground)
- `--surface-content: #0d1712` (cards, panels)
- `--surface-raised: #142019` (hover, segmented active)
- `--surface-overlay: #1a2820` (sheets, popovers)
- Text stays white-with-alpha (`0.92 / 0.56 / 0.38 / 0.26`), separators
  `rgb(255 255 255 / 0.08)`.
- Accent `#3ddc84`, danger `#ff6961`, amber `#e3b341` unchanged.

**Semantic colour contract** — colour states meaning, never decorates:

| Meaning | Token | Used for |
|---|---|---|
| Positive | `--accent` | received, completed, on-track milestones |
| Danger | `--danger` | overdue tasks and invoices |
| Warning | `--amber` | due soon, outstanding balances |
| Neutral | text tokens | informational counts |

Per the accessibility rules consulted, colour must never be the sole carrier of
meaning: every status colour is paired with a label or icon.

## 5. Components (new, `src/lib/components/`)

Each is small, presentational, and independently usable.

**`Card.svelte`** — elevated surface. Props: `padded` (bool, default true).
Slots: default, optional `header`. One radius (`--radius-panel`), one shadow
(`--shadow-raised`). No borders.

**`StatCard.svelte`** — the stat-row unit.
Props: `label: string`, `value: string`, `detail?: string`,
`tone?: "neutral" | "positive" | "warning" | "danger"` (default `neutral`),
`icon?: string` (an `Icon` name). Renders an icon tile tinted by `tone`, the
label, the value in tabular numerals, and an optional detail line. `tone` drives
the value colour and icon tint; it never changes layout.

**`SectionHeader.svelte`** — Props: `title: string`, `subtext?: string`.
Slot `actions` for right-aligned filters/buttons. Replaces the ad-hoc
kicker + heading pairs currently repeated per screen.

**`MeterBar.svelte`** — labelled progress. Props: `label: string`,
`value: number`, `max: number`, `detail?: string`, `tone?` (as above).
Renders label + detail row above a track. Always shows the numeric detail so the
bar is not the only signal.

**`StatRow.svelte`** — thin grid wrapper: 4 columns desktop, 2 at ≤1020px,
1 at ≤780px. Exists so the breakpoints live in one place.

## 6. Screen composition

Navigation, routes, IA, copy and all behaviour stay exactly as they are.

- **Today** — `StatRow` (tasks / overdue / outstanding / received) → two-column:
  today's tasks `Card` (with `SectionHeader` + filter) beside a stack of
  milestone `MeterBar`s and one reserved chart slot → upcoming `Card`.
- **Tasks** — `SectionHeader` + filters, task list in a `Card`. No stat row.
- **Work** — `StatRow` (active projects / remaining to invoice) → project
  `Card`s, each showing milestone `MeterBar`s.
- **Money** — `StatRow` (outstanding / received / overdue invoices) → invoice
  `Card` and payments `Card` + reserved chart slot.
- **Review** — `StatRow` → weekly summary `Card`s + reserved chart slots (this
  screen is the most chart-heavy in Cycle 2).
- **Settings** — `Card` grid, no stat row.

**Reserved chart slot:** a `Card` containing a fixed-height empty region with a
neutral placeholder. Cycle 2 replaces the placeholder's contents only.

## 7. Stat derivation (the only new logic)

A pure module `src/lib/features/dashboard/stats.ts` derives stat values from data
already loaded client-side. No backend changes.

- `openTaskCount(tasks)`, `completedTodayCount(tasks, today)`
- `overdueCount(tasks, invoices, today)` — tasks past `dueDate` plus issued
  invoices past their due date, excluding completed/paid/void
- `outstandingByCurrency(invoices)` — sum of unpaid balances, grouped by
  currency, never summed across currencies
- `receivedThisMonth(invoices, today)` — sum of payment amounts whose
  `receivedDate` falls in the current month, grouped by currency

All amounts stay integer minor units. All currency grouping follows the existing
rule: per-currency only, never combined.

## 8. Testing

- **Unit (`node --test`):** every function in `stats.ts` — open/completed counts,
  overdue including both tasks and invoices, per-currency grouping with two
  currencies present, month boundaries for `receivedThisMonth`, and empty-input
  cases returning zero rather than throwing.
- **Type check:** `npm run check` must remain 0 errors.
- **Visual:** the user reviews each screen; no automated visual testing exists.

## 9. Non-goals (Cycle 1)

- No charts or data visualization (Cycle 2).
- No navigation, route, IA or copy changes.
- No backend, schema or command changes.
- No light mode.
- No new colours beyond the Deep forest surface ramp.
- No window translucency.
- No changes to the floating task widget (separate surface, own styling).

## 10. Risks

- **Wide blast radius.** Every screen changes. Mitigated by converting one
  screen per task with a review gate, so a bad pattern is caught early.
- **Deleting scoped CSS can strand markup.** Each conversion task must verify
  the screen renders before moving on; `npm run check` catches unused-selector
  regressions.
- **Svelte scoping.** Sibling selectors do not match across component instances
  (already hit once with task-row separators). Structural rules that span
  instances belong in `app.css`, not in a child component's scoped block.
