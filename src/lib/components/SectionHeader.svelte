<script lang="ts">
  export let title: string;
  export let subtext: string | null = null;
</script>

<div class="section-header">
  <div class="section-copy">
    <h2>{title}</h2>
    {#if subtext}<p>{subtext}</p>{/if}
  </div>
  {#if $$slots.actions}
    <div class="section-actions"><slot name="actions" /></div>
  {/if}
</div>

<style>
  .section-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
    min-width: 0;
  }
  .section-copy { min-width: 0; }
  h2 { font-size: var(--text-15); font-weight: var(--weight-semibold); }
  p { margin-top: 2px; color: var(--text-tertiary); font-size: var(--text-11); }
  .section-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    row-gap: var(--space-2);
    /* Allow shrinking (unlike the old flex: 0 0 auto) so wide content, e.g.
       a scrollable .segmented control, can give up space instead of
       overflowing the card header and being clipped by .content-scroll. */
    flex: 0 1 auto;
    min-width: 0;
  }
</style>
