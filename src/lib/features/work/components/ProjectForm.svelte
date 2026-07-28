<script lang="ts">
  import type {
    Client,
    CreateProjectInput,
    Project,
    ProjectMilestone,
    ProjectStatus,
  } from "../types";
  import {
    evenWeekly,
    kickoffCompletion,
    phases,
    planTotalMinor,
  } from "../milestonePlans";
  import WorkDialog from "./WorkDialog.svelte";

  export let dialogTitle: string;
  export let dialogDescription = "";
  export let clients: Client[] = [];
  export let initialClientId: string | null = null;
  export let project: Project | null = null;
  export let busy = false;
  export let onSave: (input: CreateProjectInput) => void | Promise<void>;
  export let onCancel: () => void;

  // `amountText` is the raw text the user is typing. Keeping it separate from
  // `amountMinor` means the field is never rewritten mid-edit (a controlled
  // `(amountMinor / 100).toFixed(2)` value turns "5" into "5.00" and pushes
  // the caret past the decimals). Minor units are re-derived on commit.
  type MilestoneRow = ProjectMilestone & { key: string; amountText: string };
  type TemplateOption =
    | "custom"
    | "kickoff-completion"
    | "even-weekly"
    | "phases";

  function uid(): string {
    return typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `ms-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  function amountMinorFromText(raw: string): number {
    return Math.max(0, Math.round((Number(raw) || 0) * 100));
  }

  function amountTextFromMinor(amountMinor: number): string {
    return (amountMinor / 100).toFixed(2);
  }

  function toRow(milestone: ProjectMilestone): MilestoneRow {
    return {
      ...milestone,
      key: milestone.id ?? uid(),
      amountText: amountTextFromMinor(milestone.amountMinor),
    };
  }

  function isBilled(milestone: { status?: ProjectMilestone["status"] }): boolean {
    return milestone.status === "invoiced" || milestone.status === "paid";
  }

  let clientId =
    project?.clientId ?? initialClientId ?? clients[0]?.id ?? "";
  let name = project?.name ?? "";
  let description = project?.description ?? "";
  let urlsText = project?.urls.join("\n") ?? "";
  let status: ProjectStatus = project?.status ?? "active";
  let quotedValue = project
    ? (project.quotedTotalMinor / 100).toFixed(2)
    : "";
  let startDate = project?.startDate ?? "";
  let dueDate = project?.dueDate ?? "";

  let milestones: MilestoneRow[] = (project?.milestones ?? [])
    .slice()
    .sort((a, b) => a.sortOrder - b.sortOrder)
    .map(toRow);

  let template: TemplateOption = "custom";
  let weeklyWeeks = 4;
  let weeklyAmount = "";
  let confirmingTemplate = false;

  // A fresh ProjectForm instance is created each time the dialog opens (see
  // WorkView's `{#if dialog === ...}`), so this snapshot taken at
  // construction is the form's true starting point. Template/UI-only state
  // (template, weeklyWeeks, weeklyAmount, confirmingTemplate) is deliberately
  // excluded — it never reaches `onSave`.
  const initialSnapshot = JSON.stringify({
    clientId,
    name,
    description,
    urlsText,
    status,
    quotedValue,
    startDate,
    dueDate,
    milestones,
  });
  $: dirty =
    JSON.stringify({
      clientId,
      name,
      description,
      urlsText,
      status,
      quotedValue,
      startDate,
      dueDate,
      milestones,
    }) !== initialSnapshot;

  $: if (!clientId && clients.length) clientId = clients[0].id;
  $: selectedClient = clients.find((client) => client.id === clientId);
  $: currency =
    project && clientId === project.clientId
      ? project.currency
      : selectedClient?.currency ?? "USD";
  $: amountMinor = Math.round((Number(quotedValue) || 0) * 100);
  $: planTotal = planTotalMinor(milestones);
  $: hasPersistedMilestones = milestones.some((row) => Boolean(row.id));
  $: billedMilestoneCount = milestones.filter(isBilled).length;
  $: replaceableMilestoneCount = milestones.length - billedMilestoneCount;
  $: datesInvalid = Boolean(startDate && dueDate && dueDate < startDate);
  $: urls = urlsFromText();
  $: urlsInvalid = urls.some(invalidUrl);
  $: milestoneLabelsInvalid = milestones.some((row) => !row.label.trim());
  // The project total is the sum of its milestones; `quotedValue` is only a
  // legacy reference figure that seeds the Kickoff + Completion template, so
  // it must not block submitting the form.
  $: formInvalid =
    !clientId ||
    !name.trim() ||
    amountMinor < 0 ||
    datesInvalid ||
    urlsInvalid ||
    milestoneLabelsInvalid;

  function urlsFromText(): string[] {
    return urlsText
      .split(/\r?\n/)
      .map((url) => url.trim())
      .filter(Boolean);
  }

  function invalidUrl(url: string): boolean {
    try {
      const parsed = new URL(url);
      return parsed.protocol !== "https:" && parsed.protocol !== "http:";
    } catch {
      return true;
    }
  }

  function formatMoney(minor: number, currencyCode: string): string {
    try {
      return new Intl.NumberFormat("en-US", {
        style: "currency",
        currency: currencyCode,
        minimumFractionDigits: 0,
        maximumFractionDigits: 2,
      }).format(minor / 100);
    } catch {
      return `${currencyCode} ${(minor / 100).toLocaleString("en-US")}`;
    }
  }

  function computeSeededMilestones(): ProjectMilestone[] {
    switch (template) {
      case "kickoff-completion":
        return kickoffCompletion(amountMinor);
      case "even-weekly": {
        const perWeekMinor = Math.max(
          0,
          Math.round((Number(weeklyAmount) || 0) * 100),
        );
        return evenWeekly(weeklyWeeks, perWeekMinor);
      }
      case "phases":
        return phases(2);
      default:
        return [];
    }
  }

  function applyTemplate(): void {
    // Applying a template wholesale-replaces the plan. If any current
    // milestone is persisted (has an id), require an explicit confirmation
    // first so we never silently discard ids the backend uses to match
    // (and soft-delete) existing milestones.
    if (hasPersistedMilestones && !confirmingTemplate) {
      confirmingTemplate = true;
      return;
    }
    confirmingTemplate = false;

    const seeded = computeSeededMilestones();
    // Invoiced/paid milestones are billed work: never remove them via a
    // template swap. Keep them (with their ids) and append the template
    // rows after them, then re-sequence sortOrder.
    const keptRows = milestones.filter(isBilled);
    milestones = [...keptRows, ...seeded.map(toRow)].map((row, i) => ({
      ...row,
      sortOrder: i,
    }));
  }

  function cancelApplyTemplate(): void {
    confirmingTemplate = false;
  }

  function onTemplateOptionChange(): void {
    confirmingTemplate = false;
  }

  // Called on change/blur, never on every keystroke: the typed text is
  // converted to integer minor units and the field is normalized once the
  // user has finished with it.
  function commitMilestoneAmount(index: number): void {
    milestones = milestones.map((row, i) => {
      if (i !== index) return row;
      const amountMinor = amountMinorFromText(row.amountText);
      return { ...row, amountMinor, amountText: amountTextFromMinor(amountMinor) };
    });
  }

  function moveMilestone(index: number, direction: -1 | 1): void {
    const target = index + direction;
    if (target < 0 || target >= milestones.length) return;
    const next = [...milestones];
    [next[index], next[target]] = [next[target], next[index]];
    milestones = next.map((row, i) => ({ ...row, sortOrder: i }));
  }

  function removeMilestone(index: number): void {
    if (isBilled(milestones[index])) return;
    milestones = milestones
      .filter((_, i) => i !== index)
      .map((row, i) => ({ ...row, sortOrder: i }));
  }

  function addMilestone(): void {
    milestones = [
      ...milestones,
      {
        key: uid(),
        label: "",
        amountMinor: 0,
        amountText: amountTextFromMinor(0),
        kind: "custom",
        sortOrder: milestones.length,
      },
    ];
  }

  function submit(): void {
    if (formInvalid || busy) return;
    // Re-derive from the typed text rather than trusting the last committed
    // value, so submitting straight from a focused field can't drop an edit.
    const submittedMilestones: ProjectMilestone[] = milestones.map(
      ({ key, status: _rowStatus, amountText, ...rest }) => ({
        ...rest,
        amountMinor: amountMinorFromText(amountText),
      }),
    );
    void onSave({
      clientId,
      name,
      description,
      urls,
      status,
      currency,
      quotedTotalMinor: amountMinor,
      milestones: submittedMilestones,
      startDate,
      dueDate,
    });
  }
</script>

<WorkDialog wide title={dialogTitle} description={dialogDescription} {dirty} onClose={onCancel}>
<form id="project-form" on:submit|preventDefault={submit}>
  <div class="field-grid two">
    <label>
      <span class="field-label">Client <b aria-hidden="true">*</b></span>
      <select class="field-input" bind:value={clientId} data-work-autofocus required>
        {#each clients as client (client.id)}
          <option value={client.id}>{client.name} · {client.currency}</option>
        {/each}
      </select>
    </label>
    <label>
      <span class="field-label">Status</span>
      <select class="field-input" bind:value={status}>
        <option value="active">Active</option>
        <option value="draft">Draft</option>
        {#if project}
          <option value="completed">Completed</option>
          <option value="archived">Archived</option>
        {/if}
      </select>
    </label>
  </div>

  <label>
    <span class="field-label">Project name <b aria-hidden="true">*</b></span>
    <input class="field-input" bind:value={name} maxlength="160" required />
  </label>

  <div class="field-grid value-row">
    <label>
      <span class="field-label">Original quote</span>
      <div class="money-input">
        <i>{currency}</i>
        <input
          bind:value={quotedValue}
          type="number"
          inputmode="decimal"
          min="0"
          max="999999999999.99"
          step="0.01"
          placeholder="0.00"
          aria-describedby="project-quote-help"
        />
      </div>
      <small id="project-quote-help" class="field-hint">
        Optional reference · seeds the Kickoff + Completion template. The
        project total is the sum of its milestones.
      </small>
    </label>
    <label>
      <span class="field-label">Start date</span>
      <input class="field-input" bind:value={startDate} type="date" />
    </label>
    <label>
      <span class="field-label">Due date</span>
      <input
        class="field-input"
        bind:value={dueDate}
        min={startDate || undefined}
        type="date"
        aria-invalid={datesInvalid}
      />
    </label>
  </div>
  {#if datesInvalid}
    <p class="field-error" role="alert">The due date cannot be before the start date.</p>
  {/if}

  <label>
    <span class="field-label">Notes / description</span>
    <textarea class="field-textarea" bind:value={description} maxlength="2000" rows="3"></textarea>
  </label>

  <label>
    <span class="field-label">Reference URLs</span>
    <textarea
      class="field-textarea"
      bind:value={urlsText}
      class:invalid={urlsInvalid}
      aria-invalid={urlsInvalid}
      aria-describedby="project-url-help"
      rows="2"
      placeholder={"https://project.example.com\nhttps://github.com/…"}
    ></textarea>
    <small id="project-url-help" class="field-hint">Optional · one complete URL per line</small>
  </label>
  {#if urlsInvalid}
    <p class="field-error" role="alert">Use a complete http:// or https:// URL, one per line.</p>
  {/if}

  <fieldset>
    <legend>
      <span class="field-label">Milestones</span>
      <small>Plan total: {formatMoney(planTotal, currency)}</small>
    </legend>

    <div class="template-row">
      <label>
        <span class="field-label">Template</span>
        <select class="field-input" bind:value={template} on:change={onTemplateOptionChange}>
          <option value="custom">Custom / empty</option>
          <option value="kickoff-completion">Kickoff + Completion</option>
          <option value="even-weekly">Even weekly</option>
          <option value="phases">Phase-by-phase</option>
        </select>
      </label>
      {#if template === "even-weekly"}
        <label class="template-param">
          <span class="field-label">Weeks</span>
          <input class="field-input" bind:value={weeklyWeeks} type="number" min="1" step="1" />
        </label>
        <label class="template-param">
          <span class="field-label">Amount / week</span>
          <div class="money-input">
            <i>{currency}</i>
            <input
              bind:value={weeklyAmount}
              type="number"
              inputmode="decimal"
              min="0"
              step="0.01"
              placeholder="0.00"
            />
          </div>
        </label>
      {/if}
      {#if confirmingTemplate}
        <span class="template-confirm">
          <span class="template-confirm-label">
            {replaceableMilestoneCount > 0
              ? `Replace ${replaceableMilestoneCount} milestone${replaceableMilestoneCount === 1 ? "" : "s"}?`
              : "Add template? Billed milestones will be kept."}
          </span>
          <button class="danger-solid" type="button" on:click={applyTemplate}>
            Replace
          </button>
          <button class="secondary" type="button" on:click={cancelApplyTemplate}>
            Cancel
          </button>
        </span>
      {:else}
        <button class="secondary" type="button" on:click={applyTemplate}>
          Apply template
        </button>
      {/if}
    </div>

    {#each milestones as milestone, index (milestone.key)}
      <div class="milestone">
        <span class="milestone-number">{String(index + 1).padStart(2, "0")}</span>
        <label>
          <span class="field-label">Label</span>
          <input
            class="field-input"
            bind:value={milestone.label}
            maxlength="80"
            placeholder="Milestone label"
            aria-invalid={!milestone.label.trim()}
            required
          />
        </label>
        <label class="amount-field">
          <span class="field-label">Amount</span>
          <div class="money-input">
            <i>{currency}</i>
            <input
              type="number"
              inputmode="decimal"
              min="0"
              step="0.01"
              bind:value={milestone.amountText}
              on:change={() => commitMilestoneAmount(index)}
              on:blur={() => commitMilestoneAmount(index)}
            />
          </div>
        </label>
        <div class="row-actions">
          <button
            type="button"
            disabled={index === 0}
            aria-label="Move milestone up"
            on:click={() => moveMilestone(index, -1)}
          >
            ↑
          </button>
          <button
            type="button"
            disabled={index === milestones.length - 1}
            aria-label="Move milestone down"
            on:click={() => moveMilestone(index, 1)}
          >
            ↓
          </button>
          <button
            type="button"
            class="danger"
            aria-label="Remove milestone"
            disabled={isBilled(milestone)}
            title={isBilled(milestone)
              ? "Billed milestones can't be removed"
              : undefined}
            on:click={() => removeMilestone(index)}
          >
            ✕
          </button>
        </div>
      </div>
    {:else}
      <p class="empty-hint">No milestones yet — pick a template or add one manually.</p>
    {/each}

    <button class="secondary add-milestone" type="button" on:click={addMilestone}>
      + Add milestone
    </button>
    {#if milestoneLabelsInvalid}
      <p class="field-error" role="alert">Give every milestone a label.</p>
    {/if}
  </fieldset>

</form>

<svelte:fragment slot="footer">
  <footer>
    <button class="secondary" type="button" disabled={busy} on:click={onCancel}>Cancel</button>
    <button class="primary" type="submit" form="project-form" disabled={busy || formInvalid}>
      {busy ? "Saving…" : project ? "Save changes" : "Create project"}
    </button>
  </footer>
</svelte:fragment>
</WorkDialog>

<style>
  form {
    display: grid;
    gap: var(--space-4);
  }

  .field-grid {
    display: grid;
    gap: var(--space-3);
  }

  .field-grid.two {
    grid-template-columns: 1fr 160px;
  }

  .value-row {
    grid-template-columns: minmax(170px, 1fr) 145px 145px;
  }

  label {
    display: grid;
    gap: var(--space-2);
    min-width: 0;
  }

  legend > span {
    margin-bottom: 0;
  }

  b {
    color: var(--accent);
    font-weight: inherit;
  }

  select.field-input {
    color-scheme: dark;
  }

  .money-input {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    background: var(--surface-window);
    border: 1px solid var(--separator-strong);
    border-radius: var(--radius-control);
  }

  .money-input:focus-within {
    border-color: var(--accent);
    outline: none;
  }

  .money-input > i {
    padding-left: var(--space-3);
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-style: normal;
    font-weight: var(--weight-semibold);
  }

  .money-input input {
    height: 32px;
    padding: 0 var(--space-3);
    color: var(--text-primary);
    font: inherit;
    font-size: var(--text-13);
    background: transparent;
    border: 0;
    outline: 0;
  }

  fieldset {
    display: grid;
    gap: var(--space-2);
    margin: 1px 0 0;
    padding: var(--space-3);
    background: var(--accent-fill);
    border: 1px solid var(--separator);
    border-radius: var(--radius-panel);
  }

  legend {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0 2px var(--space-2);
  }

  legend small {
    color: var(--accent);
    font-size: var(--text-11);
    font-weight: var(--weight-semibold);
  }

  .template-row {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--space-2);
    margin-bottom: 2px;
  }

  .template-row > label {
    min-width: 150px;
  }

  .template-param {
    width: 120px;
  }

  .template-confirm {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .template-confirm-label {
    color: var(--danger);
    font-size: var(--text-11);
    font-weight: var(--weight-medium);
  }

  .danger-solid {
    color: var(--on-accent);
    background: var(--danger);
    border: 1px solid var(--danger);
  }

  .milestone {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) 150px 92px;
    align-items: end;
    gap: var(--space-2);
  }

  .milestone-number {
    display: grid;
    width: 30px;
    height: 30px;
    margin-bottom: 4px;
    place-items: center;
    color: var(--accent);
    font-size: var(--text-11);
    font-weight: var(--weight-semibold);
    background: var(--accent-fill);
    border-radius: 50%;
  }

  .row-actions {
    display: flex;
    gap: var(--space-1);
  }

  .row-actions button {
    min-height: 32px;
    min-width: 30px;
    padding: 0 var(--space-2);
    color: var(--text-secondary);
    background: transparent;
    border: 1px solid var(--separator-strong);
  }

  .row-actions button.danger {
    color: var(--danger);
    border-color: var(--separator-strong);
  }

  .row-actions button.danger:hover {
    border-color: var(--danger);
  }

  .add-milestone {
    justify-self: start;
  }

  .empty-hint {
    margin: 2px 0;
    color: var(--text-tertiary);
    font-size: var(--text-11);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-1);
    padding-top: var(--space-4);
    border-top: 1px solid var(--separator);
  }

  button {
    min-height: 36px;
    padding: 0 var(--space-4);
    font: inherit;
    font-size: var(--text-11);
    font-weight: var(--weight-semibold);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .secondary {
    color: var(--text-secondary);
    background: transparent;
    border: 1px solid var(--separator-strong);
  }

  .primary {
    color: var(--on-accent);
    background: var(--accent);
    border: 1px solid var(--accent);
  }

  @media (max-width: 650px) {
    .value-row {
      grid-template-columns: 1fr 1fr;
    }

    .value-row > label:first-child {
      grid-column: 1 / -1;
    }

    .milestone {
      grid-template-columns: 32px minmax(0, 1fr) 120px 92px;
    }
  }

  @media (max-width: 480px) {
    .field-grid.two,
    .value-row {
      grid-template-columns: 1fr;
    }

    .value-row > label:first-child {
      grid-column: auto;
    }

    .milestone {
      grid-template-columns: 30px minmax(0, 1fr);
    }

    .amount-field,
    .row-actions {
      grid-column: 2;
    }
  }
</style>
