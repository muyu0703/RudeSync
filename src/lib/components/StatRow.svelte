<div class="stat-row"><slot /></div>

<style>
  .stat-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--space-3);
    max-width: 1120px;
    margin: 0 auto var(--space-4);
  }
</style>
