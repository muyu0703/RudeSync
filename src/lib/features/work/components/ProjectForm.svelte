<script lang="ts">
  import type {
    Client,
    CreateProjectInput,
    Project,
    ProjectMilestone,
    ProjectStatus,
  } from "../types";

  export let clients: Client[] = [];
  export let initialClientId: string | null = null;
  export let project: Project | null = null;
  export let busy = false;
  export let onSave: (input: CreateProjectInput) => void | Promise<void>;
  export let onCancel: () => void;

  const existingKickoff = project?.milestones.find(
    (milestone) => milestone.kind === "kickoff",
  );
  const existingCompletion = project?.milestones.find(
    (milestone) => milestone.kind === "completion",
  );
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
  let kickoffLabel = existingKickoff?.label ?? "Kickoff";
  let kickoffPercent =
    (existingKickoff?.percentBasisPoints ?? 5000) / 100;
  let completionLabel = existingCompletion?.label ?? "Completion";
  let completionPercent =
    (existingCompletion?.percentBasisPoints ?? 5000) / 100;

  $: if (!clientId && clients.length) clientId = clients[0].id;
  $: selectedClient = clients.find((client) => client.id === clientId);
  $: currency =
    project && clientId === project.clientId
      ? project.currency
      : selectedClient?.currency ?? "USD";
  $: amountMinor = Math.round((Number(quotedValue) || 0) * 100);
  $: milestoneTotal = kickoffPercent + completionPercent;
  $: datesInvalid = Boolean(startDate && dueDate && dueDate < startDate);
  $: urls = urlsFromText();
  $: urlsInvalid = urls.some(invalidUrl);
  $: formInvalid =
    !clientId ||
    !name.trim() ||
    !quotedValue ||
    amountMinor < 0 ||
    milestoneTotal !== 100 ||
    !kickoffLabel.trim() ||
    !completionLabel.trim() ||
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

  function formatMilestone(percent: number): string {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency,
      maximumFractionDigits: 2,
    }).format((amountMinor * percent) / 10000);
  }

  function submit(): void {
    if (formInvalid || busy) return;
    const milestones: ProjectMilestone[] = [
      {
        kind: "kickoff",
        label: kickoffLabel,
        percentBasisPoints: Math.round(kickoffPercent * 100),
      },
      {
        kind: "completion",
        label: completionLabel,
        percentBasisPoints: Math.round(completionPercent * 100),
      },
    ];
    void onSave({
      clientId,
      name,
      description,
      urls,
      status,
      currency,
      quotedTotalMinor: amountMinor,
      milestones,
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
      <span>Invoice milestones</span>
      <small class:invalid={milestoneTotal !== 100}>{milestoneTotal}% of 100%</small>
    </legend>

    <div class="milestone">
      <span class="milestone-number">01</span>
      <label>
        <span>Label</span>
        <input bind:value={kickoffLabel} maxlength="80" required />
      </label>
      <label class="percent-field">
        <span>Percent</span>
        <div><input bind:value={kickoffPercent} type="number" min="0" max="100" step="0.01" required /><i>%</i></div>
      </label>
      <output>{formatMilestone(kickoffPercent)}</output>
    </div>

    <div class="milestone">
      <span class="milestone-number">02</span>
      <label>
        <span>Label</span>
        <input bind:value={completionLabel} maxlength="80" required />
      </label>
      <label class="percent-field">
        <span>Percent</span>
        <div><input bind:value={completionPercent} type="number" min="0" max="100" step="0.01" required /><i>%</i></div>
      </label>
      <output>{formatMilestone(completionPercent)}</output>
    </div>

    {#if milestoneTotal !== 100}
      <p class="validation" role="alert">Kickoff and completion must total exactly 100%.</p>
    {/if}
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

  legend small.invalid {
    color: var(--danger, #ef766f);
  }

  .milestone {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) 100px 92px;
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

  .percent-field > div {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-strong, #2a3a31);
    border-radius: 8px;
  }

  .percent-field > div:focus-within {
    border-color: var(--accent, #43d17f);
    outline: 1px solid var(--accent, #43d17f);
  }

  .percent-field input {
    border: 0;
    outline: 0;
  }

  .percent-field i {
    padding-right: 10px;
    color: var(--text-muted, #75847b);
    font-size: 10px;
    font-style: normal;
  }

  output {
    align-self: center;
    margin-top: 17px;
    overflow: hidden;
    color: var(--text-secondary, #aebdb4);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .validation {
    margin: -6px 0 0;
    color: var(--danger, #ef766f);
    font-size: 10px;
  }

  fieldset .validation {
    margin: 1px 0 0 42px;
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
      grid-template-columns: 32px minmax(0, 1fr) 90px;
    }

    output {
      display: none;
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

    .percent-field {
      grid-column: 2;
    }
  }
</style>
