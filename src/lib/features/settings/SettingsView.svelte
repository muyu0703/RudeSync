<script lang="ts">
  import { onMount } from "svelte";
  import Card from "../../components/Card.svelte";
  import Icon from "../../components/Icon.svelte";
  import SectionHeader from "../../components/SectionHeader.svelte";
  import DataExports from "./DataExports.svelte";
  import { createSettingsService } from "./settingsService";
  import {
    DEFAULT_INVOICE_PROFILE,
    DEFAULT_SETTINGS,
    type AppSettings,
    type BackupResult,
    type InvoiceProfile,
  } from "./types";
  import {
    getMotionPreference,
    setMotionPreference,
    type MotionPreference,
  } from "../../motion";

  const service = createSettingsService();

  // Bindable so the shell (App.svelte) can consult it before navigating away
  // and discarding unsaved edits. Computed below, same as `settingsDirty` /
  // `profileDirty` — this component still owns the value, App.svelte only
  // reads it.
  export let hasChanges = false;

  const MOTION_OPTIONS: [MotionPreference, string][] = [
    ["system", "Follow system"],
    ["always", "Always on"],
    ["reduced", "Off"],
  ];

  let settings: AppSettings = { ...DEFAULT_SETTINGS };
  let profile: InvoiceProfile = { ...DEFAULT_INVOICE_PROFILE };
  let savedSettings = JSON.stringify(settings);
  let savedProfile = JSON.stringify(profile);
  let loading = true;
  let saving = false;
  let choosingFolder = false;
  let choosingLogo = false;
  let backingUp = false;
  let restoring = false;
  let errorMessage = "";
  let successMessage = "";
  let latestBackup: BackupResult | null = null;
  let motionPreference: MotionPreference = getMotionPreference();

  $: settingsDirty = JSON.stringify(settings) !== savedSettings;
  $: profileDirty = JSON.stringify(profile) !== savedProfile;
  $: hasChanges = settingsDirty || profileDirty;
  $: backupDestination =
    settings.backupDirectory ||
    (service.isDesktop ? "No folder selected" : "Browser downloads");
  $: backupWarning = !service.isDesktop || !settings.backupEnabled
    ? ""
    : settings.backupSetupRequired || !settings.backupDirectory
      ? "Choose a dedicated folder to activate automatic daily backups."
      : settings.lastBackupError
        ? `Automatic backup needs attention: ${settings.lastBackupError}`
        : "";

  onMount(() => {
    void loadSettings();
  });

  async function loadSettings(): Promise<void> {
    loading = true;
    errorMessage = "";
    try {
      [settings, profile] = await Promise.all([
        service.getSettings(),
        service.getInvoiceProfile(),
      ]);
      savedSettings = JSON.stringify(settings);
      savedProfile = JSON.stringify(profile);
    } catch (error) {
      errorMessage = errorText(
        error,
        "Your settings could not be loaded.",
      );
    } finally {
      loading = false;
    }
  }

  function validate(): void {
    const currency = settings.defaultCurrency.trim().toUpperCase();
    if (!/^[A-Z]{3}$/.test(currency)) {
      throw new Error("Default currency must be a three-letter code.");
    }
    if (
      !Number.isSafeInteger(settings.backupRetentionCount) ||
      settings.backupRetentionCount < 1 ||
      settings.backupRetentionCount > 365
    ) {
      throw new Error("Backup retention must be between 1 and 365 copies.");
    }
    if (profile.email && profile.email.length > 240) {
      throw new Error("Email or contact details are too long.");
    }
    settings.defaultCurrency = currency;
    profile.displayName = profile.displayName.trim();
    profile.businessName = profile.businessName.trim();
    profile.email = profile.email.trim();
  }

  async function saveAll(): Promise<void> {
    if (!hasChanges || saving) return;
    saving = true;
    errorMessage = "";
    successMessage = "";
    try {
      validate();
      const operations: Promise<unknown>[] = [];
      if (settingsDirty) {
        operations.push(
          service.updateSettings(settings).then((updated) => {
            settings = updated;
            savedSettings = JSON.stringify(updated);
          }),
        );
      }
      if (profileDirty) {
        operations.push(
          service.updateInvoiceProfile(profile).then((updated) => {
            profile = updated;
            savedProfile = JSON.stringify(updated);
          }),
        );
      }
      await Promise.all(operations);
      successMessage = "Settings saved.";
    } catch (error) {
      errorMessage = errorText(error, "Your settings could not be saved.");
    } finally {
      saving = false;
    }
  }

  function discardChanges(): void {
    if (saving) return;
    settings = JSON.parse(savedSettings) as AppSettings;
    profile = JSON.parse(savedProfile) as InvoiceProfile;
    errorMessage = "";
    successMessage = "Unsaved changes discarded.";
  }

  async function chooseBackupFolder(): Promise<void> {
    if (choosingFolder) return;
    choosingFolder = true;
    errorMessage = "";
    successMessage = "";
    try {
      const saveSetupImmediately =
        service.isDesktop && settings.backupSetupRequired && !settingsDirty;
      const selected = await service.chooseBackupDirectory(
        settings.backupDirectory,
      );
      if (selected) {
        settings = { ...settings, backupDirectory: selected };
        if (saveSetupImmediately) {
          settings = await service.updateSettings(settings);
          savedSettings = JSON.stringify(settings);
          successMessage =
            "Backup folder saved. Automatic daily backups are active.";
        }
        if (!service.isDesktop) {
          successMessage =
            "Browser preview selected a folder label. Backups download through your browser.";
        }
      }
    } catch (error) {
      errorMessage = errorText(error, "The folder picker could not open.");
    } finally {
      choosingFolder = false;
    }
  }

  async function chooseLogo(): Promise<void> {
    if (choosingLogo) return;
    choosingLogo = true;
    errorMessage = "";
    successMessage = "";
    try {
      const selected = await service.chooseLogoPath(profile.logoPath);
      if (selected) profile = { ...profile, logoPath: selected };
    } catch (error) {
      errorMessage = errorText(error, "The logo picker could not open.");
    } finally {
      choosingLogo = false;
    }
  }

  async function runManualBackup(): Promise<void> {
    if (backingUp || saving) return;
    backingUp = true;
    errorMessage = "";
    successMessage = "";
    try {
      latestBackup = await service.runManualBackup();
      if (!latestBackup) return;
      successMessage = `Backup complete: ${latestBackup.fileName}`;
    } catch (error) {
      errorMessage = errorText(error, "The backup could not be completed.");
    } finally {
      backingUp = false;
    }
  }

  async function restoreFromBackup(): Promise<void> {
    if (restoring || saving || backingUp) return;
    if (
      !window.confirm(
        "Restore a RudeSync backup? Current data will be preserved in a pre-restore safety copy, then this screen will reload.",
      )
    ) {
      return;
    }
    restoring = true;
    errorMessage = "";
    successMessage = "";
    try {
      const restored = await service.restoreFromBackup();
      if (!restored) return;
      successMessage = `Restore complete. Safety copy: ${restored.safetyBackupPath}`;
      window.setTimeout(() => window.location.reload(), 250);
    } catch (error) {
      errorMessage = errorText(error, "The backup could not be restored.");
    } finally {
      restoring = false;
    }
  }

  function clearFeedback(): void {
    successMessage = "";
  }

  function selectMotionPreference(value: MotionPreference): void {
    motionPreference = value;
    setMotionPreference(value);
  }

  function errorText(error: unknown, fallback: string): string {
    if (error instanceof Error && error.message) return error.message;
    if (typeof error === "string" && error.trim()) return error;
    return fallback;
  }

  function formatTimestamp(value: string | null): string {
    if (!value) return "No backup recorded yet";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat("en-US", {
      month: "long",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  }

  function formatBytes(value: number | null): string {
    if (value == null) return "";
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<section
  class="settings-view"
  aria-label="RudeSync settings"
  aria-busy={loading}
>
  <div class="intro-header">
    <SectionHeader
      title="Make RudeSync yours."
      subtext="Set up invoices, daily behavior, and backups. Your working data stays on this device."
    >
      <svelte:fragment slot="actions">
        <span class="status-pill" title="RudeSync works without a cloud connection">
          Local-first · {service.isDesktop ? "Desktop database" : "Browser preview storage"}
        </span>
      </svelte:fragment>
    </SectionHeader>
  </div>

  {#if !loading && backupWarning}
    <div class="message warning" role="status">
      <Icon name="database" size={14} />
      <span>{backupWarning}</span>
      <button
        class="message-action"
        type="button"
        disabled={choosingFolder}
        on:click={chooseBackupFolder}
      >{choosingFolder ? "Opening…" : "Choose folder"}</button>
    </div>
  {/if}

  {#if errorMessage}
    <div class="message error" role="alert">
      <Icon name="circle" size={14} />
      <span>{errorMessage}</span>
      <button
        class="dismiss"
        type="button"
        aria-label="Dismiss error"
        on:click={() => (errorMessage = "")}
      ><Icon name="x" size={13} /></button>
    </div>
  {/if}

  {#if successMessage}
    <div class="message success" role="status">
      <Icon name="check" size={14} strokeWidth={2.2} />
      <span>{successMessage}</span>
      <button
        class="dismiss"
        type="button"
        aria-label="Dismiss message"
        on:click={() => (successMessage = "")}
      ><Icon name="x" size={13} /></button>
    </div>
  {/if}

  {#if loading}
    <div class="skeleton-list" aria-label="Loading settings">
      <span></span><span></span><span></span>
    </div>
  {:else}
    <form
      class="settings-form"
      on:submit|preventDefault={saveAll}
      on:input={clearFeedback}
      on:change={clearFeedback}
    >
      <div class="page-actions">
        <span class="save-status">
          {hasChanges
            ? "You have unsaved changes."
            : "Everything is up to date."}
        </span>
        <button
          class="text-button"
          type="button"
          disabled={!hasChanges || saving}
          on:click={discardChanges}
        >Discard</button>
        <button
          class="primary-button"
          type="submit"
          disabled={!hasChanges || saving}
        >
          <Icon name="check" size={14} strokeWidth={2.2} />
          {saving ? "Saving…" : "Save changes"}
        </button>
      </div>

      <Card>
        <SectionHeader
          slot="header"
          title="Invoice profile"
          subtext="This information appears on printable invoices."
        >
          <svelte:fragment slot="actions">
            {#if profileDirty}<span class="unsaved-badge">Unsaved</span>{/if}
          </svelte:fragment>
        </SectionHeader>

        <div class="field-grid">
          <label class="field">
            <span class="field-label">Your name <b>Required for invoices</b></span>
            <input
              class="field-input"
              bind:value={profile.displayName}
              type="text"
              maxlength="160"
              autocomplete="name"
              placeholder="Your full name"
            />
          </label>

          <label class="field">
            <span class="field-label">Business name <b>Optional</b></span>
            <input
              class="field-input"
              bind:value={profile.businessName}
              type="text"
              maxlength="160"
              autocomplete="organization"
              placeholder="Studio or company name"
            />
          </label>

          <label class="field wide">
            <span class="field-label">Business address</span>
            <textarea
              class="field-textarea"
              bind:value={profile.address}
              rows="3"
              maxlength="1000"
              autocomplete="street-address"
              placeholder="Address shown in the invoice header"
            ></textarea>
          </label>

          <label class="field">
            <span class="field-label">Email or contact</span>
            <input
              class="field-input"
              bind:value={profile.email}
              type="text"
              maxlength="240"
              autocomplete="email"
              placeholder="you@example.com"
            />
          </label>

          <div class="field">
            <label class="field-label" for="invoice-logo">Logo path <b>Optional</b></label>
            <div class="path-control">
              <input
                id="invoice-logo"
                class="field-input"
                bind:value={profile.logoPath}
                type="text"
                maxlength="1000"
                placeholder="No logo selected"
              />
              <button
                class="compact-button"
                type="button"
                disabled={choosingLogo}
                on:click={chooseLogo}
              >{choosingLogo ? "Opening…" : "Browse"}</button>
            </div>
          </div>

          <label class="field wide">
            <span class="field-label">Payment instructions</span>
            <textarea
              class="field-textarea"
              bind:value={profile.paymentInstructions}
              rows="4"
              maxlength="4000"
              placeholder="Bank, transfer, or payment details for your clients"
            ></textarea>
          </label>
        </div>
      </Card>

      <div class="settings-grid">
        <Card>
          <SectionHeader slot="header" title="Money and calendar" subtext="Defaults" />

          <div class="field-stack">
            <label class="field">
              <span class="field-label">Default currency</span>
              <input
                class="field-input short-input"
                bind:value={settings.defaultCurrency}
                type="text"
                maxlength="3"
                inputmode="text"
                aria-describedby="currency-help"
              />
              <small id="currency-help" class="field-hint">USD by default; each client can override it.</small>
            </label>

            <label class="field">
              <span class="field-label">Default invoice term</span>
              <select class="field-input" bind:value={settings.defaultInvoiceTerm}>
                <option value="immediate">Due immediately</option>
                <option value="7-days">Due in 7 days</option>
                <option value="14-days">Due in 14 days</option>
                <option value="30-days">Due in 30 days</option>
              </select>
            </label>

            <label class="field">
              <span class="field-label">Date display</span>
              <select class="field-input" bind:value={settings.dateFormat}>
                <option value="MMMM d, yyyy">July 23, 2026</option>
                <option value="MM/dd/yyyy">07/23/2026</option>
                <option value="yyyy-MM-dd">2026-07-23</option>
              </select>
            </label>

            <label class="field">
              <span class="field-label">Week starts on</span>
              <select class="field-input" bind:value={settings.weekStartsOn}>
                <option value={1}>Monday</option>
                <option value={0}>Sunday</option>
              </select>
            </label>
          </div>
        </Card>

        <Card>
          <SectionHeader slot="header" title="Startup and alerts" subtext="Windows behavior" />

          <div class="toggle-list">
            <label class="toggle-row">
              <span>
                <strong>Open when Windows starts</strong>
                <small>Keep today’s plan ready after sign-in.</small>
              </span>
              <input
                bind:checked={settings.autostartEnabled}
                type="checkbox"
              />
              <i aria-hidden="true"></i>
            </label>

            <label class="toggle-row">
              <span>
                <strong>Close to system tray</strong>
                <small>The X button hides RudeSync; Quit closes it fully.</small>
              </span>
              <input bind:checked={settings.closeToTray} type="checkbox" />
              <i aria-hidden="true"></i>
            </label>

            <label class="toggle-row">
              <span>
                <strong>Windows notifications</strong>
                <small>Tasks, invoice dates, and personal loan due dates.</small>
              </span>
              <input
                bind:checked={settings.notificationsEnabled}
                type="checkbox"
              />
              <i aria-hidden="true"></i>
            </label>
          </div>

          {#if settings.autostartEnabled && service.isDesktop}
            <div class:registered={settings.autostartRegistered} class="registration-note">
              <span aria-hidden="true"></span>
              {settings.autostartRegistered
                ? "Startup registration is active."
                : "Startup registration will be applied when settings are saved."}
            </div>
          {/if}
        </Card>

        <Card>
          <SectionHeader
            slot="header"
            title="Appearance"
            subtext="How RudeSync animates on this device."
          >
            <svelte:fragment slot="actions">
              <span class="soft-badge" title="This control saves itself; it does not use the Save changes button below.">Applies immediately</span>
            </svelte:fragment>
          </SectionHeader>

          <div class="field">
            <span class="field-label">Animations</span>
            <div class="segmented" aria-label="Animations">
              {#each MOTION_OPTIONS as option}
                <button
                  class:active={motionPreference === option[0]}
                  type="button"
                  aria-pressed={motionPreference === option[0]}
                  on:click={() => selectMotionPreference(option[0])}
                >{option[1]}</button>
              {/each}
            </div>
            <small class="field-hint">Applies immediately and is saved on this device only. Follow system uses your Windows animation setting.</small>
          </div>
        </Card>
      </div>

      <Card>
        <SectionHeader
          slot="header"
          title="Local backups"
          subtext="Keep one automatic copy each day and remove older copies safely."
        >
          <svelte:fragment slot="actions">
            <span class={settings.backupEnabled ? "status-pill" : "soft-badge"}>
              {settings.backupEnabled ? "Daily" : "Paused"}
            </span>
          </svelte:fragment>
        </SectionHeader>

        <div class="backup-layout">
          <div class="backup-main">
            <label class="toggle-row standalone">
              <span>
                <strong>Automatic daily backup</strong>
                <small>Creates one automatic copy per local calendar day.</small>
              </span>
              <input bind:checked={settings.backupEnabled} type="checkbox" />
              <i aria-hidden="true"></i>
            </label>

            <div class="field">
              <label class="field-label" for="backup-folder">Backup folder</label>
              <div class="path-control">
                <input
                  id="backup-folder"
                  class="field-input"
                  value={backupDestination}
                  type="text"
                  readonly
                  aria-describedby="backup-folder-help"
                />
                <button
                  class="compact-button"
                  type="button"
                  disabled={choosingFolder}
                  on:click={chooseBackupFolder}
                >{choosingFolder ? "Opening…" : "Choose folder"}</button>
              </div>
              <small id="backup-folder-help" class="field-hint">
                {service.isDesktop
                  ? "Automatic retained copies use this folder; manual copies can be saved elsewhere."
                  : "Browser preview downloads a portable JSON backup instead."}
              </small>
            </div>
          </div>

          <div class="backup-meta">
            <label class="field retention-field">
              <span class="field-label">Keep latest copies</span>
              <div class="number-control">
                <input
                  class="field-input"
                  bind:value={settings.backupRetentionCount}
                  type="number"
                  min="1"
                  max="365"
                  step="1"
                />
                <span>copies</span>
              </div>
            </label>

            <div class="last-backup">
              <span>{latestBackup ? "Latest manual backup" : "Last automatic backup"}</span>
              <strong>{formatTimestamp(latestBackup?.completedAt ?? settings.lastBackupAt)}</strong>
              {#if latestBackup?.sizeBytes != null}
                <small>{latestBackup.destinationPath} · {formatBytes(latestBackup.sizeBytes)}</small>
              {/if}
            </div>

            <button
              class="primary-button"
              type="button"
              disabled={backingUp || saving || restoring}
              on:click={runManualBackup}
            >
              <Icon name="database" size={14} />
              {backingUp ? "Backing up…" : "Create manual backup"}
            </button>

            {#if service.isDesktop}
              <button
                class="compact-button"
                type="button"
                disabled={restoring || saving || backingUp}
                on:click={restoreFromBackup}
              >
                <Icon name="shield" size={14} />
                {restoring ? "Restoring…" : "Restore from backup"}
              </button>
            {/if}
          </div>
        </div>
      </Card>

      <DataExports />

      <aside class="local-card" aria-label="Local-first data status">
        <Icon name="shield" size={18} />
        <div>
          <strong>Your workspace is local-first.</strong>
          <span>
            Tasks, projects, invoices, and loan schedules remain on this
            device. Cloud sync is not enabled in this version.
          </span>
        </div>
        <span class="on-device">
          <i aria-hidden="true"></i>
          On device
        </span>
      </aside>
    </form>
  {/if}
</section>

<style>
  .settings-view {
    width: min(100%, 1120px);
    margin: 0 auto;
  }

  .intro-header {
    margin-bottom: var(--space-4);
  }

  .message {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    min-height: 40px;
    margin: 0 0 var(--space-4);
    padding: var(--space-2) var(--space-3);
    font-size: var(--text-12);
    border-radius: var(--radius-control);
  }
  .message.error { color: var(--danger); background: var(--danger-fill); }
  .message.warning { color: var(--amber); background: var(--amber-fill); }
  .message.success { color: var(--accent); background: var(--accent-fill); }
  .message > span { flex: 1; min-width: 0; }
  .message .dismiss {
    display: grid;
    width: 26px;
    height: 26px;
    padding: 0;
    place-items: center;
    color: inherit;
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
  }
  .message-action {
    padding: var(--space-1) var(--space-2);
    color: var(--on-accent);
    font-size: var(--text-11);
    font-weight: var(--weight-medium);
    background: var(--amber);
    border: 0;
    border-radius: var(--radius-control);
    white-space: nowrap;
  }

  .save-status { color: var(--text-tertiary); font-size: var(--text-12); }

  /* The form runs several screens long, so the save row stays reachable
     instead of only living above the fold (finding 20). */
  .page-actions {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: var(--space-3) 0;
    background: var(--surface-window);
    border-bottom: 1px solid var(--separator);
  }

  .settings-form { display: grid; gap: var(--space-4); }

  .field { display: flex; min-width: 0; flex-direction: column; gap: var(--space-1); }
  .field-stack { display: grid; gap: var(--space-4); }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-4);
  }
  .field-grid .wide { grid-column: 1 / -1; }

  .field b {
    margin-left: var(--space-1);
    color: var(--text-tertiary);
    font-weight: var(--weight-regular);
  }

  select.field-input { color-scheme: dark; }
  .field-input[readonly] { color: var(--text-tertiary); cursor: default; }
  .short-input { text-transform: uppercase; }

  .path-control {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--space-2);
  }

  .compact-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-1);
    min-height: 32px;
    padding: 0 var(--space-3);
    white-space: nowrap;
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--text-12);
    font-weight: var(--weight-medium);
    background: var(--surface-raised);
    border: 1px solid var(--separator-strong);
    border-radius: var(--radius-control);
  }
  .compact-button:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--accent-line);
  }

  .toggle-list { display: grid; }
  .toggle-list .toggle-row + .toggle-row { border-top: 1px solid var(--separator); }
  .toggle-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-4);
    min-height: 56px;
    cursor: pointer;
  }
  .toggle-row.standalone { min-height: 44px; }
  .toggle-row > span { display: flex; min-width: 0; flex-direction: column; }
  .toggle-row strong { font-size: var(--text-13); font-weight: var(--weight-medium); }
  .toggle-row small { margin-top: 2px; color: var(--text-tertiary); font-size: var(--text-11); line-height: 1.4; }
  .toggle-row input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .toggle-row i {
    position: relative;
    width: 34px;
    height: 18px;
    background: var(--surface-raised);
    border: 1px solid var(--separator-strong);
    border-radius: var(--radius-pill);
    transition: background var(--duration) var(--ease), border-color var(--duration) var(--ease);
  }
  .toggle-row i::after {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    content: "";
    background: var(--text-tertiary);
    border-radius: 50%;
    transition: left var(--duration) var(--ease), background var(--duration) var(--ease);
  }
  .toggle-row input:checked + i { background: var(--accent-fill); border-color: var(--accent-line); }
  .toggle-row input:checked + i::after { left: 18px; background: var(--accent); }
  .toggle-row input:focus-visible + i { outline: 2px solid var(--accent); outline-offset: 2px; }

  .registration-note {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-3);
    padding: var(--space-2) var(--space-3);
    color: var(--text-tertiary);
    font-size: var(--text-11);
    background: var(--surface-window);
    border-radius: var(--radius-control);
  }
  .registration-note span { width: 5px; height: 5px; background: var(--amber); border-radius: 50%; }
  .registration-note.registered span { background: var(--accent); }

  .backup-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(220px, 0.6fr);
    gap: var(--space-5);
  }
  .backup-main, .backup-meta { display: grid; align-content: start; gap: var(--space-4); }
  .backup-meta { padding-left: var(--space-4); border-left: 1px solid var(--separator); }

  .number-control { display: grid; grid-template-columns: 82px auto; align-items: center; gap: var(--space-2); }
  .number-control span { color: var(--text-tertiary); font-size: var(--text-11); }

  .last-backup { display: flex; min-width: 0; flex-direction: column; }
  .last-backup > span { color: var(--text-tertiary); font-size: var(--text-11); }
  .last-backup strong { margin-top: var(--space-1); font-size: var(--text-12); font-weight: var(--weight-medium); }
  .last-backup small {
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unsaved-badge {
    padding: 2px var(--space-2);
    color: var(--amber);
    font-size: var(--text-11);
    font-weight: var(--weight-medium);
    background: var(--amber-fill);
    border-radius: var(--radius-pill);
  }

  .on-device {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin-left: auto;
    color: var(--accent);
    font-size: var(--text-11);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }
  .on-device i { width: 5px; height: 5px; background: currentColor; border-radius: 50%; }

  @media (max-width: 880px) {
    .backup-layout { grid-template-columns: 1fr; }
    .backup-meta {
      padding-left: 0;
      padding-top: var(--space-4);
      border-left: 0;
      border-top: 1px solid var(--separator);
    }
  }

  @media (max-width: 650px) {
    .field-grid { grid-template-columns: 1fr; }
    .field-grid .wide { grid-column: auto; }
  }
</style>
