<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Card from "../../components/Card.svelte";
  import SectionHeader from "../../components/SectionHeader.svelte";

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
      label: "待办事项",
      description: "包含计划、提醒、优先级、状态和项目关联。",
      fileLabel: "tasks",
    },
    {
      kind: "completed-work",
      label: "已完成工作",
      description: "包含日期、客户、项目、备注和链接的工作记录。",
      fileLabel: "completed-work",
    },
    {
      kind: "invoices-payments",
      label: "发票与收款",
      description: "包含发票汇总，以及每笔已收款记录。",
      fileLabel: "invoices-payments",
    },
    {
      kind: "loan-schedules",
      label: "借款计划",
      description: "包含每个到期日及其已还/未还状态。",
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
      feedback = "CSV 导出仅在已安装的桌面版中可用。";
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
    return "无法创建 CSV 导出文件。";
  }
</script>

<Card>
  <SectionHeader
    slot="header"
    title="CSV 导出"
    subtext="导出可阅读的表格副本，不会修改本地数据库。"
  />

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
          {exporting === option.kind ? "正在导出…" : "导出 CSV"}
        </button>
      </article>
    {/each}
  </div>

  {#if feedback}
    <p class:error={isError} class="export-feedback" role={isError ? "alert" : "status"}>
      {feedback}
    </p>
  {/if}
</Card>

<style>
  .export-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
  }

  .export-grid article {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-width: 0;
    padding: var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-control);
  }

  .export-grid article > div { min-width: 0; }
  .export-grid strong, .export-grid small { display: block; }
  .export-grid strong { font-size: var(--text-13); font-weight: var(--weight-medium); }
  .export-grid small {
    margin-top: var(--space-1);
    color: var(--text-tertiary);
    font-size: var(--text-11);
    line-height: 1.45;
  }
  .export-grid button { flex: 0 0 auto; }

  .compact-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 32px;
    padding: 0 var(--space-3);
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--text-12);
    font-weight: var(--weight-medium);
    background: var(--surface-active);
    border: 1px solid var(--separator-strong);
    border-radius: var(--radius-control);
    cursor: pointer;
  }
  .compact-button:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--accent-line);
  }
  .compact-button:disabled { cursor: not-allowed; opacity: 0.45; }

  .export-feedback {
    margin: var(--space-3) 0 0;
    color: var(--accent);
    font-size: var(--text-12);
  }
  .export-feedback.error { color: var(--danger); }

  @media (max-width: 780px) {
    .export-grid { grid-template-columns: 1fr; }
  }
</style>
