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

  export let clients: Client[] = [];
  export let initialClientId: string | null = null;
  export let project: Project | null = null;
  export let busy = false;
  export let onSave: (input: CreateProjectInput) => void | Promise<void>;
  export let onCancel: () => void;

  type MilestoneRow = ProjectMilestone & { key: string };
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

  function toRow(milestone: ProjectMilestone): MilestoneRow {
    return { ...milestone, key: milestone.id ?? uid() };
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
  $: formInvalid =
    !clientId ||
    !name.trim() ||
    !quotedValue ||
    amountMinor < 0 ||
    datesInvalid ||
    urlsInvalid;

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

  function setMilestoneAmount(index: number, raw: string): void {
    const amountMinor = Math.max(0, Math.round((Number(raw) || 0) * 100));
    milestones = milestones.map((row, i) =>
      i === index ? { ...row, amountMinor } : row,
    );
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
        kind: "custom",
        sortOrder: milestones.length,
      },
    ];
  }

  function submit(): void {
    if (formInvalid || busy) return;
    const submittedMilestones: ProjectMilestone[] = milestones.map(
      ({ key, status: _rowStatus, ...rest }) => rest,
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

<form on:submit|preventDefault={submit}>
  <div class="field-grid two">
    <label>
      <span>Client <b aria-hidden="true">*</b></span>
      <select bind:value={clientId} data-work-autofocus required>
        {#each clients as client (client.id)}
          <option value={client.id}>{client.name} · {client.currency}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>Status</span>
      <select bind:value={status}>
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
    <span>Project name <b aria-hidden="true">*</b></span>
    <input bind:value={name} maxlength="160" required />
  </label>

  <div class="field-grid value-row">
    <label>
      <span>Fixed project value <b aria-hidden="true">*</b></span>
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
          required
        />
      </div>
    </label>
    <label>
      <span>Start date</span>
      <input bind:value={startDate} type="date" />
    </label>
    <label>
      <span>Due date</span>
      <input
        bind:value={dueDate}
        min={startDate || undefined}
        type="date"
        aria-invalid={datesInvalid}
      />
    </label>
  </div>
  {#if datesInvalid}
    <p class="validation" role="alert">The due date cannot be before the start date.</p>
  {/if}

  <label>
    <span>Notes / description</span>
    <textarea bind:value={description} maxlength="2000" rows="3"></textarea>
  </label>

  <label>
    <span>Reference URLs</span>
    <textarea
      bind:value={urlsText}
      class:invalid={urlsInvalid}
      aria-invalid={urlsInvalid}
      aria-describedby="project-url-help"
      rows="2"
      placeholder={"https://project.example.com\nhttps://github.com/…"}
    ></textarea>
    <small id="project-url-help">Optional · one complete URL per line</small>
  </label>
  {#if urlsInvalid}
    <p class="validation" role="alert">Use a complete http:// or https:// URL, one per line.</p>
  {/if}

  <fieldset>
    <legend>
      <span>Milestones</span>
      <small>Plan total: {formatMoney(planTotal, currency)}</small>
    </legend>

    <div class="template-row">
      <label>
        <span>Template</span>
        <select bind:value={template} on:change={onTemplateOptionChange}>
          <option value="custom">Custom / empty</option>
          <option value="kickoff-completion">Kickoff + Completion</option>
          <option value="even-weekly">Even weekly</option>
          <option value="phases">Phase-by-phase</option>
        </select>
      </label>
      {#if template === "even-weekly"}
        <label class="template-param">
          <span>Weeks</span>
          <input bind:value={weeklyWeeks} type="number" min="1" step="1" />
        </label>
        <label class="template-param">
          <span>Amount / week</span>
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
          <span>Label</span>
          <input
            bind:value={milestone.label}
            maxlength="80"
            placeholder="Milestone label"
            required
          />
        </label>
        <label class="amount-field">
          <span>Amount</span>
          <div class="money-input">
            <i>{currency}</i>
            <input
              type="number"
              inputmode="decimal"
              min="0"
              step="0.01"
              value={(milestone.amountMinor / 100).toFixed(2)}
              on:input={(event) =>
                setMilestoneAmount(index, event.currentTarget.value)}
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
  </fieldset>

  <footer>
    <button class="secondary" type="button" disabled={busy} on:click={onCancel}>Cancel</button>
    <button class="primary" type="submit" disabled={busy || formInvalid}>
      {busy ? "Saving…" : project ? "Save changes" : "Create project"}
    </button>
  </footer>
</form>

<style>
  form {
    display: grid;
    gap: 15px;
  }

  .field-grid {
    display: grid;
    gap: 13px;
  }

  .field-grid.two {
    grid-template-columns: 1fr 160px;
  }

  .value-row {
    grid-template-columns: minmax(170px, 1fr) 145px 145px;
  }

  label {
    display: grid;
    gap: 7px;
    min-width: 0;
  }

  label > span,
  legend > span {
    color: var(--text-secondary, #aebdb4);
    font-size: 10.5px;
    font-weight: 600;
  }

  b {
    color: var(--accent, #43d17f);
    font-weight: inherit;
  }

  input,
  textarea,
  select {
    width: 100%;
    color: var(--text-primary, #edf5f0);
    font: inherit;
    font-size: 12px;
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-strong, #2a3a31);
    border-radius: 8px;
  }

  input,
  select {
    height: 38px;
    padding: 0 11px;
  }

  select {
    color-scheme: dark;
  }

  textarea {
    min-height: 68px;
    padding: 9px 11px;
    line-height: 1.5;
    resize: vertical;
  }

  input:hover,
  textarea:hover,
  select:hover {
    border-color: #3a4c42;
  }

  input:focus,
  textarea:focus,
  select:focus {
    border-color: var(--accent, #43d17f);
    outline: 1px solid var(--accent, #43d17f);
    outline-offset: 0;
  }

  input[aria-invalid="true"],
  textarea.invalid {
    border-color: var(--danger, #ef766f);
  }

  .money-input {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-strong, #2a3a31);
    border-radius: 8px;
  }

  .money-input:focus-within {
    border-color: var(--accent, #43d17f);
    outline: 1px solid var(--accent, #43d17f);
  }

  .money-input > i {
    padding-left: 11px;
    color: var(--text-muted, #75847b);
    font-size: 9px;
    font-style: normal;
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .money-input input {
    border: 0;
    outline: 0;
  }

  fieldset {
    display: grid;
    gap: 10px;
    margin: 1px 0 0;
    padding: 14px;
    background: rgba(67, 209, 127, 0.025);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 10px;
  }

  legend {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0 2px 7px;
  }

  legend small {
    color: var(--accent, #43d17f);
    font-size: 9.5px;
    font-weight: 650;
  }

  .template-row {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 10px;
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
    gap: 8px;
  }

  .template-confirm-label {
    color: var(--danger, #ef766f);
    font-size: 10.5px;
    font-weight: 600;
  }

  .danger-solid {
    color: #1a0906;
    background: var(--danger, #ef766f);
    border: 1px solid var(--danger, #ef766f);
  }

  .milestone {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) 150px 92px;
    align-items: end;
    gap: 10px;
  }

  .milestone-number {
    display: grid;
    width: 30px;
    height: 30px;
    margin-bottom: 4px;
    place-items: center;
    color: var(--accent, #43d17f);
    font-size: 9px;
    font-weight: 700;
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-radius: 50%;
  }

  .row-actions {
    display: flex;
    gap: 6px;
  }

  .row-actions button {
    min-height: 38px;
    min-width: 30px;
    padding: 0 8px;
    color: var(--text-secondary, #aebdb4);
    background: transparent;
    border: 1px solid var(--border-strong, #2a3a31);
  }

  .row-actions button.danger {
    color: var(--danger, #ef766f);
    border-color: var(--border-strong, #2a3a31);
  }

  .row-actions button.danger:hover {
    border-color: var(--danger, #ef766f);
  }

  .add-milestone {
    justify-self: start;
  }

  .empty-hint {
    margin: 2px 0;
    color: var(--text-muted, #75847b);
    font-size: 10.5px;
  }

  .validation {
    margin: -6px 0 0;
    color: var(--danger, #ef766f);
    font-size: 10px;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
    padding-top: 16px;
    border-top: 1px solid var(--border-subtle, #1b2821);
  }

  button {
    min-height: 36px;
    padding: 0 14px;
    font: inherit;
    font-size: 11px;
    font-weight: 650;
    border-radius: 8px;
    cursor: pointer;
  }

  button:focus-visible {
    outline: 2px solid var(--accent, #43d17f);
    outline-offset: 2px;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .secondary {
    color: var(--text-secondary, #aebdb4);
    background: transparent;
    border: 1px solid var(--border-strong, #2a3a31);
  }

  .primary {
    color: #07120c;
    background: var(--accent, #43d17f);
    border: 1px solid var(--accent, #43d17f);
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
