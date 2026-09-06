<script lang="ts">
  import type {
    CreateWorkEntryInput,
    Project,
    WorkEntry,
  } from "../types";
  import { workDateUtils } from "../workService";
  import WorkDialog from "./WorkDialog.svelte";

  export let dialogTitle: string;
  export let dialogDescription = "";
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

  // A fresh WorkEntryForm instance is created each time the dialog opens, so
  // this snapshot taken at construction is the form's true starting point.
  const initialSnapshot = JSON.stringify({ projectId, title, details, workDate, urlsText });
  $: dirty =
    JSON.stringify({ projectId, title, details, workDate, urlsText }) !== initialSnapshot;

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

<WorkDialog title={dialogTitle} description={dialogDescription} {dirty} onClose={onCancel}>
<form id="work-entry-form" on:submit|preventDefault={submit}>
  <div class="field-grid project-row">
    <label>
      <span class="field-label">项目</span>
      <select class="field-input" bind:value={projectId} data-work-autofocus>
        <option value="">无项目 · 个人工作</option>
        {#each projects as project (project.id)}
          <option value={project.id}>{project.name}</option>
        {/each}
      </select>
    </label>
    <label>
      <span class="field-label">完成日期 <b aria-hidden="true">*</b></span>
      <input class="field-input" bind:value={workDate} type="date" required />
    </label>
  </div>

  <label>
    <span class="field-label">完成了什么？ <b aria-hidden="true">*</b></span>
    <input
      class="field-input"
      bind:value={title}
      maxlength="200"
      placeholder="例如：完成店铺设置流程"
      required
    />
  </label>

  <label>
    <span class="field-label">详情</span>
    <textarea
      class="field-textarea"
      bind:value={details}
      maxlength="2400"
      rows="4"
      placeholder="简要记录完成或交付的内容。"
    ></textarea>
  </label>

  <label>
    <span class="field-label">参考链接</span>
    <textarea
      class="field-textarea"
      bind:value={urlsText}
      class:invalid={validationMessage}
      aria-invalid={Boolean(validationMessage)}
      aria-describedby="work-url-help"
      rows="3"
      placeholder={"https://project.example.com\nhttps://github.com/…"}
    ></textarea>
    <small id="work-url-help" class="field-hint">可选 · 每行一个完整链接</small>
  </label>

  {#if validationMessage}
    <p class="field-error" role="alert">{validationMessage}</p>
  {/if}

</form>

<svelte:fragment slot="footer">
  <footer>
    <button class="secondary" type="button" disabled={busy} on:click={onCancel}>取消</button>
    <button class="primary" type="submit" form="work-entry-form" disabled={busy || !title.trim() || !workDate}>
      {busy ? "Saving…" : entry ? "Save changes" : "Record completed work"}
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

  .project-row {
    grid-template-columns: minmax(0, 1fr) 155px;
  }

  label {
    display: grid;
    gap: var(--space-2);
    min-width: 0;
  }

  b {
    color: var(--accent);
    font-weight: inherit;
  }

  select.field-input {
    color-scheme: dark;
  }

  .field-textarea.invalid {
    border-color: var(--danger);
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

  @media (max-width: 500px) {
    .project-row {
      grid-template-columns: 1fr;
    }
  }
</style>
