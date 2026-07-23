<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  type ExportKind =
    | "tasks"
    | "completed-work"
    | "invoices-payments"
    | "loan-schedules";

  interface ExportResult {
    destinationPath: string;
    rowCount: number;
  }

  const exportOptions: Array<{
    kind: ExportKind;
    label: string;
    description: string;
    fileLabel: string;
  }> = [
    {
      kind: "tasks",
      label: "Tasks",
      description: "Schedules, reminders, priorities, status, and project links.",
      fileLabel: "tasks",
    },
    {
      kind: "completed-work",
      label: "Completed work",
      description: "Dated work records with client, project, notes, and URLs.",
      fileLabel: "completed-work",
    },
    {
      kind: "invoices-payments",
      label: "Invoices & payments",
      description: "Invoice totals and a row for every received payment.",
      fileLabel: "invoices-payments",
    },
    {
      kind: "loan-schedules",
      label: "Loan schedules",
      description: "Every due date and its paid or unpaid state.",
      fileLabel: "loan-schedules",
    },
  ];

  let exporting: ExportKind | null = null;
  let feedback = "";
  let isError = false;

  async function exportData(option: (typeof exportOptions)[number]): Promise<void> {
    if (exporting) return;
    feedback = "";
    isError = false;

    if (!isTauriDesktop()) {
      feedback = "CSV export is available in the installed desktop application.";
      isError = true;
      return;
    }

    exporting = option.kind;
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const destination = await save({
        title: `Export ${option.label}`,
        defaultPath: `RudeSync-${option.fileLabel}-${localDate()}.csv`,
        filters: [{ name: "CSV spreadsheet", extensions: ["csv"] }],
      });
      if (!destination) return;

      const result = await invoke<ExportResult>("export_csv", {
        exportKind: option.kind,
        destination: ensureCsvExtension(destination),
      });
      feedback = `${option.label} exported: ${result.rowCount} ${
        result.rowCount === 1 ? "row" : "rows"
      }.`;
    } catch (error) {
      isError = true;
      feedback = errorText(error);
    } finally {
      exporting = null;
    }
  }

  function isTauriDesktop(): boolean {
    if (typeof window === "undefined") return false;
    return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
  }

  function ensureCsvExtension(path: string): string {
    return /\.csv$/i.test(path) ? path : `${path}.csv`;
  }

  function localDate(): string {
    const now = new Date();
    const year = now.getFullYear();
    const month = String(now.getMonth() + 1).padStart(2, "0");
    const day = String(now.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function errorText(error: unknown): string {
    if (error instanceof Error && error.message) return error.message;
    if (typeof error === "string" && error.trim()) return error;
    return "The CSV export could not be created.";
  }
</script>

<section class="settings-card export-card" aria-labelledby="export-title">
  <header class="card-header">
    <div>
      <span class="card-kicker">Portable data</span>
      <h3 id="export-title">CSV exports</h3>
      <p>Save readable spreadsheet copies without changing your local database.</p>
    </div>
  </header>

  <div class="export-grid">
    {#each exportOptions as option}
      <article>
        <div>
          <strong>{option.label}</strong>
          <small>{option.description}</small>
        </div>
        <button
          class="compact-button"
          type="button"
          disabled={exporting !== null}
          on:click={() => exportData(option)}
        >
          {exporting === option.kind ? "Exporting…" : "Export CSV"}
        </button>
      </article>
    {/each}
  </div>

  {#if feedback}
    <p class:error={isError} class="export-feedback" role={isError ? "alert" : "status"}>
      {feedback}
    </p>
  {/if}
</section>

<style>
  .export-card {
    margin-top: 18px;
    min-width: 0;
    overflow: hidden;
    background: var(--surface-1, #0e1512);
    border: 1px solid var(--border-subtle, #1b2922);
    border-radius: 12px;
  }

  .card-header {
    padding: 15px 17px;
    border-bottom: 1px solid var(--border-subtle, #1b2922);
  }

  .card-kicker {
    color: var(--accent, #43d17f);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .card-header h3 {
    margin: 3px 0 0;
    color: var(--text-primary, #eef5f1);
    font-size: 13px;
    font-weight: 620;
  }

  .card-header p {
    max-width: 620px;
    margin: 4px 0 0;
    color: var(--text-muted, #84938b);
    font-size: 9.5px;
    line-height: 1.55;
  }

  .export-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    margin-top: 18px;
  }

  .export-grid article {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-width: 0;
    padding: 14px;
    border: 1px solid var(--border-subtle, #233028);
    border-radius: 10px;
    background: var(--surface-raised, #111713);
  }

  .export-grid article > div {
    min-width: 0;
  }

  .export-grid strong,
  .export-grid small {
    display: block;
  }

  .export-grid strong {
    color: var(--text-primary, #edf4ef);
    font-size: 13px;
  }

  .export-grid small {
    margin-top: 4px;
    color: var(--text-muted, #8f9c93);
    font-size: 11px;
    line-height: 1.45;
  }

  .export-grid button {
    flex: 0 0 auto;
  }

  .compact-button {
    display: inline-flex;
    min-height: 34px;
    align-items: center;
    justify-content: center;
    padding: 0 12px;
    color: var(--text-secondary, #c6d2cc);
    font: inherit;
    font-size: 9.5px;
    font-weight: 650;
    background: var(--surface-raised, #18221d);
    border: 1px solid var(--border-strong, #26362e);
    border-radius: 7px;
    cursor: pointer;
  }

  .compact-button:hover:not(:disabled) {
    color: var(--text-primary, #eef5f1);
    border-color: var(--accent-border, rgba(67, 209, 127, 0.38));
  }

  .compact-button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .export-feedback {
    margin: 12px 0 0;
    color: var(--accent, #35c96f);
    font-size: 12px;
  }

  .export-feedback.error {
    color: var(--danger, #ef6c75);
  }

  @media (max-width: 980px) {
    .export-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
