# UX Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all 27 findings from the UX audit — 7 blocking, 14 important, 6 polish — so the app stops losing work, hiding its primary actions, and presenting the same idea three different ways.

**Architecture:** Findings are grouped into seven tasks by the area they touch, so each task is independently reviewable and two tasks never edit the same region. Tasks A-C are the blocking ones and run first.

**Tech Stack:** Svelte 5, TypeScript, Tauri 2, existing design tokens and primitives.

## Global Constraints

- **No backend, schema, migration or Rust command changes.** Every fix is frontend.
- Money is integer minor units; per-currency only, never summed across currencies.
- Palette fixed: ground `#070d0a`, cards `#0d1712`, raised `#142019`, overlay `#1a2820`; accent `#3ddc84`, danger `#ff6961`, amber `#e3b341`. Semantic colour is never the only carrier of meaning.
- Type scale whole pixels only (11,12,13,15,17,20,24,28); weights 400/500/600/700; radii 6/10/14/pill.
- **Every Svelte transition duration routes through `motionDuration()`** in `src/lib/motion.ts` — Svelte 5 uses the Web Animations API, which the CSS `prefers-reduced-motion` rule cannot reach.
- Minimum window width is **900px**; sidebar 232px (60px below the 1020px breakpoint); `.content-scroll` is `overflow-x: hidden`, so horizontal overflow is invisible, not scrollable.
- `src/widget/**` is out of scope and must not be touched.
- `npm run check` → **0 errors** (4 pre-existing a11y warnings in `src/widget/TaskWidget.svelte` are expected). `npm test` → 31 pass. `npm run build` → succeeds.
- Copy changes ARE permitted in this plan where a finding calls for them, but only the specific strings named.

---

### Task A: The page can't scroll

**Finding 1 (Blocking).** `src/app.css:276-282` with `src/App.svelte:624-660`.
`.workspace` declares three rows (`var(--topbar-height) auto minmax(0,1fr)`) but renders 2-4 children depending on whether the error banner and backup warning are showing. With **no** banner — the normal case — `.content-scroll` lands in the `auto` row, sizes to its full content, and `.workspace { overflow: hidden }` clips everything past the fold. Its own `overflow-y: auto` never engages because nothing constrains its height. The user sees the bottom of a screen cut off **with no scrollbar**.

**Files:** `src/app.css`, possibly `src/App.svelte`.

- [ ] **Step 1: Make the scroller always occupy the flexible row**

Give `.content-scroll` an explicit `grid-row: -2 / -1` so it always lands in the final `minmax(0,1fr)` track regardless of how many banners render. Alternatively wrap the two optional banners in a single always-rendered element so the child count is constant. Pick one, and say which and why.

- [ ] **Step 2: Prove it in all four banner combinations**

Reason through and state in your report what row each child occupies with: no banner; error only; backup warning only; both. In every case `.content-scroll` must be in a height-constrained track so its `overflow-y: auto` engages.

- [ ] **Step 3: Verify**

`npm run check` → 0 errors. `npm test` → 31 pass. `npm run build` → succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/app.css src/App.svelte
git commit -m "fix(ui): keep the content area scrollable in every banner state"
```

---

### Task B: Dialogs hide their primary action

**Finding 2 (Blocking).** `WorkDialog.svelte:67,155-161`; footers at `ProjectForm.svelte:463`, `ClientForm.svelte:74`, `WorkEntryForm.svelte:117`. The submit button sits inside the scrolling `.dialog-body`, so on a long form the user reaches the end and sees no "Create project" button.

**Finding 3 (Blocking).** `WorkDialog.svelte:90-96,155-161`. `.dialog-body` reserves a hard-coded 78px for the header, but the header renders a title *plus* a two-line description (~100px at 470px wide), so header + body exceeds `.dialog-card`'s `max-height` and `overflow: hidden` shaves 20-30px off the bottom — even after scrolling, the footer is sliced.

**Finding 4 (Blocking).** `TaskDialog.svelte:389-394,413-421`. `.dialog` is the scroller and `header` is `position: sticky`, but `footer` is not — so on a task with recurrence and subtasks the save button leaves the screen while the header stays.

**Finding 26 (Polish, same area).** `TaskDialog.svelte:210-227` is a plain `<div role="dialog">`, so Tab walks out into the page behind it and focus is lost to `<body>` on close. `WorkDialog.svelte:15` uses a native `<dialog>` and gets focus trapping and restoration for free.

**Files:** `src/lib/features/work/components/WorkDialog.svelte`, `src/lib/features/tasks/TaskDialog.svelte`, and the three work forms if their footers must move.

- [ ] **Step 1: Rebuild `WorkDialog` as a flex column**

Replace the hard-coded `max-height: calc(min(760px, 100vh - 32px) - 78px)` arithmetic with:

```css
.dialog-card { display: flex; flex-direction: column; }
.dialog-body { flex: 1; min-height: 0; overflow-y: auto; }
```

so the body takes the leftover space whatever the header's real height is. This closes finding 3.

- [ ] **Step 2: Give the dialog a pinned footer**

Add a `footer` slot to `WorkDialog` that renders as a sibling of `.dialog-body` inside `.dialog-card`, so it never scrolls. Move each form's existing `<footer>` markup into that slot — `ProjectForm`, `ClientForm`, `WorkEntryForm`. **Do not change the buttons, their labels, their `disabled` expressions or their handlers**; only their position in the tree. Closes finding 2.

- [ ] **Step 3: Pin `TaskDialog`'s footer**

`TaskDialog` owns its own overlay. Make its `footer` sticky to the bottom of the scroller the same way its `header` is already sticky to the top, or restructure it as a flex column like `WorkDialog`. Closes finding 4.

- [ ] **Step 4: Convert `TaskDialog` to a native `<dialog>`**

Use `showModal()` for focus trapping and restoration, mirroring `WorkDialog.svelte`'s approach. Keep every existing handler and the Escape-to-close behaviour. **Note:** `WorkDialog` has no exit transition because a native `dialog.close()` is synchronous — expect and accept the same here; keep the intro transition gated through `motionDuration()`. Closes finding 26.

- [ ] **Step 5: Verify**

`npm run check` → 0 errors, no unused-selector warnings in touched files. `npm test` → 31 pass. `npm run build` → succeeds. In your report, state for each of the four dialogs where its footer now sits and why it can no longer scroll away.

- [ ] **Step 6: Commit**

```bash
git add src/lib/features/work/components src/lib/features/tasks/TaskDialog.svelte
git commit -m "fix(ui): pin dialog footers so the primary action is always reachable"
```

---

### Task C: The app throws away your work

**Finding 5 (Blocking).** `SettingsView.svelte:44-46,334-354` with `App.svelte:429-436`. Settings tracks `hasChanges` and even displays "You have unsaved changes", but clicking a sidebar item destroys the component with no warning.

**Finding 6 (Blocking).** `App.svelte:558-582`. `Ctrl+N` while editing a task re-hydrates the open dialog to a blank new task (`TaskDialog.svelte:53-62`), erasing what was typed; `Ctrl+3` while a Work dialog is open unmounts `WorkView` and the modal vanishes mid-edit.

**Finding 7 (Blocking).** `WorkDialog.svelte:21-28,36`; `TaskDialog.svelte:203-205,215`. A stray backdrop click or Escape discards a half-filled form instantly, with no warning.

**Finding 27 (Polish, same area).** `ProjectForm.svelte:99-104,400-406`. `formInvalid` never checks milestone labels, so "Create project" is enabled with a blank label and clicking it only produces a native browser bubble on a field that may be scrolled out of view.

**Files:** `src/App.svelte`, `src/lib/features/settings/SettingsView.svelte`, `src/lib/features/work/components/WorkDialog.svelte`, `src/lib/features/tasks/TaskDialog.svelte`, `src/lib/features/work/components/ProjectForm.svelte`.

- [ ] **Step 1: Guard keyboard shortcuts while a modal is open**

In `App.svelte`'s `handleShortcut`, early-return when any dialog is open. There is already state for the task dialog; check what tracks the Work dialogs and use it. Closes finding 6.

- [ ] **Step 2: Confirm before discarding a dirty dialog**

Track dirtiness in `WorkDialog` and `TaskDialog` and confirm before closing on backdrop click or Escape. The explicit Cancel button stays the no-questions exit — do not add friction there. Closes finding 7.

- [ ] **Step 3: Protect unsaved Settings edits**

`selectSection` in `App.svelte` must confirm when the settings form is dirty. `SettingsView` already computes `hasChanges`; surface it so the shell can consult it. Closes finding 5.

- [ ] **Step 4: Make milestone labels block submission**

Add `milestones.some((m) => !m.label.trim())` to `ProjectForm`'s `formInvalid`, and show the same `.field-error` treatment the date fields use rather than relying on the native validation bubble. Closes finding 27.

- [ ] **Step 5: Verify**

`npm run check` → 0 errors. `npm test` → 31 pass. `npm run build` → succeeds. In your report, walk through: editing a task then pressing Ctrl+N; editing Settings then clicking Work; clicking the backdrop of a half-filled project form. State what happens in each.

- [ ] **Step 6: Commit**

```bash
git add src/App.svelte src/lib/features/settings/SettingsView.svelte src/lib/features/work/components src/lib/features/tasks/TaskDialog.svelte
git commit -m "fix(ui): stop discarding unsaved work without warning"
```

---

### Task D: Money's dangerous and invisible actions

**Finding 10 (Important, dangerous).** `MoneyView.svelte:1004,1140-1142`. "Create & issue" is the composer form's submit button, so pressing Enter in any line-item or tax field **irreversibly issues the invoice**, locking its number and issue date — while the same operation from the invoice list (`:496-499`) asks for confirmation first.

**Finding 11 (Important).** `MoneyView.svelte:764-779`. Marking a loan installment paid opens a raw `window.prompt` asking the user to type `YYYY-MM-DD`, in an app where every other date uses a picker. A typo is rejected by a banner at the top of the page.

**Finding 8 (Important).** `MoneyView.svelte:321-350`, composer renders at `:1003`. Clicking "Edit draft" on an invoice far down the list opens the composer at the top of the panel with no scroll, so nothing appears to happen and the user clicks again.

**Finding 9 (Important).** `MoneyView.svelte:926-937` vs actions at `:1210-1229`. Voiding or issuing an invoice writes its success/error message to a banner above the tab bar, off-screen from the action.

**Finding 18 (Important).** `MoneyView.svelte:939-964`. The primary action sits *above* the tab strip that decides what it does, changes label between tabs, and vanishes entirely on Earnings — so the tab bar jumps as the user switches.

**Finding 21 (Important).** `MoneyView.svelte:893-900` vs `App.svelte:207,625-629`. Money prints its own "Invoices and obligations / Money" heading on top of the one the shell already renders, and its `h2` is 24px against the topbar's 15px `h1`. No other screen does this.

**Files:** `src/lib/features/money/MoneyView.svelte`.

- [ ] **Step 1: Make Enter safe in the composer**

Make "Save draft" the form's submit action, and give "Create & issue" `type="button"` routed through the same confirmation the invoice list already uses ("Issue …? Its number and issue date will be locked."). A user pressing Enter must never irreversibly issue an invoice. Closes finding 10.

- [ ] **Step 2: Replace the browser prompt with the existing date control**

An inline `type="date"` control already exists two lines below at `:1398-1413`. Reuse that pattern for marking an installment paid instead of `window.prompt`. Closes finding 11.

- [ ] **Step 3: Scroll and focus the composer when it opens**

`scrollIntoView` the composer and focus its first field. Closes finding 8.

- [ ] **Step 4: Put feedback where the action happened**

Render the success/error notice inline on the affected invoice card, or scroll the existing banner into view when its content changes. Pick one and say which. Closes finding 9.

- [ ] **Step 5: Stop the tab bar jumping**

Move the primary action inside each tab panel, or keep the row rendered so it holds its height on Earnings. Closes finding 18.

- [ ] **Step 6: Delete the duplicate page heading**

Remove the `.money-heading` block; move its one-line explainer into the tab panel so the information is kept. Closes finding 21.

- [ ] **Step 7: Verify**

`npm run check` → 0 errors, no unused-selector warnings in `MoneyView.svelte`. `npm test` → 31 pass. `npm run build` → succeeds. Confirm in your report that no invoice/payment/loan handler, service call or validation attribute changed behaviour — only how the actions are triggered and confirmed.

- [ ] **Step 8: Commit**

```bash
git add src/lib/features/money/MoneyView.svelte
git commit -m "fix(money): confirm irreversible actions and put feedback where they happen"
```

---

### Task E: Controls that lie

**Finding 12 (Important).** `App.svelte:634-637`. The top-bar bell has a green notification dot that never clears and no click handler; the "RS" avatar labelled "Open profile" is inert. Users click both, repeatedly.

**Finding 13 (Important).** `Sidebar.svelte:57-63`. "Weekly reset · Friday · 4:00 PM" is hard-coded static text in a nav-shaped block; nothing schedules or reacts to it.

**Finding 14 (Important).** `App.svelte:804,936,940`; `MoneyView.svelte:1149`; style at `app.css:411-415`. Four cards titled "Earnings trend", "Completion trend", "Earnings by month" and "Invoiced vs received" render as empty dashed rectangles, which reads as broken rather than "coming later".

**Finding 15 (Important).** `App.svelte:631,438-441,198-200`. The global-looking "Search — Ctrl K" jumps to Tasks and filters task titles only; it will never find a client, project or invoice number.

**Finding 17 (Important).** `App.svelte:470-478,842-844`. "Pop out widget" returns silently if the Tauri bridge is absent, and `void openTaskWidget()` swallows any rejection — the button just does nothing.

**Files:** `src/App.svelte`, `src/lib/components/Sidebar.svelte`, `src/lib/features/money/MoneyView.svelte`, `src/app.css`.

- [ ] **Step 1: Remove the inert top-bar controls**

Delete the bell (with its permanent dot) and the avatar. They have no behaviour, and a control that does nothing is worse than no control. Closes finding 12.

- [ ] **Step 2: Remove the fictional weekly-reset block**

Delete it from `Sidebar.svelte`. It describes a feature that does not exist. Closes finding 13.

- [ ] **Step 3: Hide the empty chart cards**

Remove the four chart-slot cards and the `.chart-slot` rule. Cycle 2 will reintroduce them together with real charts; until then they only signal breakage. Closes finding 14.

- [ ] **Step 4: Tell the truth about search**

Relabel the top-bar control **"Find task"** and render it only on the Today and Tasks sections. Closes finding 15.

- [ ] **Step 5: Surface pop-out failures**

`try/catch` the invoke into the existing `errorMessage` state, and disable the button when the Tauri bridge is unavailable. Closes finding 17.

- [ ] **Step 6: Verify**

`npm run check` → 0 errors, no unused-selector warnings. `npm test` → 31 pass. `npm run build` → succeeds.

- [ ] **Step 7: Commit**

```bash
git add src/App.svelte src/lib/components/Sidebar.svelte src/lib/features/money/MoneyView.svelte src/app.css
git commit -m "fix(ui): remove controls that promise behaviour they do not have"
```

---

### Task F: Nothing in Work can be deleted

**Finding 16 (Important).** `WorkView.svelte:429-445,615-617,629-634,707-715`. A client, project or completed-work record created by mistake is permanent. Projects can only be set to "archived" from inside the edit form (`ProjectForm.svelte:263-266`); clients and work entries have no removal path at all.

**Files:** `src/lib/features/work/WorkView.svelte`, and `src/lib/features/work/workService.ts` only if a delete method genuinely does not exist.

- [ ] **Step 1: Find out what the backend already supports**

Read `src/lib/features/work/workService.ts` and the registered commands in `src-tauri/src/lib.rs`. **Do not add a Rust command** — that is out of scope for this plan. Report exactly which of client / project / work-entry deletion the backend already exposes.

- [ ] **Step 2: Add confirmed delete where the backend allows it**

For each entity the backend supports, add a delete action with an explicit confirmation naming what will be removed. Follow the discard-draft pattern already used in `MoneyView` for wording and placement.

- [ ] **Step 3: Report honestly on what you could not add**

If the backend exposes no deletion for an entity, do NOT fake it and do NOT add a Rust command. State clearly in your report which entities still cannot be deleted, so it can be scheduled as backend work.

- [ ] **Step 4: Verify**

`npm run check` → 0 errors. `npm test` → 31 pass. `npm run build` → succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/lib/features/work
git commit -m "feat(work): allow deleting records created by mistake"
```

---

### Task G: One app, one vocabulary

**Finding 24 (Polish, largest).** `MoneyView.svelte:966-994` uses a bespoke `.tabs` control instead of the shared `.segmented` used in `App.svelte:849` and `SettingsView.svelte:553`; `:1435,1444-1475` re-declare `input`/`select`/`button` styles instead of using `.field-input`/`.primary-button`; `:1155-1163,1263,1346-1350` use `INV`/`USD`/`LOAN` text badges where every other empty state uses `<Icon>`.

**Finding 25 (Polish).** Work opens modal dialogs (`WorkView.svelte:784-838`); Money expands an inline composer (`MoneyView.svelte:1003-1144`) — the same task with two mental models and two different Cancel behaviours.

**Finding 19 (Important).** `SettingsView.svelte:551-564` vs `:500-531,588`. The Animations segmented control saves instantly while the visually identical toggles around it need "Save changes"; the user cannot tell which is which.

**Finding 20 (Important).** `SettingsView.svelte:334-354`. "Save changes" sits at the top of the longest form in the app, several screens above where the user finishes editing.

**Finding 22 (Polish).** `StatRow.svelte:4-10` is a fixed 4-column grid, but `WorkView.svelte:394-418` and `MoneyView.svelte:902-924` supply 3 cards, leaving a quarter-width hole.

**Finding 23 (Polish).** `WorkView.svelte:629-631,712-715,766-769` and `TaskRow.svelte:73-80` use the "⋯" overflow glyph as the Edit button, so users expect a menu and get an edit dialog; on client and task rows it is icon-only with no visible label.

**Files:** `src/lib/components/StatRow.svelte`, `src/lib/features/work/WorkView.svelte`, `src/lib/components/TaskRow.svelte`, `src/lib/features/settings/SettingsView.svelte`, `src/lib/features/money/MoneyView.svelte`.

- [ ] **Step 1: Make the stat row fit its contents**

`grid-template-columns: repeat(auto-fit, minmax(220px, 1fr))`. Verify it still collapses sensibly at the 1020px and 780px breakpoints and cannot clip at 900px. Closes finding 22.

- [ ] **Step 2: Use an edit affordance that means edit**

Replace the "⋯" glyph with a pencil-style icon from the existing `Icon` set, or keep "⋯" and make it open a real menu. Whichever you choose, every instance must have an accessible name. Closes finding 23.

- [ ] **Step 3: Make Settings' save behaviour consistent**

Either route the motion preference through the same dirty/save cycle as its neighbours, or move it out of the form with an explicit "Applies immediately" note. Pick one and say why. Closes finding 19.

- [ ] **Step 4: Put Save where the editing ends**

Make `.page-actions` sticky within `.content-scroll`, or repeat the save row at the end of the form. Closes finding 20.

- [ ] **Step 5: Migrate Money onto the shared vocabulary**

Replace `.tabs` with `.segmented`; replace the local input/select/button styles with `.field-input`, `.field-label`, `.field-hint` and `.primary-button`; replace the `INV`/`USD`/`LOAN` text badges with `<Icon>` as every other empty state does. Do not change any handler or validation attribute. Closes finding 24.

- [ ] **Step 6: Report on the modal-vs-inline split**

Finding 25 (Work uses modals, Money uses an inline composer) is a structural change larger than a styling pass. **Do not convert Money's composer to a modal in this task.** Assess it and report whether the remaining inconsistency is acceptable now that Money uses the shared vocabulary, so it can be scheduled separately if not.

- [ ] **Step 7: Verify**

`npm run check` → 0 errors, no unused-selector warnings in touched files. `npm test` → 31 pass. `npm run build` → succeeds.

- [ ] **Step 8: Commit**

```bash
git add src/lib/components src/lib/features
git commit -m "fix(ui): unify controls, affordances and save behaviour across screens"
```

---

## Self-Review

**Coverage:** all 27 findings map to a task — A:1 · B:2,3,4,26 · C:5,6,7,27 · D:8,9,10,11,18,21 · E:12,13,14,15,17 · F:16 · G:19,20,22,23,24,25.

**Placeholder scan:** no TBD/TODO. Each step names the finding it closes, the file, and the fix. Where a fix has two acceptable shapes the task requires choosing one and justifying it, rather than leaving it open.

**Ordering:** blocking tasks (A, B, C) run first so the app stops losing work before cosmetic work begins. D and E remove danger and dishonesty. F adds a missing capability. G is last because it is the largest and purely consistency.

## Open Risks

- **Task B moves markup between components** (form footers into a dialog slot). The buttons' `disabled` expressions reference form-local state — if a footer moves out of the form's scope, those bindings break. The task must keep each footer inside its own form component and only change where that component renders it.
- **Task C's dirty-tracking** could make dialogs annoying if it fires on untouched forms. Confirm only when something actually changed.
- **Task E deletes the chart cards** that the Cycle 2 chart plan expects to fill. Cycle 2 must re-add them alongside real charts; note this in that plan when it is written.
- **Task F may find the backend has no delete commands**, in which case part of the finding cannot be closed frontend-only. That outcome must be reported, not worked around.
