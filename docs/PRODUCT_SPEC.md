# RudeSync v1 Product Specification

Status: Approved scope, implementation-ready  
Primary platform: Windows 11  
Future platform: macOS  
Product type: Offline-first, single-user desktop application

## 1. Product goal

RudeSync is a lightweight personal operations tracker for a project-based web developer. It opens at Windows login and makes this daily loop fast:

1. Review today's work and due items.
2. Complete tasks and optionally turn project tasks into completed-work records.
3. Record project progress, invoices, and client payments.
4. Check personal loan installments.
5. Review the current week and plan upcoming days.

The UI uses a minimal dark developer aesthetic: near-black, subtly green-tinted surfaces; restrained emerald accents; off-white text; and amber/red reserved for warnings. It uses system fonts/icons, short functional transitions, and no decorative gradients, blur-heavy effects, animated backgrounds, or unnecessary charts.

## 2. Goals and non-goals

### Goals

- Fast capture and daily review of tasks.
- Manual records of completed project work, without time tracking.
- Clear organization of clients, fixed-price projects, milestone invoices, partial client payments, and received earnings.
- Printable and PDF-exportable invoices.
- A separate personal-loan due-date tracker.
- Reliable native Windows reminders while RudeSync is running or in the system tray.
- User-selected, automatic local backups.
- A small installer/runtime footprint and negligible idle CPU use.
- Data structures that do not block a future macOS build or optional cloud sync.

### Non-goals for v1

- Hourly rates, timers, timesheets, or employee/team features.
- Full bookkeeping, bank feeds, accounting reconciliation, currency conversion, expenses, subscriptions, savings goals, or tax filing.
- Client loans or accounts receivable outside project invoices.
- Loan principal, interest, balances, amortization, or partial installment amounts.
- File attachments other than an invoice-profile logo; work records use text and URLs.
- Cloud sync, collaboration, user accounts, PIN/password protection, or app-level database encryption.
- Mobile or browser-based versions.

## 3. Product and technical constraints

- Package as a standalone Windows desktop application; no separately managed web server.
- Preferred implementation: Tauri 2, Svelte, TypeScript, and SQLite. Platform-specific behavior must be isolated so a future macOS build is feasible.
- SQLite is the local source of truth. All persisted entities use stable UUIDs plus `created_at`, `updated_at`, and nullable `deleted_at` timestamps to support future sync.
- Store money as integer minor units and an ISO 4217 currency code; never use floating-point values for financial calculations.
- Display dates as `MMMM d, yyyy`, for example `July 23, 2026`. Store timestamps in UTC and render them in the operating system's local time zone.
- USD is the default currency. A client may use another currency, inherited by new projects and invoices. v1 performs no exchange-rate conversion or cross-currency totals.
- Performance targets on a typical Windows 11 development PC:
  - Main window usable within 2 seconds of launch.
  - Idle CPU settles below 1%.
  - Idle working set target is 150 MB or less.
  - No polling or animation loop while idle.

## 4. Information architecture

The primary navigation contains:

1. **Today**
   - Overdue tasks
   - Today's planned tasks
   - Quick add
   - Upcoming loan installments
   - Invoice warnings
   - Next seven days
2. **Tasks**
   - Inbox
   - Today
   - Upcoming
   - Recurring
   - Completed
   - Categories
3. **Work**
   - Clients
   - Projects
   - Completed-work history
   - Project detail acts as the container for its overview, work records, milestones, invoices, and payments
4. **Money**
   - Invoices
   - Received earnings
   - Personal loans
5. **Review**
   - Current-week completed tasks
   - Completed work
   - Invoiced and received amounts, separated by currency
   - Loan installments paid and upcoming
   - Next-week planning
6. **Settings**
   - Invoice profile
   - Notification defaults
   - Startup/tray behavior
   - Backup folder, retention, manual backup, and restore
   - Appearance

## 5. Core workflows and rules

### 5.1 Startup and tray behavior

- RudeSync registers for launch at Windows login after the user enables the default-on setting during first-run setup.
- An automatic launch opens the main window on **Today**.
- Clicking the window close button hides RudeSync to the system tray; it does not terminate reminder processing.
- The tray menu provides **Open RudeSync** and **Quit**. Only **Quit** fully exits the application.
- On ordinary manual launch, RudeSync opens or focuses the existing instance; multiple running instances are not allowed.

### 5.2 Tasks

A task has:

- Required title.
- Optional notes and URLs.
- Status: open or completed, with completion timestamp.
- Priority: none, low, medium, high, or urgent.
- Optional category.
- Optional planned work date.
- Optional deadline date and time, separate from the planned date.
- Optional exact reminder date and time.
- Optional recurrence.
- Zero or more one-level subtasks.
- Optional client and project relationship.

Rules:

- The planned work date controls placement in Today/Upcoming; the deadline controls overdue state. A task may have either or both.
- If both dates exist, the deadline cannot precede the planned work date.
- Supported v1 recurrence patterns are daily, weekdays, weekly on selected days, monthly, and a custom interval in days/weeks/months.
- Completing a recurring occurrence keeps its history and generates the next occurrence from the recurrence rule.
- Completing a project-linked task offers **Record as completed work**. If accepted, RudeSync pre-fills a new work record from the task without duplicating it automatically.
- Completing a parent task does not silently complete unfinished subtasks; confirmation is required.
- Quick Add requires only a title. All other fields may be added later.

### 5.3 Clients, projects, and completed work

A client contains a display name, optional billing/contact details, and a default currency. New clients default to USD.

A project:

- Belongs to one client.
- Is a container for its milestones, tasks, completed-work records, invoices, and payment records.
- Has a title, fixed project value, currency inherited from its client, status, optional dates, notes, and URLs.
- Has no hourly rate or time entries.

New projects suggest two editable milestones:

- Kickoff/deposit: 50%.
- Completion: 50%.

The user may change the labels, percentages, values, and number of milestones before invoices are issued. Percentage-based milestones must total 100% of the project value. Editing the project value updates unissued milestone amounts but never changes an issued invoice.

A completed-work record contains a completion date, title, optional notes, optional URLs, and optional client/project links. It may be entered manually or pre-filled from a completed task.

### 5.4 Invoices and earnings

Invoices belong to a project, and a project can contain multiple invoices. The default 50% kickoff and 50% completion invoices are separate invoices within the same project container.

An invoice contains:

- Immutable UUID.
- Human-readable invoice number.
- Client and project snapshot details.
- Issue date and due date.
- Currency.
- One or more description/quantity/unit-price line items.
- Optional invoice-level discount: fixed amount or percentage.
- Optional tax percentage.
- Notes and payment instructions.
- Status: draft, issued, partially paid, paid, overdue, or void.
- Zero or more payment records, each with amount, received date, and optional note/reference.

Rules:

- Number format is `INV-YYYY-MM-DD-####`, for example `INV-2026-07-23-0001`.
- `####` is a zero-padded, collision-safe sequence starting at `0001` for each issue date.
- Drafts receive their number from the selected issue date. Changing a draft's issue date regenerates the number. Once issued, the number and issue date are locked.
- Due terms are user-selectable per invoice: due immediately, 7 days, 14 days, 30 days, or a custom due date. “Due immediately” sets the due date equal to the issue date.
- The default due term is configurable in Settings; it never prevents choosing another term on an invoice.
- Calculation order is: line-item subtotal, then discount, then tax on the discounted subtotal, then invoice total.
- Percentage inputs are stored with sufficient decimal precision; displayed monetary totals round to the currency's minor unit.
- Multiple payments may be recorded against one invoice. A positive balance with at least one payment is **partially paid**; a zero balance is **paid**. Overpayment requires confirmation and is retained as an explicit credit on that invoice.
- An unpaid issued invoice becomes **overdue** after its due date. A partially paid invoice can also be overdue.
- Only payment records count as received earnings. Project value and issued invoice totals are expected/invoiced amounts, not earnings.
- Voiding an invoice preserves its number and audit history and excludes its unpaid balance from active totals.
- Totals are grouped by currency; currencies are never combined into a converted grand total.

### 5.5 Invoice profile, printing, and PDF

Settings contains one editable invoice profile with:

- Personal or business name.
- Address.
- Email and optional contact details.
- Optional logo.
- Default payment instructions.

The invoice preview and exported PDF must use the invoice's stored snapshot, not mutable current client/project data. Printing uses the same layout as PDF export.

PDF requirements:

- Standard A4 layout with sensible margins.
- Legible in color and grayscale.
- Includes invoice identity, issue/due dates, sender and client details, project, line items, subtotal, discount, tax, total, amount paid, balance due, notes, and payment instructions.
- Supports a multi-page line-item table without clipped content or repeated/overlapping totals.
- The user chooses the output location.

### 5.6 Personal loans

Personal loans are wholly separate from clients, projects, invoices, and earnings. A loan represents money the user borrowed from a lender/operator.

A loan contains:

- Lender/operator name.
- Optional description.
- Loan date.
- Explicit first payment date.
- Number of installments, except that a custom schedule may derive this from its entered dates.
- Frequency.
- Generated installment due dates.
- Per-installment status: unpaid or paid.
- Timestamp/date marked paid.

v1 intentionally stores no installment amount, principal, interest, balance, or partial-payment state. An installment may be marked paid before its due date.

Supported frequencies:

1. **Monthly:** same day of each subsequent month, starting on the explicit first payment date.
2. **Weekly:** every 7 days from the first payment date.
3. **Every two weeks:** every 14 days from the first payment date.
4. **Twice monthly:** two user-selected days of the month; generate chronological occurrences on those days beginning with the first payment date.
5. **Custom:** user enters each due date explicitly.

Date rules:

- The loan date never implies the first payment date. Example: a loan dated July 13 with first payment August 13 begins on August 13.
- If a selected monthly or twice-monthly day does not exist in a month, use that month's final calendar day.
- For twice-monthly schedules, the two selected day numbers must differ, and the explicit first payment date must match one generated occurrence.
- Generated dates are previewed before saving.
- The user may edit any generated installment date after generation; editing one occurrence does not shift later occurrences.
- Marking an installment paid records the current date by default, which the user may edit. Reopening it clears the paid date after confirmation.

### 5.7 Notifications

Notifications use the operating system's local time. The default reminder schedule is:

- Task: at its user-selected reminder time.
- Loan installment: 7 days before, 1 day before, and at 9:00 AM on its due date.
- Invoice: 3 days before and at 9:00 AM on its due date.

Rules:

- Notification defaults are editable globally and may be overridden or disabled per record.
- Marking a task/loan installment paid or an invoice paid/void cancels future reminders for that item.
- A reminder that became due while RudeSync was not running appears once on the next launch if the item remains actionable; duplicate catch-up notifications are suppressed.
- Today always shows overdue actionable items even if their notification was dismissed.
- Native reminders are guaranteed only while RudeSync is open or resident in the system tray.

### 5.8 Backup, export, and restore

- First-run setup prompts the user to choose a backup folder; the folder can be changed later.
- RudeSync creates one automatic backup per local calendar day when the application is running.
- Automatic backups use SQLite's consistent online-backup mechanism, not a raw copy of an open database.
- Automatic filename format: `RudeSync-backup-YYYY-MM-DD-HHmmss.db`.
- Retain the latest 30 automatic backups and delete only older automatic backups from the configured RudeSync backup set.
- Manual backup is always available, lets the user choose a destination, and is never deleted by automatic retention.
- Restore validates the backup, creates a pre-restore safety backup, replaces the local data atomically, and relaunches the app.
- A failed backup or unavailable folder produces a visible, non-blocking warning and retries on the next launch/day; it must not damage the active database.
- CSV export is available for tasks, completed work, invoices/payments, and loan schedules. PDF export is available for invoices.

## 6. Offline-first and future sync

- Every v1 feature works without internet access.
- No analytics, network request, or cloud dependency is required for normal use.
- Local writes commit immediately and transactionally.
- UUIDs, UTC timestamps, soft deletion, and explicit parent relationships are required so optional future sync can be added without migrating away from local-first ownership.
- Future sync must be opt-in and must not become a prerequisite for opening or using RudeSync.

## 7. First-run defaults

- Theme: dark with emerald accent.
- Currency: USD.
- Date display: `MMMM d, yyyy`.
- Startup at Windows login: enabled after user confirmation.
- Close button: hide to tray.
- Invoice milestones: 50% kickoff and 50% completion.
- Loan due-date reminder time: 9:00 AM local time.
- Invoice due-date reminder time: 9:00 AM local time.
- Automatic backup: daily; retain 30.

The first-run flow collects the backup folder and invoice profile. The invoice profile may be completed later; it is required before issuing or exporting the first invoice.

## 8. Acceptance criteria

RudeSync v1 is accepted when all of the following pass on Windows 11:

### Shell and daily use

- Launch at login opens one RudeSync instance on Today.
- Closing the window leaves the tray process active; **Quit** exits it.
- Today correctly separates overdue, today, upcoming-loan, invoice-warning, and next-seven-day items.
- A task can be quick-added, scheduled, reminded, completed, recurred, categorized, prioritized, and broken into subtasks.
- Planned date and deadline behave independently, and overdue state follows the deadline.
- Completing a linked task can pre-fill one completed-work record.

### Projects and money

- A USD client and fixed-price project can be created with editable 50/50 kickoff/completion milestones.
- Both milestone invoices remain visible inside the same project.
- Invoice terms support immediate, 7, 14, 30 days, and a custom due date.
- Invoice numbers exactly match `INV-YYYY-MM-DD-####` and do not collide.
- Discounts and tax calculate in the documented order.
- Two or more payments can move an invoice from issued to partially paid to paid.
- Received earnings equal recorded payments, not project or invoice values.
- Different client currencies remain separate and are not converted or summed together.
- Print preview and the exported PDF contain matching totals and render correctly across multiple pages.

### Personal loans

- A loan's first installment can begin after its loan date.
- Monthly, weekly, every-two-weeks, twice-monthly, and custom schedules generate the documented dates.
- A 29th, 30th, or 31st monthly occurrence falls on the final day when needed.
- An installment can be marked paid/unpaid and stores/clears its paid date correctly.
- No personal-loan record appears as client/project/invoice data or received earnings.

### Reliability

- Default native notifications fire at the documented times while the app is open or in the tray, and resolved items do not continue notifying.
- A consistent daily backup is written to the chosen folder; only the latest 30 automatic backups are retained.
- A manual backup survives automatic retention and can restore all application data.
- Core workflows work with network access disabled.
- Performance targets in Section 3 are measured in a release build, with any miss treated as a release blocker unless explicitly waived.

## 9. Deferred decisions

- Optional cloud-sync provider and conflict-resolution UI.
- macOS packaging, autostart, tray, and notification validation.
- PIN/password protection and app-level encryption.
- Expenses, subscriptions, savings goals, and broader accounting.
- The first day of the week is not yet confirmed; the implementation must keep it configurable rather than hard-code a locale assumption.
