<script lang="ts">
  import { onMount } from "svelte";

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
  <section class="dialog-card">
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
    color: var(--text-primary, #edf5f0);
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
    background: var(--surface-1, #0c1210);
    border: 1px solid var(--border-strong, #2a3a31);
    border-radius: 14px;
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.5);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    padding: 20px 22px 17px;
    border-bottom: 1px solid var(--border-subtle, #1b2821);
  }

  h2 {
    margin: 0;
    color: var(--text-primary, #edf5f0);
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  p {
    margin: 5px 0 0;
    color: var(--text-muted, #75847b);
    font-size: 11.5px;
    line-height: 1.55;
  }

  .close-button {
    display: grid;
    width: 30px;
    height: 30px;
    flex: 0 0 auto;
    padding: 0;
    place-items: center;
    color: var(--text-muted, #75847b);
    background: transparent;
    border: 0;
    border-radius: 7px;
    cursor: pointer;
  }

  .close-button:hover {
    color: var(--text-primary, #edf5f0);
    background: var(--surface-raised, #141d18);
  }

  .close-button:focus-visible {
    outline: 2px solid var(--accent, #43d17f);
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
    padding: 20px 22px 22px;
    overflow-y: auto;
    scrollbar-color: var(--border-strong, #2a3a31) transparent;
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
      padding-right: 17px;
      padding-left: 17px;
    }
  }
</style>
