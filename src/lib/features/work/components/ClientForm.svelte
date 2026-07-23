<script lang="ts">
  import type { Client, CreateClientInput } from "../types";

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

<form on:submit|preventDefault={submit}>
  <div class="field-grid two">
    <label>
      <span>Client name <b aria-hidden="true">*</b></span>
      <input bind:value={name} maxlength="120" autocomplete="name" data-work-autofocus required />
    </label>
    <label>
      <span>Company</span>
      <input bind:value={companyName} maxlength="160" autocomplete="organization" />
    </label>
  </div>

  <div class="field-grid currency-row">
    <label>
      <span>Email</span>
      <input bind:value={email} maxlength="254" type="email" autocomplete="email" />
    </label>
    <label>
      <span>Currency</span>
      <input
        value={currency}
        on:input={(e) => (currency = e.currentTarget.value.toUpperCase())}
        maxlength="3"
        minlength="3"
        pattern={"[A-Z]{3}"}
        autocapitalize="characters"
        aria-describedby="currency-help"
        required
      />
      <small id="currency-help">Three-letter code</small>
    </label>
  </div>

  <label>
    <span>Billing address</span>
    <textarea bind:value={billingAddress} maxlength="600" rows="2"></textarea>
  </label>

  <label>
    <span>Private notes</span>
    <textarea bind:value={notes} maxlength="1200" rows="3"></textarea>
  </label>

  <footer>
    <button class="secondary" type="button" disabled={busy} on:click={onCancel}>Cancel</button>
    <button class="primary" type="submit" disabled={busy || !name.trim() || currency.length !== 3}>
      {busy ? "Saving…" : client ? "Save changes" : "Create client"}
    </button>
  </footer>
</form>

<style>
  form {
    display: grid;
    gap: 15px;
  }

  .field-grid {
    display: grid;
    gap: 13px;
  }

  .field-grid.two {
    grid-template-columns: 1fr 1fr;
  }

  .currency-row {
    grid-template-columns: minmax(0, 1fr) 112px;
  }

  label {
    display: grid;
    gap: 7px;
    min-width: 0;
  }

  label > span {
    color: var(--text-secondary, #aebdb4);
    font-size: 10.5px;
    font-weight: 600;
  }

  b {
    color: var(--accent, #43d17f);
    font-weight: inherit;
  }

  input,
  textarea {
    width: 100%;
    color: var(--text-primary, #edf5f0);
    font: inherit;
    font-size: 12px;
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-strong, #2a3a31);
    border-radius: 8px;
  }

  input {
    height: 38px;
    padding: 0 11px;
  }

  textarea {
    min-height: 62px;
    padding: 9px 11px;
    line-height: 1.5;
    resize: vertical;
  }

  input:hover,
  textarea:hover {
    border-color: #3a4c42;
  }

  input:focus,
  textarea:focus {
    border-color: var(--accent, #43d17f);
    outline: 1px solid var(--accent, #43d17f);
    outline-offset: 0;
  }

  small {
    color: var(--text-faint, #536158);
    font-size: 9px;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
    padding-top: 16px;
    border-top: 1px solid var(--border-subtle, #1b2821);
  }

  button {
    min-height: 36px;
    padding: 0 14px;
    font: inherit;
    font-size: 11px;
    font-weight: 650;
    border-radius: 8px;
    cursor: pointer;
  }

  button:focus-visible {
    outline: 2px solid var(--accent, #43d17f);
    outline-offset: 2px;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .secondary {
    color: var(--text-secondary, #aebdb4);
    background: transparent;
    border: 1px solid var(--border-strong, #2a3a31);
  }

  .primary {
    color: #07120c;
    background: var(--accent, #43d17f);
    border: 1px solid var(--accent, #43d17f);
  }

  @media (max-width: 520px) {
    .field-grid.two,
    .currency-row {
      grid-template-columns: 1fr;
    }
  }
</style>
