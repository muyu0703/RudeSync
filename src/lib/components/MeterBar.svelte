<script lang="ts">
  export let label: string;
  export let value: number;
  export let max: number;
  export let detail: string | null = null;
  export let tone: "neutral" | "positive" | "warning" | "danger" = "positive";

  $: percent = max > 0 ? Math.min(100, Math.max(0, (value / max) * 100)) : 0;
</script>

<div class="meter" data-tone={tone}>
  <div class="meter-head">
    <span class="meter-label">{label}</span>
    {#if detail}<span class="meter-detail">{detail}</span>{/if}
  </div>
  <div
    class="meter-track"
    role="progressbar"
    aria-label={label}
    aria-valuenow={value}
    aria-valuemin={0}
    aria-valuemax={max}
  >
    <i style={`width:${percent}%`}></i>
  </div>
</div>

<style>
  .meter { min-width: 0; }
  .meter-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-2);
    margin-bottom: var(--space-1);
  }
  .meter-label { font-size: var(--text-12); }
  .meter-detail {
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
  }
  .meter-track {
    height: 5px;
    overflow: hidden;
    background: var(--surface-raised);
    border-radius: var(--radius-pill);
  }
  .meter-track i {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
    transition: width var(--duration) var(--ease);
  }
  [data-tone="warning"] .meter-track i { background: var(--amber); }
  [data-tone="danger"] .meter-track i { background: var(--danger); }
  [data-tone="neutral"] .meter-track i { background: var(--text-tertiary); }
</style>
