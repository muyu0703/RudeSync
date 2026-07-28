<script lang="ts">
  import type { ChartPoint } from "../features/dashboard/chartSeries.ts";

  export let points: ChartPoint[] = [];
  /** Full-precision formatter for tooltips and the table. */
  export let formatValue: (value: number) => string = (value) => String(value);
  /** Compact formatter for axis ticks. Defaults to `formatValue`. */
  export let formatAxis: ((value: number) => string) | null = null;
  /** Column heading for the measured value, e.g. "Collected". */
  export let valueLabel = "Value";
  /** Column heading for the whole, when points carry a `total`. */
  export let totalLabel = "Total";
  export let emptyMessage = "Nothing to show yet.";
  export let tableCaption = "Chart data";
  /**
   * Show every Nth tick, counted back from the most recent bar so the newest
   * one is always labelled. Axis labels are thinned, never all shown at once.
   */
  export let labelEvery = 1;
  /**
   * Dim rather than unmount while refreshing. A skeleton on refetch flashes
   * and jumps the layout; holding the last render is calmer and keeps the
   * card the same height.
   */
  export let loading = false;

  const PLOT_HEIGHT = 118;

  $: axisFormatter = formatAxis ?? formatValue;
  $: isMeter = points.some((point) => point.total !== null);
  $: rawMax = points.reduce(
    (max, point) => Math.max(max, point.value, point.total ?? 0),
    0,
  );
  $: axisMax = niceMax(rawMax);
  $: isEmpty = rawMax === 0;
  /**
   * Direct-label exactly one bar -- the tallest. A number on every column is
   * noise that goes unread; the axis ticks and the tooltip carry the rest.
   */
  $: peakIndex = points.reduce(
    (best, point, index) =>
      Math.max(point.value, point.total ?? 0) >
      Math.max(points[best]?.value ?? 0, points[best]?.total ?? 0)
        ? index
        : best,
    0,
  );

  let hovered: number | null = null;

  /**
   * Round the axis top to a clean number so ticks read 0 / 5 / 10, not 0 / 7.
   * The ladder is deliberately even: the midpoint tick is axisMax / 2, and an
   * odd top would print "2.5" on an axis counting whole tasks. Every value
   * plotted is an integer (task counts, money in minor units), so the smallest
   * top is 2 and the magnitude is never fractional -- which keeps the midpoint
   * a whole number at every scale.
   */
  function niceMax(raw: number): number {
    if (raw <= 0) return 2;
    const magnitude = 10 ** Math.floor(Math.log10(raw));
    for (const step of [2, 4, 6, 8, 10]) {
      if (step * magnitude >= raw) return step * magnitude;
    }
    return 10 * magnitude;
  }

  function heightPercent(value: number): number {
    return Math.max(0, Math.min(100, (value / axisMax) * 100));
  }

  /** Portion of the track that is filled. Overpayment clamps, so a bar never
   *  overflows its own track; the true figures still show in the tooltip. */
  function fillPercent(point: ChartPoint): number {
    if (point.total === null || point.total <= 0) return 0;
    return Math.max(0, Math.min(100, (point.value / point.total) * 100));
  }

  function showsTick(index: number): boolean {
    return (points.length - 1 - index) % labelEvery === 0;
  }

  /** Keep an edge tooltip inside the card instead of clipping it. */
  function tooltipAlign(index: number): "start" | "center" | "end" {
    if (points.length < 3) return "center";
    if (index <= 0) return "start";
    if (index >= points.length - 1) return "end";
    return "center";
  }
</script>

<figure class="chart" class:loading>
  {#if isEmpty}
    <!-- Never assert "nothing here" while the data is still in flight; the
         reserved box also stops the card resizing when the bars arrive. -->
    <p class="chart-empty">{loading ? "" : emptyMessage}</p>
  {:else}
    <div class="chart-body" style={`--plot-height:${PLOT_HEIGHT}px`}>
      <div class="y-axis" aria-hidden="true">
        <span>{axisFormatter(axisMax)}</span>
        <span>{axisFormatter(axisMax / 2)}</span>
        <span>{axisFormatter(0)}</span>
      </div>

      <!-- The plot duplicates the table below it, so it is hidden from
           assistive tech rather than producing one tab stop per bar. Every
           value stays reachable through the table view. -->
      <div class="plot" aria-hidden="true">
        <div class="plot-area">
          <span class="grid-line" style="top:0"></span>
          <span class="grid-line" style="top:50%"></span>
          <span class="grid-line baseline" style="bottom:0"></span>

          <div class="columns">
          {#each points as point, index (point.key)}
            <!-- Decorative: the whole plot is aria-hidden and the table view
                 below carries every value. Deliberately not a button -- a
                 focusable control inside an aria-hidden subtree is still
                 tabbable and would strand keyboard users on 14 silent stops. -->
            <div
              class="column"
              role="presentation"
              class:active={hovered === index}
              on:mouseenter={() => (hovered = index)}
              on:mouseleave={() => (hovered = null)}
            >
              {#if index === peakIndex}
                <!-- Compact form, and edge-aligned like the tooltip: a full
                     "$12,500.00" centred on the first or last column would
                     hang outside the plot and off the card. -->
                <span class="peak-label" data-align={tooltipAlign(index)}>
                  {axisFormatter(isMeter ? (point.total ?? 0) : point.value)}
                </span>
              {/if}

              <div
                class="bar"
                class:meter={isMeter}
                class:empty={(isMeter ? (point.total ?? 0) : point.value) === 0}
                style={`height:${heightPercent(isMeter ? (point.total ?? 0) : point.value)}%`}
              >
                {#if isMeter}
                  <i class="bar-fill" style={`height:${fillPercent(point)}%`}></i>
                {/if}
              </div>

              {#if hovered === index}
                <div class="tooltip" data-align={tooltipAlign(index)}>
                  <strong>{point.fullLabel}</strong>
                  <span>{valueLabel}: {formatValue(point.value)}</span>
                  {#if isMeter}
                    <span>{totalLabel}: {formatValue(point.total ?? 0)}</span>
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
          </div>
        </div>
      </div>

      <div class="x-axis" aria-hidden="true">
        {#each points as point, index (point.key)}
          <span>{showsTick(index) ? point.label : ""}</span>
        {/each}
      </div>
    </div>

    <details class="table-view">
      <summary>View as table</summary>
      <div class="table-scroll">
        <table>
          <caption class="visually-hidden">{tableCaption}</caption>
          <thead>
            <tr>
              <th scope="col">Period</th>
              <th scope="col">{valueLabel}</th>
              {#if isMeter}<th scope="col">{totalLabel}</th>{/if}
            </tr>
          </thead>
          <tbody>
            {#each points as point (point.key)}
              <tr>
                <th scope="row">{point.fullLabel}</th>
                <td>{formatValue(point.value)}</td>
                {#if isMeter}<td>{formatValue(point.total ?? 0)}</td>{/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </details>
  {/if}
</figure>

<style>
  .chart {
    margin: 0;
    min-width: 0;
  }
  .chart.loading {
    opacity: 0.55;
  }

  .chart-empty {
    display: grid;
    place-items: center;
    min-height: 96px;
    margin: 0;
    color: var(--text-tertiary);
    font-size: var(--text-12);
    text-align: center;
  }

  /* The gutter carries the y ticks; the plot and the x-axis band share the
     same right-hand column so ticks line up under their bars, and the figure
     grows to include the axis band rather than scrolling inside itself. */
  .chart-body {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-2);
  }

  /* The axis gutter and the plot must resolve to the same total height, so
     both are content-box: the 18px label headroom sits on top of the plot
     height rather than eating into it under a global border-box. */
  .y-axis {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    height: var(--plot-height);
    padding-top: 18px;
    box-sizing: content-box;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .y-axis span {
    transform: translateY(-50%);
  }
  .y-axis span:first-child {
    transform: translateY(0);
  }
  .y-axis span:last-child {
    transform: translateY(-100%);
  }

  /* Reserved headroom so the one direct label and the tooltip sit above the
     tallest cap instead of being clipped by the plot edge. The gridlines live
     in .plot-area, whose box is exactly the value range -- putting them in
     .plot would anchor them to the padding box and float the top line up into
     the headroom, misplacing the axis maximum. */
  .plot {
    position: relative;
    padding-top: 18px;
  }
  .plot-area {
    position: relative;
    height: var(--plot-height);
  }

  .grid-line {
    position: absolute;
    left: 0;
    right: 0;
    height: 1px;
    background: var(--viz-grid);
    pointer-events: none;
  }
  /* Solid hairlines only -- a dashed grid reads as a threshold. */
  .grid-line.baseline {
    background: var(--separator-strong);
  }

  .columns {
    display: flex;
    align-items: flex-end;
    gap: 2px; /* the surface gap that separates touching bars */
    height: 100%;
  }

  /* The hit area is the whole slot, not the bar, so a thin bar is still easy
     to hover. */
  .column {
    position: relative;
    display: flex;
    flex: 1 1 0;
    align-items: flex-end;
    justify-content: center;
    min-width: 0;
    height: 100%;
  }

  .bar {
    width: 100%;
    max-width: 24px; /* never let a bar fill a wide slot */
    /* Keeps a small non-zero value visible... */
    min-height: 2px;
    background: var(--viz-fill);
    border-radius: 4px 4px 0 0; /* rounded data-end, square on the baseline */
  }
  /* ...but a genuine zero must draw nothing, or the stub reads as a count of
     one. The axis tick still shows the day was in range. */
  .bar.empty {
    min-height: 0;
  }
  .bar.meter {
    position: relative;
    overflow: hidden;
    background: var(--viz-track);
  }
  .bar-fill {
    position: absolute;
    inset: auto 0 0 0;
    display: block;
    background: var(--viz-fill);
    border-radius: 4px 4px 0 0;
  }
  .column.active .bar {
    filter: brightness(1.12);
  }

  .peak-label {
    position: absolute;
    bottom: 100%;
    margin-bottom: 4px;
    color: var(--text-secondary);
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .peak-label[data-align="center"] {
    left: 50%;
    transform: translateX(-50%);
  }
  .peak-label[data-align="start"] {
    left: 0;
  }
  .peak-label[data-align="end"] {
    right: 0;
  }

  .tooltip {
    position: absolute;
    bottom: calc(100% + 6px);
    z-index: 5;
    display: grid;
    gap: 1px;
    min-width: max-content;
    padding: var(--space-2) var(--space-3);
    background: var(--surface-overlay);
    border: 1px solid var(--separator-strong);
    border-radius: var(--radius-control);
    box-shadow: 0 8px 20px rgb(0 0 0 / 0.35);
    pointer-events: none;
  }
  .tooltip[data-align="center"] {
    left: 50%;
    transform: translateX(-50%);
  }
  .tooltip[data-align="start"] {
    left: 0;
  }
  .tooltip[data-align="end"] {
    right: 0;
  }
  .tooltip strong {
    color: var(--text-primary);
    font-size: var(--text-12);
    font-weight: var(--weight-semibold);
  }
  .tooltip span {
    color: var(--text-secondary);
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
  }

  .x-axis {
    /* Explicit: with three children in a two-column grid, auto-placement would
       drop this into row 2 column 1 -- under the y-axis gutter rather than
       under the bars it labels. */
    grid-column: 2;
    display: flex;
    gap: 2px;
    margin-top: var(--space-2);
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
  }
  .x-axis span {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-align: center;
    white-space: nowrap;
  }

  .table-view {
    margin-top: var(--space-3);
  }
  .table-view summary {
    color: var(--text-tertiary);
    cursor: pointer;
    font-size: var(--text-11);
  }
  .table-view summary:hover {
    color: var(--text-secondary);
  }
  .table-scroll {
    overflow-x: auto;
    margin-top: var(--space-2);
  }
  .table-view table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
  }
  .table-view th,
  .table-view td {
    padding: 3px var(--space-2) 3px 0;
    text-align: left;
    white-space: nowrap;
  }
  .table-view thead th {
    color: var(--text-tertiary);
    font-weight: var(--weight-medium);
  }
  .table-view tbody th {
    color: var(--text-secondary);
    font-weight: var(--weight-regular);
  }
  .table-view td {
    color: var(--text-primary);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
