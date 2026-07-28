# Spatial Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give RudeSync spatial continuity — sheets that emerge rather than snap, completed tasks that settle out, content that grows rather than jumps, and sections that cross-fade instead of cutting.

**Architecture:** Svelte's built-in `transition:`/`animate:` directives only. No new dependency. Motion is expressed through existing `--duration` (180ms) and `--ease` tokens where CSS drives it, and matching numeric durations where Svelte drives it.

**Tech Stack:** Svelte 5, TypeScript, existing design tokens.

## Global Constraints

- **Motion must be motivated.** Every transition communicates hierarchy, state change, feedback or spatial continuity. Decorative motion is rejected.
- **Durations:** sheets 200ms in / 140ms out; list outros 180ms; expand/collapse 180ms; section cross-fade 140ms. Exits are shorter than entrances.
- **Reduced motion is already handled globally** — `src/app.css` sets `animation-duration: 0.01ms !important` under `prefers-reduced-motion: reduce`, and Svelte's built-in transitions compile to CSS animations, so they are neutered automatically. **Do not add JS-based motion that bypasses this**, and verify the rule still covers each transition you add.
- **Never block input.** No transition may gate a click, and no `|local` modifier may trap focus. Dialogs must stay operable the moment they appear.
- Animate `transform` and `opacity` only. Never animate `width`, `height`, `top` or `left` — with the single exception of Svelte's `slide`, which is purpose-built for expand/collapse.
- No new dependencies. No new colours, radii, type sizes or weights.
- No behaviour, handler, validation, routing or copy changes. This is presentation only.
- The floating task widget (`src/widget/**`) is out of scope — it is a glance surface and must stay still.
- `npm run check` must report **0 errors**; 4 pre-existing a11y warnings in `src/widget/TaskWidget.svelte` are expected. `npm test` must stay at 31 passing.

---

### Task 1: Sheets emerge instead of snapping

**Files:**
- Modify: `src/lib/features/tasks/TaskDialog.svelte`
- Modify: `src/lib/features/work/components/WorkDialog.svelte`
- Modify: `src/App.svelte` (the first-run setup overlay)

**Interfaces:**
- Produces: a consistent sheet entrance/exit that later tasks do not touch.

All three currently mount with a bare `{#if open}` and appear instantly.

- [ ] **Step 1: Add the sheet transition to `TaskDialog.svelte`**

Import at the top of the `<script>`:

```ts
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
```

Apply to the overlay (the scrim) and the panel (the sheet). The scrim fades; the panel rises and scales:

```svelte
<div class="overlay" transition:fade={{ duration: 140 }}>
  <div
    class="dialog"
    in:scale={{ duration: 200, start: 0.96, opacity: 0, easing: cubicOut }}
    out:scale={{ duration: 140, start: 0.98, opacity: 0, easing: cubicOut }}
  >
```

Use the file's real class names — read them first; do not rename anything.

- [ ] **Step 2: Apply the identical treatment to `WorkDialog.svelte`**

Same imports, same durations, same `start` values, on that file's overlay and panel elements. Consistency across sheets is the point — do not vary the numbers.

- [ ] **Step 3: Apply it to the first-run setup overlay in `src/App.svelte`**

The markup uses `.setup-overlay` and `.setup-dialog`. Same treatment.

- [ ] **Step 4: Verify reduced motion still wins**

Confirm `src/app.css`'s `@media (prefers-reduced-motion: reduce)` block still contains `animation-duration: 0.01ms !important`, and state in your report why that neuters these transitions (Svelte's `fade`/`scale` emit CSS animations).

- [ ] **Step 5: Verify**

Run: `npm run check` → 0 ERRORS.
Run: `npm test` → 31 pass.
Run: `npm run build` → succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/lib/features/tasks/TaskDialog.svelte src/lib/features/work/components/WorkDialog.svelte src/App.svelte
git commit -m "feat(motion): let sheets emerge instead of snapping in"
```

---

### Task 2: Completed tasks settle out of the list

**Files:**
- Modify: `src/App.svelte` (the Today and Tasks list `{#each}` blocks)

**Interfaces:**
- Consumes: the keyed `{#each … (task.id)}` blocks that already exist.

Ticking a task currently changes the list with no acknowledgement of what moved.

- [ ] **Step 1: Add outro and reflow to the Today lists**

The lists at roughly `src/App.svelte:721` (`overdueTasks`), `:729` (`todayTasks`), and `:799` (`nextSevenTasks`) are already keyed by `task.id`, which is what outros and FLIP require.

Import:

```ts
  import { fade } from "svelte/transition";
  import { flip } from "svelte/animate";
```

Apply to the element inside each keyed block:

```svelte
  {#each todayTasks as task (task.id)}
    <div out:fade={{ duration: 180 }} animate:flip={{ duration: 180 }}>
      <TaskRow … />
    </div>
  {/each}
```

Only add the wrapper if `TaskRow` cannot take the directives directly — check first; a wrapper that breaks the list's grid or separator rules is worse than no animation. The row separators are owned by `.task-list .task-row + .task-row` in `src/app.css`, so confirm they still render after any wrapper is introduced.

- [ ] **Step 2: Apply the same to the Tasks screen list**

Find the `{:else if active === "tasks"}` block's keyed list and give it the identical treatment and durations.

- [ ] **Step 3: Verify the separators survived**

Run `npm run check` and confirm no unused-selector warning appears for `.task-row` rules — that would mean a wrapper broke the sibling selector.

- [ ] **Step 4: Verify**

Run: `npm run check` → 0 ERRORS. `npm test` → 31 pass. `npm run build` → succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/App.svelte
git commit -m "feat(motion): settle completed tasks out of their list"
```

---

### Task 3: Expanding content grows instead of jumping

**Files:**
- Modify: `src/lib/features/work/WorkView.svelte`
- Modify: `src/lib/features/money/MoneyView.svelte`

**Interfaces:**
- Consumes: existing `{#if expanded}`-style conditional blocks.

- [ ] **Step 1: Find the expand/collapse regions**

In each file, locate the conditional blocks that reveal detail — project milestones in `WorkView`, invoice/payment/loan detail rows in `MoneyView`. List them in your report before editing.

- [ ] **Step 2: Apply `slide`**

```ts
  import { slide } from "svelte/transition";
```

```svelte
  {#if expanded}
    <div transition:slide={{ duration: 180 }}>
      <!-- existing content, unchanged -->
    </div>
  {/if}
```

`slide` animates height, which is the one permitted exception to the transform/opacity rule because it is purpose-built for this. Do not add it to anything that is not an expand/collapse.

- [ ] **Step 3: Verify**

Run: `npm run check` → 0 ERRORS and no unused-selector warnings in either file. `npm test` → 31 pass. `npm run build` → succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/lib/features/work/WorkView.svelte src/lib/features/money/MoneyView.svelte
git commit -m "feat(motion): grow expanding detail instead of jumping"
```

---

### Task 4: Sections cross-fade instead of cutting

**Files:**
- Modify: `src/App.svelte` (the content region that switches on `active`)

This is the most easily overdone of the four. It must be **fast** — this is a tool used all day, and a slow section change becomes friction on every single navigation.

- [ ] **Step 1: Key the content region on the active section**

Wrap the section-switching content in `{#key active}` so Svelte tears down and rebuilds on change, then fade in:

```svelte
{#key active}
  <div in:fade={{ duration: 140 }}>
    <!-- the existing {#if active === "today"} … {/if} chain, unchanged -->
  </div>
{/key}
```

Use `in:` only — no `out:`. An outro would delay the new section appearing, which is exactly the friction to avoid.

- [ ] **Step 2: Confirm scroll position and focus are unaffected**

`{#key}` remounts the subtree. Verify in your report that this does not reset anything the user would notice: check that `.content-scroll` (the scroll container) sits OUTSIDE the keyed block, so scroll position is not affected mid-section, and that no input inside a section loses focus during normal use.

If keying the region turns out to remount something stateful (a form mid-edit, a filter selection), do NOT ship it — report that instead. A cosmetic fade is not worth losing user state.

- [ ] **Step 3: Verify**

Run: `npm run check` → 0 ERRORS. `npm test` → 31 pass. `npm run build` → succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/App.svelte
git commit -m "feat(motion): cross-fade between sections"
```

---

## Self-Review

**Spec coverage:** all four places the user chose are covered — sheets (Task 1), task completion (Task 2), expand/collapse (Task 3), section switching (Task 4).

**Placeholder scan:** no TBD/TODO. Tasks 2-4 instruct locating the real markup first because the exact class names and block structures live in large existing files; the directives, durations and easings are given exactly.

**Type consistency:** durations are consistent and stated once per context (sheets 200/140, lists 180, expand 180, sections 140). Every import is named exactly (`fade`, `scale`, `slide` from `svelte/transition`; `flip` from `svelte/animate`; `cubicOut` from `svelte/easing`).

## Open Risks

- **Task 4 is the one that can regress behaviour.** `{#key}` remounts its subtree; if a stateful child sits inside it, state is lost on every navigation. The task requires verifying this and abandoning the fade rather than shipping the loss.
- **Task 2's wrapper element** could break `.task-list .task-row + .task-row`, the sibling selector that draws row separators. The task requires checking the rendered separators afterwards.
- Reduced motion is covered by the existing global rule, but only because Svelte transitions compile to CSS animations. If any future motion is written in JS, that coverage silently stops applying.
