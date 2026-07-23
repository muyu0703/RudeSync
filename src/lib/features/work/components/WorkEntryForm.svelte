<script lang="ts">
  import type {
    CreateWorkEntryInput,
    Project,
    WorkEntry,
  } from "../types";
  import { workDateUtils } from "../workService";

  export let projects: Project[] = [];
  export let initialProjectId: string | null = null;
  export let initialTitle = "";
  export let initialDetails = "";
  export let initialWorkDate = workDateUtils.localIsoDay();
  export let entry: WorkEntry | null = null;
  export let busy = false;
  export let onSave: (input: CreateWorkEntryInput) => void | Promise<void>;
  export let onCancel: () => void;

  let projectId = entry?.projectId ?? initialProjectId ?? "";
  let title = entry?.title ?? initialTitle;
  let details = entry?.details ?? initialDetails;
  let workDate = entry?.workDate ?? initialWorkDate;
  let urlsText = entry?.urls.join("\n") ?? "";
  let validationMessage = "";

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

  function submit(): void {
    if (!title.trim() || !workDate || busy) return;
    const urls = urlsFromText();
    if (urls.some(invalidUrl)) {
      validationMessage = "Use a complete http:// or https:// URL, one per line.";
      return;
    }
    validationMessage = "";
    void onSave({
      projectId: projectId || null,
      title,
      details,
      workDate,
      urls,
    });
  }
</script>

<form on:submit|preventDefault={submit}>
  <div class="field-grid project-row">
    <label>
      <span>Project</span>
      <select bind:value={projectId} data-work-autofocus>
        <option value="">No project · personal work</option>
        {#each projects as project (project.id)}
          <option value={project.id}>{project.name}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>Completed on <b aria-hidden="true">*</b></span>
      <input bind:value={workDate} type="date" required />
    </label>
  </div>

  <label>
    <span>What did you complete? <b aria-hidden="true">*</b></span>
    <input
      bind:value={title}
      maxlength="200"
      placeholder="Finished the account settings flow"
      required
    />
  </label>

  <label>
    <span>Details</span>
    <textarea
      bind:value={details}
      maxlength="2400"
      rows="4"
      placeholder="A concise record of what changed or shipped."
    ></textarea>
  </label>

  <label>
    <span>Reference URLs</span>
    <textarea
      bind:value={urlsText}
      class:invalid={validationMessage}
      aria-invalid={Boolean(validationMessage)}
      aria-describedby="work-url-help"
      rows="3"
      placeholder={"https://project.example.com\nhttps://github.com/…"}
    ></textarea>
    <small id="work-url-help">Optional · one complete URL per line</small>
  </label>

  {#if validationMessage}
    <p class="validation" role="alert">{validationMessage}</p>
  {/if}

  <footer>
    <button class="secondary" type="button" disabled={busy} on:click={onCancel}>Cancel</button>
    <button class="primary" type="submit" disabled={busy || !title.trim() || !workDate}>
      {busy ? "Saving…" : entry ? "Save changes" : "Record completed work"}
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

  .project-row {
    grid-template-columns: minmax(0, 1fr) 155px;
  }

  label {
    display: grid;
    gap: 7px;
    min-width: 0;
  }

  label > span {
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

  textarea.invalid {
    border-color: var(--danger, #ef766f);
  }

  small {
    color: var(--text-faint, #536158);
    font-size: 9px;
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

  @media (max-width: 500px) {
    .project-row {
      grid-template-columns: 1fr;
    }
  }
</style>
