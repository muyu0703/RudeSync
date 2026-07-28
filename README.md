# RudeSync

RudeSync is a lightweight, offline-first personal operations tracker for
Windows 11. It keeps daily tasks, client work, milestone invoices, received
payments, and personal loan due dates in one private desktop application.

The application is built with Tauri 2, Svelte, TypeScript, Rust, and SQLite. It
does not require an account, a separately managed web server, or a cloud
connection. Windows is the supported platform for this release; the
architecture preserves a future path to macOS.

It is free and open source under the [MIT License](LICENSE).

## Screenshots

> Add screenshots here before sharing the repository. Suggested set: **Today**
> (the dashboard with its stat row), **Work** (a project with milestone
> progress), **Money** (invoices), and the floating task widget over another
> window. Put the files in `docs/screenshots/` and reference them as
> `![Today](docs/screenshots/today.png)`.

## Features

- **Today and tasks:** quick capture, planned dates, separate deadlines,
  priorities, categories, reminders, recurrence, subtasks, project links, and
  Today/Upcoming/Completed views.
- **Clients and projects:** client records, per-client currencies, fixed-price
  project containers, and lightweight completed-work records with notes and
  URLs. Records created by mistake can be deleted, with financial history
  protected — a project that already has invoices refuses deletion rather than
  quietly taking them with it.
- **Flexible milestone plans:** a project bills through any number of
  explicitly-priced milestones, seeded from templates (kickoff + completion,
  even weekly, phase-by-phase, or custom). Out-of-scope work is just another
  milestone added at any time, so the project total grows with it. Each
  milestone tracks its own status — not invoiced, invoiced, paid — and the
  project shows what is left to bill.
- **Floating task widget:** an always-on-top window that lists open tasks, with
  check-off, quick add, subtask toggling and inline editing. It syncs live with
  the main window and remembers where you put it.
- **Invoices and earnings:** date-based invoice numbers such as
  `INV-2026-07-23-0001`, selectable payment terms, milestone billing, line
  items, editable drafts, issue/void history, fixed or percentage discounts,
  tax, partial payments, received earnings, print layouts, and A4 PDF export.
- **Personal loans:** loan schedules kept completely separate from client
  money, with monthly, weekly, every-two-weeks, twice-monthly, and custom due
  dates plus paid/unpaid installment tracking and per-occurrence date
  corrections.
- **Weekly review:** completed tasks and work, collected payments, upcoming
  obligations, and next-week planning.
- **Desktop behavior:** single-instance launch, optional launch at Windows
  sign-in, system-tray residency, and native due-date notifications.
- **Local recovery:** automatic and on-demand consistent SQLite backups to a
  folder you choose.
- **Portable exports:** CSV exports for tasks, completed work,
  invoices/payments, and personal-loan schedules.

The full product rules and accepted scope are documented in
[docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md).

## Install on Windows 11

The release build is a 64-bit NSIS installer named
`RudeSync_0.1.7_x64-setup.exe`. A locally built installer is written to:

```text
src-tauri\target\release\bundle\nsis\RudeSync_0.1.7_x64-setup.exe
```

To install:

1. Close any older RudeSync instance from its tray icon.
2. Run `RudeSync_0.1.7_x64-setup.exe`.
3. Complete the installer, then open RudeSync from the Start menu.
4. Open **Settings** and configure your invoice profile, backup folder,
   startup preference, and notification preference.

Windows 11 normally includes the Microsoft Edge WebView2 runtime used by
Tauri. The installer is configured to download WebView2 when it is missing, so
that uncommon case requires an internet connection.

### Unsigned installer and SmartScreen

The current installer is not code signed. Microsoft Defender SmartScreen may
therefore show **Windows protected your PC** even for a valid local build.

Only continue when you built the installer from this repository or received it
through a source you trust. In the SmartScreen window, select **More info**,
verify that the app is RudeSync, and then select **Run anyway**. Do not bypass
the warning for an installer from an unknown source.

## How to use it

### First run

Open **Settings** first and fill in your invoice profile — your name, email and
address appear on every invoice you generate, so setting them once saves
editing them later. While you are there, choose a backup folder; RudeSync keeps
one consistent copy per day and never touches unrelated files in it.

Then work outside-in: a **client** owns **projects**, a project owns
**milestones**, and a milestone is what you actually invoice. Nothing else can
be billed, so this order matters.

### 1. Set up a client and a project

In **Work**, add a client. Each client has its own currency, and RudeSync never
adds two currencies together — a USD balance and a EUR balance stay two
separate numbers everywhere in the app.

Create a project under that client with its total fixed price, then choose how
it bills. The milestone plan is seeded from a template and every milestone
carries its own explicit amount:

| Template | Produces |
| --- | --- |
| Kickoff + completion | Two milestones, an even split (odd amounts give the remainder to completion, so the parts always re-sum to the total) |
| Even weekly | One milestone per week for the number of weeks you set |
| Phase by phase | A milestone per phase |
| Custom | Start empty and add your own |

**Out-of-scope work is just another milestone.** When a client asks for
something that was not in the original quote, add a milestone for it with its
price — the project total grows to match, and the new milestone is invoiced
like any other. You do not need to edit the original agreement or fake a line
item.

Each milestone tracks its own state — *not invoiced*, *invoiced*, *paid* — and
the project shows what is still left to bill.

### 2. Run your day

**Today** is the daily surface. Type into the quick-add box for a fast capture,
or press `Ctrl+N` for the full editor when a task needs a planned date, a
separate deadline, a priority, a category, a project link, a reminder,
recurrence or subtasks.

Planned date and due date are deliberately different things: the planned date
is when you intend to *do* it, the deadline is when it is *due*. A task can be
planned for today and due next week.

**Tasks** holds everything, with filters for Open, Inbox, Today, Upcoming,
Recurring, Categories and Completed. `Ctrl+K` searches.

For work that happens outside the app, use **Pop out widget** — a small
always-on-top window that lists your open tasks over whatever you are working
in. You can check tasks off, add new ones and toggle subtasks from it, and it
stays in sync with the main window and remembers where you put it.

As you finish real deliverables, record them as **completed work** entries
against the project, with notes and links. These are what you review at the end
of the week.

### 3. Bill and get paid

**Money** has three tabs.

**Invoices** — create an invoice from a project milestone. RudeSync assigns a
date-based number such as `INV-2026-07-23-0001`, applies your chosen payment
terms to set the due date, and pulls in the milestone amount as a line item.
Add more line items, a fixed or percentage discount, and tax if you charge it.

An invoice stays a **draft** until you issue it, and drafts are freely
editable. Once issued it is part of your financial history: correct it by
voiding and reissuing rather than by silently rewriting it. Print it or export
it as an A4 PDF from the invoice itself.

**Earnings** — record each payment as it arrives, including partial payments;
the balance due updates as you go. The chart here shows what you billed against
what you have collected, grouped by the month the invoice was *issued*, so each
bar answers "of what I billed that month, how much has come in?"

**Loans** — your own personal obligations, kept completely separate from client
money and never mixed into any client total. Schedules support monthly, weekly,
every-two-weeks, twice-monthly and custom due dates, and you can mark
individual installments paid or correct a single occurrence's date without
disturbing the rest of the schedule.

### 4. Close the week

**Review** collects what you completed, the work you recorded, what you
collected, and what is coming up, then lets you plan the next week. Every
figure on the page describes the same Monday-to-Sunday week.

### Deleting things

Deletion never destroys financial history. A client or project that already has
invoices refuses to be deleted and tells you why, rather than quietly taking
your billing records with it. Remove the invoices first if you genuinely mean
to.

### Shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+N` | Open the task editor |
| `Ctrl+K` | Search tasks |
| `Ctrl+1` through `Ctrl+5` | Open Today, Tasks, Work, Money, or Review |

## Startup and system tray

**Open when Windows starts** and **Close to system tray** are configurable in
Settings and enabled by default in the application preferences.

- Closing the main window with **X** hides it when close-to-tray is enabled.
  RudeSync remains available for due-date checks and scheduled backups.
- Select **Open RudeSync** from the tray menu to restore the window.
- Select **Quit** from the tray menu to stop RudeSync completely.
- Opening RudeSync again focuses the existing instance instead of starting a
  second copy.

If reminders or backups must continue, leave RudeSync running in the tray
rather than selecting Quit.

## Local data and backups

The desktop application stores its source-of-truth SQLite database in the
Windows application-data directory. With the current application identifier,
the usual location is:

```text
%APPDATA%\com.rudesync.desktop\rudesync.sqlite3
```

The exact location is chosen by Windows/Tauri. SQLite may create temporary
`-wal` and `-shm` files beside the database while RudeSync is running. Do not
edit, move, or replace these files while the app is open or resident in the
tray.

In **Settings → Local backups**:

- Choose an existing, dedicated backup folder.
- Enable or pause automatic backups.
- Set how many recognized RudeSync backups to retain; the default is 30.
- Select **Create manual backup** to save an exempt copy anywhere you choose.

When enabled and a folder is configured, RudeSync creates one consistent
automatic SQLite backup per local calendar day while it is running. A failed
automatic attempt is shown in the app and retried after a one-hour backoff.
Backup files are named `RudeSync-backup-YYYY-MM-DD-HHmmss.db`. Retention
cleanup removes only older files matching RudeSync's generated automatic
backup pattern; unrelated files in the selected folder are left alone.

Manual copies use a separate filename pattern and are never removed by
automatic retention. **Restore from backup** validates the selected database,
creates a pre-restore safety copy, swaps the database on the same volume, and
reloads the application. If the restored database cannot be opened, RudeSync
rolls back to the original database and reports the error.

RudeSync has no cloud sync and no application-level database encryption.
Protect the Windows account and backup folder appropriately. Browser-based
development uses browser local storage and is separate from the desktop SQLite
database.

## Development

### Prerequisites

- Node.js 20 or newer
- Rust stable and Cargo; the crate declares Rust 1.77.2 as its minimum version
- Microsoft C++ Build Tools with the Desktop development with C++ workload
- Windows SDK and Microsoft Edge WebView2

Install JavaScript dependencies:

```powershell
npm.cmd install
```

Run the browser frontend preview:

```powershell
npm.cmd run dev
```

Run the full Tauri desktop application:

```powershell
npm.cmd run tauri dev
```

Run frontend type and Svelte checks:

```powershell
npm.cmd run check
```

Run TypeScript domain tests:

```powershell
npm.cmd test
```

Run Rust tests:

```powershell
cargo test --manifest-path .\src-tauri\Cargo.toml
```

Build the frontend only:

```powershell
npm.cmd run build
```

Build the Windows release executable and installer:

```powershell
npm.cmd run tauri build
```

Generated frontend and Rust build outputs live in `dist\` and
`src-tauri\target\` respectively and are intentionally ignored by Git.

## Customising

RudeSync is built to be re-skinned without touching feature code. The entire
visual language lives in the `:root` block of [`src/app.css`](src/app.css).

**Colour.** The app ships a dark "deep forest" palette. Change these and the
whole interface follows:

```css
--surface-window: #070d0a;   /* app background        */
--surface-content: #0d1712;  /* cards and panels      */
--surface-raised: #142019;   /* rows nested in a card */
--surface-overlay: #1a2820;  /* dialogs and sheets    */
--accent: #3ddc84;           /* positive, completed   */
--danger: #ff6961;           /* overdue               */
--amber: #e3b341;            /* due soon, outstanding */
--viz-fill: var(--accent);   /* chart bars            */
--viz-track: #33403a;        /* the unfilled remainder*/
--viz-grid: var(--separator);/* chart gridlines       */
```

Keep the ground dark enough that `--accent` still reads as a signal. If the
background drifts toward the accent hue, the accent stops meaning anything and
the interface flattens.

**Charts deliberately use one colour, not a palette.** Each chart plots a single
hue against a neutral track — the "emphasis" form — rather than assigning a
colour per series. That is a measured choice, not a stylistic one. Against
`--surface-content`, `--viz-fill` lands at 10.25:1 contrast and separates from
`--viz-track` by ΔE 47.9 under simulated deuteranopia; the obvious alternative
of green bars beside amber bars managed only 7.2, inside the range where
red–green colourblind readers start confusing the two. If you re-skin
`--viz-fill`, keep it far from `--viz-track` in *lightness*, not just in hue —
hue is the channel colourblind readers lose first.

**Type.** `--font-ui` prefers the system font on Apple platforms and falls back
to the bundled Inter elsewhere, so the app looks native without a network
request. Sizes come from a fixed whole-pixel scale (`--text-11` … `--text-28`)
and four weights.

**Shape, spacing, motion.** Four radii (`--radius-control`, `--radius-panel`,
`--radius-sheet`, `--radius-pill`), a 4px spacing scale (`--space-1` …
`--space-10`), and `--duration` / `--ease` for transitions.

**Structure.** Screens are composed from small primitives in
`src/lib/components/` — `Card`, `StatCard`, `StatRow`, `SectionHeader`,
`MeterBar`, `BarChart`. Restyling a primitive restyles every screen at once,
which is the point: consistency is structural rather than a matter of
discipline.

`BarChart` draws either plain bars or part-to-whole meters from the same data
shape: a point with a `total` renders a filled track, a point without one
renders a plain bar. The series themselves are pure functions in
[`src/lib/features/dashboard/chartSeries.ts`](src/lib/features/dashboard/chartSeries.ts),
kept free of Svelte and of the clock so they can be unit tested directly.

Two rules worth keeping if you fork it: colour should never be the only carrier
of meaning (every status colour is paired with a text label), and every
animation duration should pass through `motionDuration()` in
[`src/lib/motion.ts`](src/lib/motion.ts) so the reduced-motion preference is
honoured. See [CONTRIBUTING.md](CONTRIBUTING.md) for the full rationale.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) — it
covers the constraints that shape the codebase (offline-first, integer money,
soft deletion, and the design system) and the checks a pull request must pass.

## License

Released under the [MIT License](LICENSE). You may use, modify and distribute
it freely, including commercially, provided the copyright notice and licence
text are retained.
