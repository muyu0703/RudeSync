<script lang="ts">
  import Icon from "./Icon.svelte";

  export let label: string;
  export let value: string;
  export let detail: string | null = null;
  export let tone: "neutral" | "positive" | "warning" | "danger" = "neutral";
  export let icon:
    | "today"
    | "tasks"
    | "work"
    | "money"
    | "review"
    | "settings"
    | "plus"
    | "search"
    | "bell"
    | "chevron-right"
    | "calendar"
    | "check"
    | "clock"
    | "invoice"
    | "loan"
    | "arrow-up-right"
    | "more"
    | "link"
    | "spark"
    | "tray"
    | "database"
    | "palette"
    | "shield"
    | "keyboard"
    | "circle"
    | "x"
    | "briefcase"
    | null = null;
</script>

<article class="stat" data-tone={tone}>
  <div class="stat-head">
    {#if icon}
      <span class="stat-icon"><Icon name={icon} size={14} /></span>
    {/if}
    <span class="stat-label">{label}</span>
  </div>
  <div class="stat-value">{value}</div>
  {#if detail}<div class="stat-detail">{detail}</div>{/if}
</article>

<style>
  .stat {
    min-width: 0;
    padding: var(--space-4);
    background: var(--surface-content);
    border-radius: var(--radius-panel);
    box-shadow: var(--shadow-raised);
  }
  .stat-head { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-3); }
  .stat-icon {
    display: grid;
    width: 24px;
    height: 24px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: var(--radius-control);
  }
  .stat-label { color: var(--text-secondary); font-size: var(--text-12); }
  .stat-value {
    font-size: var(--text-24);
    font-weight: var(--weight-semibold);
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .stat-detail { margin-top: var(--space-1); color: var(--text-tertiary); font-size: var(--text-11); }

  /* Tone colours the value and the icon tile. It never changes layout, and the
     label always states the meaning in words so colour is not the only signal. */
  [data-tone="positive"] .stat-icon { color: var(--accent); background: var(--accent-fill); }
  [data-tone="positive"] .stat-value { color: var(--accent); }
  [data-tone="warning"] .stat-icon { color: var(--amber); background: var(--amber-fill); }
  [data-tone="danger"] .stat-icon { color: var(--danger); background: var(--danger-fill); }
  [data-tone="danger"] .stat-value { color: var(--danger); }
  [data-tone="neutral"] .stat-icon { color: var(--text-secondary); background: var(--surface-raised); }
</style>
