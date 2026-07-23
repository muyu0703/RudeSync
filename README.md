# RudeSync

RudeSync is a lightweight, offline-first personal operations tracker for
Windows 11. It keeps daily tasks, client work, milestone invoices, received
payments, and personal loan due dates in one private desktop application.

The application is built with Tauri 2, Svelte, TypeScript, Rust, and SQLite. It
does not require an account, a separately managed web server, or a cloud
connection. Windows is the supported platform for this release; the
architecture preserves a future path to macOS.

## Features

- **Today and tasks:** quick capture, planned dates, separate deadlines,
  priorities, categories, reminders, recurrence, subtasks, project links, and
  Today/Upcoming/Completed views.
- **Clients and projects:** client records, per-client currencies, fixed-price
  project containers, default 50/50 kickoff and completion milestones, and
  lightweight completed-work records with notes and URLs.
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
`RudeSync_0.1.0_x64-setup.exe`. A locally built installer is written to:

```text
src-tauri\target\release\bundle\nsis\RudeSync_0.1.0_x64-setup.exe
```

To install:

1. Close any older RudeSync instance from its tray icon.
2. Run `RudeSync_0.1.0_x64-setup.exe`.
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

## Daily use

A practical first workflow is:

1. Add clients, then create their fixed-price projects in **Work**.
2. Add today's tasks from **Today** or press `Ctrl+N` for the full task editor.
3. Complete tasks and record shipped results as completed work.
4. Create milestone invoices in **Money**, record each client payment, and
   print or export invoices as PDF.
5. Add your personal loan schedules and mark installments paid as they occur.
6. Use **Review** to close the week and plan the next one.

Useful shortcuts:

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

RudeSync 0.1.0 has no cloud sync and no application-level database encryption.
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
