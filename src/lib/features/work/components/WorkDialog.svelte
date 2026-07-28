<script lang="ts">
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { scale } from "svelte/transition";
  import { motionDuration } from "../../../motion";

  export let title: string;
  export let description = "";
  export let wide = false;
  export let onClose: () => void;

  let dialog: HTMLDialogElement;

  onMount(() => {
    dialog.showModal();
    dialog
      .querySelector<HTMLElement>("[data-work-autofocus]")
      ?.focus();
  });

  function close(): void {
    if (dialog.open) dialog.close();
    onClose();
  }

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === dialog) close();
  }
</script>

<dialog
  bind:this={dialog}
  class:wide
  aria-labelledby="work-dialog-title"
  aria-describedby={description ? "work-dialog-description" : undefined}
  on:cancel|preventDefault={close}
  on:click={handleBackdrop}
>
  <!--
    No transition on the <dialog> root: its visible scrim is the native
    ::backdrop pseudo-element (styled below), which Svelte cannot animate,
    and `dialog { background: transparent }` makes a fade on the root itself
    invisible.

    No out:transition on .dialog-card either: every close path calls the
    native dialog.close() synchronously, which strips the `open` attribute
    and lets the UA stylesheet (`dialog:not([open]) { display: none }`) hide
    the element before an outro could render, so it would just be dead code.
  -->
  <section
    class="dialog-card"
    in:scale={{ duration: motionDuration(200), start: 0.96, opacity: 0, easing: cubicOut }}
  >
    <header>
      <div>
        <h2 id="work-dialog-title">{title}</h2>
        {#if description}
          <p id="work-dialog-description">{description}</p>
        {/if}
      </div>
      <button class="close-button" type="button" aria-label="Close dialog" on:click={close}>
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="m6 6 12 12M18 6 6 18"></path>
        </svg>
      </button>
    </header>
    <div class="dialog-body"><slot /></div>
  </section>
</dialog>

<style>
  dialog {
    width: min(470px, calc(100vw - 32px));
    max-height: min(760px, calc(100vh - 32px));
    padding: 0;
    color: var(--text-primary);
    background: transparent;
    border: 0;
    overflow: visible;
  }

  dialog.wide {
    width: min(690px, calc(100vw - 32px));
  }

  dialog::backdrop {
    background: rgba(2, 5, 4, 0.74);
  }

  .dialog-card {
    max-height: min(760px, calc(100vh - 32px));
    overflow: hidden;
    background: var(--surface-overlay);
    border-radius: var(--radius-sheet);
    box-shadow: var(--shadow-sheet);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-5);
    padding: var(--space-5) var(--space-5) var(--space-4);
    border-bottom: 1px solid var(--separator);
  }

  h2 {
    margin: 0;
    color: var(--text-primary);
    font-size: var(--text-17);
    font-weight: var(--weight-semibold);
    letter-spacing: -0.02em;
  }

  p {
    margin: var(--space-1) 0 0;
    color: var(--text-tertiary);
    font-size: var(--text-12);
    line-height: 1.55;
  }

  .close-button {
    display: grid;
    width: 30px;
    height: 30px;
    flex: 0 0 auto;
    padding: 0;
    place-items: center;
    color: var(--text-tertiary);
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .close-button:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }

  .close-button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  svg {
    width: 17px;
    height: 17px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-width: 1.8;
  }

  .dialog-body {
    max-height: calc(min(760px, 100vh - 32px) - 78px);
    padding: var(--space-5);
    overflow-y: auto;
    scrollbar-color: var(--separator-strong) transparent;
    scrollbar-width: thin;
  }

  @media (max-width: 540px) {
    dialog,
    dialog.wide {
      width: calc(100vw - 20px);
      max-height: calc(100vh - 20px);
    }

    .dialog-card {
      max-height: calc(100vh - 20px);
    }

    header,
    .dialog-body {
      padding-right: var(--space-4);
      padding-left: var(--space-4);
    }
  }
</style>
