<script lang="ts">
  import type { Client, CreateClientInput } from "../types";
  import WorkDialog from "./WorkDialog.svelte";

  export let dialogTitle: string;
  export let dialogDescription = "";
  export let busy = false;
  export let defaultCurrency = "USD";
  export let client: Client | null = null;
  export let onSave: (input: CreateClientInput) => void | Promise<void>;
  export let onCancel: () => void;

  let name = client?.name ?? "";
  let companyName = client?.companyName ?? "";
  let email = client?.email ?? "";
  let billingAddress = client?.billingAddress ?? "";
  let currency = client?.currency ?? defaultCurrency;
  let notes = client?.notes ?? "";

  // A fresh ClientForm instance is created each time the dialog opens (see
  // WorkView's `{#if dialog === ...}`), so this snapshot taken at
  // construction is the form's true starting point.
  const initialSnapshot = JSON.stringify({
    name,
    companyName,
    email,
    billingAddress,
    currency,
    notes,
  });
  $: dirty =
    JSON.stringify({ name, companyName, email, billingAddress, currency, notes }) !==
    initialSnapshot;

  function submit(): void {
    if (!name.trim() || busy) return;
    void onSave({
      name,
      companyName,
      email,
      billingAddress,
      currency,
      notes,
    });
  }
</script>

<WorkDialog title={dialogTitle} description={dialogDescription} {dirty} onClose={onCancel}>
  <form id="client-form" on:submit|preventDefault={submit}>
    <div class="field-grid two">
      <label>
        <span class="field-label">Client name <b aria-hidden="true">*</b></span>
        <input class="field-input" bind:value={name} maxlength="120" autocomplete="name" data-work-autofocus required />
      </label>
      <label>
        <span class="field-label">公司</span>
        <input class="field-input" bind:value={companyName} maxlength="160" autocomplete="organization" />
      </label>
    </div>

    <div class="field-grid currency-row">
      <label>
        <span class="field-label">邮箱</span>
        <input class="field-input" bind:value={email} maxlength="254" type="email" autocomplete="email" />
      </label>
      <label>
        <span class="field-label">币种</span>
        <input
          class="field-input"
          value={currency}
          on:input={(e) => (currency = e.currentTarget.value.toUpperCase())}
          maxlength="3"
          minlength="3"
          pattern={"[A-Z]{3}"}
          autocapitalize="characters"
          aria-describedby="currency-help"
          required
        />
        <small id="currency-help" class="field-hint">三位字母代码</small>
      </label>
    </div>

    <label>
      <span class="field-label">账单地址</span>
      <textarea class="field-textarea" bind:value={billingAddress} maxlength="600" rows="2"></textarea>
    </label>

    <label>
      <span class="field-label">内部备注</span>
      <textarea class="field-textarea" bind:value={notes} maxlength="1200" rows="3"></textarea>
    </label>
  </form>

  <svelte:fragment slot="footer">
    <footer>
      <button class="secondary" type="button" disabled={busy} on:click={onCancel}>取消</button>
      <button class="primary" type="submit" form="client-form" disabled={busy || !name.trim() || currency.length !== 3}>
        {busy ? "Saving…" : client ? "Save changes" : "Create client"}
      </button>
    </footer>
  </svelte:fragment>
</WorkDialog>

<style>
  form {
    display: grid;
    gap: var(--space-4);
  }

  .field-grid {
    display: grid;
    gap: var(--space-3);
  }

  .field-grid.two {
    grid-template-columns: 1fr 1fr;
  }

  .currency-row {
    grid-template-columns: minmax(0, 1fr) 112px;
  }

  label {
    display: grid;
    gap: var(--space-2);
    min-width: 0;
  }

  b {
    color: var(--accent);
    font-weight: inherit;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-1);
    padding-top: var(--space-4);
    border-top: 1px solid var(--separator);
  }

  button {
    min-height: 36px;
    padding: 0 var(--space-4);
    font: inherit;
    font-size: var(--text-11);
    font-weight: var(--weight-semibold);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .secondary {
    color: var(--text-secondary);
    background: transparent;
    border: 1px solid var(--separator-strong);
  }

  .primary {
    color: var(--on-accent);
    background: var(--accent);
    border: 1px solid var(--accent);
  }

  @media (max-width: 520px) {
    .field-grid.two,
    .currency-row {
      grid-template-columns: 1fr;
    }
  }
</style>
