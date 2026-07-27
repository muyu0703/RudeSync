<script lang="ts">
  export let padded = true;
</script>

<section class="card" class:padded>
  {#if $$slots.header}
    <header class="card-header"><slot name="header" /></header>
  {/if}
  <slot />
</section>

<style>
  .card {
    min-width: 0;
    background: var(--surface-content);
    border-radius: var(--radius-panel);
    box-shadow: var(--shadow-raised);
  }
  .padded { padding: var(--space-4); }
  .card-header {
    margin: calc(var(--space-4) * -1) calc(var(--space-4) * -1) var(--space-4);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--separator);
  }
  .card:not(.padded) > .card-header { margin: 0; }
</style>
