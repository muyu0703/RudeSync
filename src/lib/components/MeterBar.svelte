<script lang="ts">
  export let label: string;
  export let value: number;
  export let max: number;
  export let detail: string | null = null;
  export let tone: "neutral" | "positive" | "warning" | "danger" = "positive";
  // Only set this when `detail` restates the value itself (e.g. "2 of 7 done",
  // "Paid"). aria-valuetext REPLACES the spoken percentage, so a detail that
  // describes something else (e.g. a complement like "3 still open") must not
  // be wired in here.
  export let valueText: string | null = null;

  $: percent = max > 0 ? Math.min(100, Math.max(0, (value / max) * 100)) : 0;
  // A progressbar whose max equals its min is invalid, and an empty day or week
  // legitimately has nothing to measure. Keep the range non-degenerate.
  $: valueMax = max > 0 ? max : 1;
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
    aria-valuemax={valueMax}
    aria-valuetext={valueText ?? undefined}
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
  .meter-label { font-size: var(--text-12); font-variant-numeric: tabular-nums; }
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
