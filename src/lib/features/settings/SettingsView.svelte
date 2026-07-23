<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import DataExports from "./DataExports.svelte";
  import { createSettingsService } from "./settingsService";
  import {
    DEFAULT_INVOICE_PROFILE,
    DEFAULT_SETTINGS,
    type AppSettings,
    type BackupResult,
    type InvoiceProfile,
  } from "./types";

  const service = createSettingsService();

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
  <header class="settings-hero">
    <div>
      <span class="eyebrow">Workspace preferences</span>
      <h2>Make RudeSync yours.</h2>
      <p>
        Set up invoices, daily behavior, and backups. Your working data stays
        on this device.
      </p>
    </div>
    <div class="local-chip" title="RudeSync works without a cloud connection">
      <span class="pulse" aria-hidden="true"></span>
      <span>
        <strong>Local-first</strong>
        <small>{service.isDesktop ? "Desktop database" : "Browser preview storage"}</small>
      </span>
    </div>
  </header>

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
        type="button"
        aria-label="Dismiss message"
        on:click={() => (successMessage = "")}
      ><Icon name="x" size={13} /></button>
    </div>
  {/if}

  {#if loading}
    <div class="loading-grid" aria-label="Loading settings">
      <span></span><span></span><span></span>
    </div>
  {:else}
    <form
      class="settings-form"
      on:submit|preventDefault={saveAll}
      on:input={clearFeedback}
      on:change={clearFeedback}
    >
      <section class="settings-card profile-card" aria-labelledby="profile-title">
        <header class="card-header">
          <div class="card-icon"><Icon name="invoice" size={18} /></div>
          <div>
            <span class="card-kicker">PDF invoices</span>
            <h3 id="profile-title">Invoice profile</h3>
            <p>This information appears on printable invoices.</p>
          </div>
          {#if profileDirty}<span class="unsaved-badge">Unsaved</span>{/if}
        </header>

        <div class="field-grid">
          <label class="field">
            <span>Your name <b>Required for invoices</b></span>
            <input
              bind:value={profile.displayName}
              type="text"
              maxlength="160"
              autocomplete="name"
              placeholder="Your full name"
            />
          </label>

          <label class="field">
            <span>Business name <b>Optional</b></span>
            <input
              bind:value={profile.businessName}
              type="text"
              maxlength="160"
              autocomplete="organization"
              placeholder="Studio or company name"
            />
          </label>

          <label class="field wide">
            <span>Business address</span>
            <textarea
              bind:value={profile.address}
              rows="3"
              maxlength="1000"
              autocomplete="street-address"
              placeholder="Address shown in the invoice header"
            ></textarea>
          </label>

          <label class="field">
            <span>Email or contact</span>
            <input
              bind:value={profile.email}
              type="text"
              maxlength="240"
              autocomplete="email"
              placeholder="you@example.com"
            />
          </label>

          <div class="field">
            <label for="invoice-logo">Logo path <b>Optional</b></label>
            <div class="path-control">
              <input
                id="invoice-logo"
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
            <span>Payment instructions</span>
            <textarea
              bind:value={profile.paymentInstructions}
              rows="4"
              maxlength="4000"
              placeholder="Bank, transfer, or payment details for your clients"
            ></textarea>
          </label>
        </div>
      </section>

      <div class="two-column">
        <section class="settings-card" aria-labelledby="defaults-title">
          <header class="card-header compact">
            <div class="card-icon"><Icon name="palette" size={18} /></div>
            <div>
              <span class="card-kicker">Defaults</span>
              <h3 id="defaults-title">Money and calendar</h3>
            </div>
          </header>

          <div class="field-stack">
            <label class="field">
              <span>Default currency</span>
              <input
                class="short-input"
                bind:value={settings.defaultCurrency}
                type="text"
                maxlength="3"
                inputmode="text"
                aria-describedby="currency-help"
              />
              <small id="currency-help">USD by default; each client can override it.</small>
            </label>

            <label class="field">
              <span>Default invoice term</span>
              <select bind:value={settings.defaultInvoiceTerm}>
                <option value="immediate">Due immediately</option>
                <option value="7-days">Due in 7 days</option>
                <option value="14-days">Due in 14 days</option>
                <option value="30-days">Due in 30 days</option>
              </select>
            </label>

            <label class="field">
              <span>Date display</span>
              <select bind:value={settings.dateFormat}>
                <option value="MMMM d, yyyy">July 23, 2026</option>
                <option value="MM/dd/yyyy">07/23/2026</option>
                <option value="yyyy-MM-dd">2026-07-23</option>
              </select>
            </label>

            <label class="field">
              <span>Week starts on</span>
              <select bind:value={settings.weekStartsOn}>
                <option value={1}>Monday</option>
                <option value={0}>Sunday</option>
              </select>
            </label>
          </div>
        </section>

        <section class="settings-card" aria-labelledby="behavior-title">
          <header class="card-header compact">
            <div class="card-icon"><Icon name="tray" size={18} /></div>
            <div>
              <span class="card-kicker">Windows behavior</span>
              <h3 id="behavior-title">Startup and alerts</h3>
            </div>
          </header>

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
        </section>
      </div>

      <section class="settings-card backup-card" aria-labelledby="backup-title">
        <header class="card-header">
          <div class="card-icon"><Icon name="database" size={18} /></div>
          <div>
            <span class="card-kicker">Recovery</span>
            <h3 id="backup-title">Local backups</h3>
            <p>Keep one automatic copy each day and remove older copies safely.</p>
          </div>
          <span class:active={settings.backupEnabled} class="state-badge">
            {settings.backupEnabled ? "Daily" : "Paused"}
          </span>
        </header>

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
              <label for="backup-folder">Backup folder</label>
              <div class="path-control">
                <input
                  id="backup-folder"
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
              <small id="backup-folder-help">
                {service.isDesktop
                  ? "Automatic retained copies use this folder; manual copies can be saved elsewhere."
                  : "Browser preview downloads a portable JSON backup instead."}
              </small>
            </div>
          </div>

          <div class="backup-meta">
            <label class="field retention-field">
              <span>Keep latest copies</span>
              <div class="number-control">
                <input
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
              class="backup-button"
              type="button"
              disabled={backingUp || saving || restoring}
              on:click={runManualBackup}
            >
              <Icon name="database" size={14} />
              {backingUp ? "Backing up…" : "Create manual backup"}
            </button>

            {#if service.isDesktop}
              <button
                class="restore-button"
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
      </section>

      <DataExports />

      <aside class="privacy-note" aria-label="Local-first data status">
        <span class="privacy-icon"><Icon name="shield" size={18} /></span>
        <div>
          <strong>Your workspace is local-first.</strong>
          <p>
            Tasks, projects, invoices, and loan schedules remain on this
            device. Cloud sync is not enabled in this version.
          </p>
        </div>
        <span class="local-status">
          <i aria-hidden="true"></i>
          On device
        </span>
      </aside>

      <footer class="save-bar">
        <span>
          {hasChanges
            ? "You have unsaved changes."
            : "Everything is up to date."}
        </span>
        <div>
          <button
            class="secondary-button"
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
      </footer>
    </form>
  {/if}
</section>

<style>
  .settings-view {
    width: min(100%, 1120px);
    margin: 0 auto;
    color: var(--text-primary, #eef5f1);
  }

  .settings-hero {
    display: flex;
    min-height: 116px;
    align-items: center;
    justify-content: space-between;
    gap: 28px;
    margin-bottom: 16px;
    padding: 22px 24px;
    background: var(--surface-1, #0e1512);
    border: 1px solid var(--border-subtle, #1b2922);
    border-radius: 12px;
  }

  .eyebrow,
  .card-kicker {
    color: var(--accent, #43d17f);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .settings-hero h2 {
    margin: 6px 0 5px;
    font-size: clamp(20px, 2.5vw, 28px);
    font-weight: 630;
    letter-spacing: -0.035em;
  }

  .settings-hero p,
  .card-header p {
    max-width: 620px;
    margin: 0;
    color: var(--text-muted, #84938b);
    font-size: 11.5px;
    line-height: 1.55;
  }

  .local-chip {
    display: flex;
    min-width: 168px;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background: var(--accent-soft, rgba(67, 209, 127, 0.08));
    border: 1px solid var(--accent-border, rgba(67, 209, 127, 0.2));
    border-radius: 9px;
  }

  .local-chip > span:last-child {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .local-chip strong {
    color: var(--text-secondary, #c6d2cc);
    font-size: 10.5px;
    font-weight: 650;
  }

  .local-chip small {
    margin-top: 2px;
    color: var(--text-muted, #84938b);
    font-size: 8.5px;
  }

  .pulse {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    background: var(--accent, #43d17f);
    border-radius: 50%;
    box-shadow: 0 0 0 4px var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .message {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 9px;
    min-height: 40px;
    margin-bottom: 12px;
    padding: 8px 10px 8px 13px;
    font-size: 10.5px;
    border: 1px solid;
    border-radius: 8px;
  }

  .message.error {
    color: #ffb9b4;
    background: rgba(185, 65, 58, 0.1);
    border-color: rgba(239, 118, 111, 0.24);
  }

  .message.success {
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.08));
    border-color: var(--accent-border, rgba(67, 209, 127, 0.2));
  }

  .message.warning {
    color: var(--amber, #e6b85c);
    background: var(--amber-soft, rgba(230, 184, 92, 0.1));
    border-color: rgba(230, 184, 92, 0.24);
  }

  .message button {
    display: grid;
    padding: 5px;
    place-items: center;
    color: inherit;
    background: transparent;
    border: 0;
    border-radius: 5px;
    cursor: pointer;
  }

  .message .message-action {
    min-width: 88px;
    padding: 6px 9px;
    color: #171208;
    font-size: 9px;
    font-weight: 700;
    background: var(--amber, #e6b85c);
  }

  .loading-grid {
    display: grid;
    gap: 12px;
  }

  .loading-grid span {
    min-height: 150px;
    background: var(--surface-1, #0e1512);
    border: 1px solid var(--border-subtle, #1b2922);
    border-radius: 12px;
    animation: settings-pulse 1.2s ease-in-out infinite alternate;
  }

  .loading-grid span:first-child {
    min-height: 330px;
  }

  @keyframes settings-pulse {
    to { opacity: 0.52; }
  }

  .settings-form {
    display: grid;
    gap: 14px;
  }

  .settings-card {
    display: block;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    gap: 0;
    padding: 0;
    background: var(--surface-1, #0e1512);
    border: 1px solid var(--border-subtle, #1b2922);
    border-radius: 12px;
  }

  .card-header {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    min-height: 76px;
    padding: 14px 17px;
    border-bottom: 1px solid var(--border-subtle, #1b2922);
  }

  .card-header.compact {
    min-height: 68px;
  }

  .card-icon,
  .privacy-icon {
    display: grid;
    width: 36px;
    height: 36px;
    place-items: center;
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.08));
    border-radius: 9px;
  }

  .card-header h3 {
    margin: 3px 0 0;
    font-size: 13px;
    font-weight: 620;
  }

  .card-header p {
    margin-top: 4px;
    font-size: 9.5px;
  }

  .unsaved-badge,
  .state-badge {
    padding: 4px 7px;
    color: var(--amber, #e6b85c);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.05em;
    background: var(--amber-soft, rgba(230, 184, 92, 0.1));
    border: 1px solid rgba(230, 184, 92, 0.2);
    border-radius: 99px;
    text-transform: uppercase;
  }

  .state-badge {
    color: var(--text-muted, #84938b);
    background: var(--surface-raised, #18221d);
    border-color: var(--border-subtle, #1b2922);
  }

  .state-badge.active {
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.08));
    border-color: var(--accent-border, rgba(67, 209, 127, 0.2));
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 15px 18px;
    padding: 18px;
  }

  .field-grid .wide {
    grid-column: 1 / -1;
  }

  .field,
  .field-stack {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .field-stack {
    gap: 15px;
    padding: 18px;
  }

  .field > span,
  .field > label,
  .field label {
    margin-bottom: 6px;
    color: var(--text-secondary, #c6d2cc);
    font-size: 9.5px;
    font-weight: 590;
  }

  .field b {
    margin-left: 4px;
    color: var(--text-faint, #5f6c65);
    font-size: 8px;
    font-weight: 520;
  }

  .field small {
    margin-top: 6px;
    color: var(--text-faint, #5f6c65);
    font-size: 8.5px;
    line-height: 1.45;
  }

  input,
  select,
  textarea {
    width: 100%;
    min-width: 0;
    color: var(--text-primary, #eef5f1);
    font: inherit;
    font-size: 10.5px;
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-strong, #26362e);
    border-radius: 7px;
    outline: none;
  }

  input,
  select {
    height: 36px;
    padding: 0 10px;
  }

  textarea {
    min-height: 68px;
    padding: 9px 10px;
    line-height: 1.5;
    resize: vertical;
  }

  input::placeholder,
  textarea::placeholder {
    color: var(--text-faint, #5f6c65);
  }

  input:focus,
  select:focus,
  textarea:focus {
    border-color: var(--accent-border, rgba(67, 209, 127, 0.38));
    box-shadow: 0 0 0 3px rgba(67, 209, 127, 0.05);
  }

  input[readonly] {
    color: var(--text-muted, #84938b);
    cursor: default;
  }

  .short-input {
    text-transform: uppercase;
  }

  .path-control {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 7px;
  }

  .compact-button,
  .secondary-button,
  .primary-button,
  .backup-button,
  .restore-button {
    display: inline-flex;
    min-height: 34px;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 12px;
    color: var(--text-secondary, #c6d2cc);
    font: inherit;
    font-size: 9.5px;
    font-weight: 650;
    background: var(--surface-raised, #18221d);
    border: 1px solid var(--border-strong, #26362e);
    border-radius: 7px;
    cursor: pointer;
  }

  .compact-button:hover:not(:disabled),
  .secondary-button:hover:not(:disabled),
  .restore-button:hover:not(:disabled) {
    color: var(--text-primary, #eef5f1);
    border-color: var(--accent-border, rgba(67, 209, 127, 0.38));
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .two-column {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
  }

  .toggle-list {
    display: grid;
    padding: 5px 17px;
  }

  .toggle-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    min-height: 63px;
    cursor: pointer;
  }

  .toggle-list .toggle-row + .toggle-row {
    border-top: 1px solid var(--border-subtle, #1b2922);
  }

  .toggle-row > span {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .toggle-row strong {
    color: var(--text-secondary, #c6d2cc);
    font-size: 10.5px;
    font-weight: 590;
  }

  .toggle-row small {
    margin-top: 4px;
    color: var(--text-muted, #84938b);
    font-size: 8.5px;
    line-height: 1.4;
  }

  .toggle-row input {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
  }

  .toggle-row i {
    position: relative;
    width: 34px;
    height: 18px;
    background: var(--surface-raised, #18221d);
    border: 1px solid var(--border-strong, #26362e);
    border-radius: 99px;
    transition: 120ms ease;
  }

  .toggle-row i::after {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 10px;
    height: 10px;
    content: "";
    background: var(--text-faint, #5f6c65);
    border-radius: 50%;
    transition: 120ms ease;
  }

  .toggle-row input:checked + i {
    background: var(--accent-soft, rgba(67, 209, 127, 0.12));
    border-color: var(--accent-border, rgba(67, 209, 127, 0.4));
  }

  .toggle-row input:checked + i::after {
    left: 19px;
    background: var(--accent, #43d17f);
  }

  .toggle-row input:focus-visible + i {
    outline: 2px solid var(--accent, #43d17f);
    outline-offset: 2px;
  }

  .registration-note {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 2px 17px 15px;
    padding: 8px 9px;
    color: var(--text-muted, #84938b);
    font-size: 8.5px;
    background: var(--surface-0, #090d0b);
    border-radius: 6px;
  }

  .registration-note span {
    width: 5px;
    height: 5px;
    background: var(--amber, #e6b85c);
    border-radius: 50%;
  }

  .registration-note.registered span {
    background: var(--accent, #43d17f);
  }

  .backup-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(245px, 0.6fr);
  }

  .backup-main,
  .backup-meta {
    display: grid;
    gap: 17px;
    padding: 17px;
  }

  .backup-meta {
    align-content: start;
    background: var(--surface-0, #090d0b);
    border-left: 1px solid var(--border-subtle, #1b2922);
  }

  .toggle-row.standalone {
    min-height: 48px;
  }

  .number-control {
    display: grid;
    grid-template-columns: 82px auto;
    align-items: center;
    gap: 8px;
  }

  .number-control span {
    color: var(--text-muted, #84938b);
    font-size: 9px;
  }

  .last-backup {
    display: flex;
    min-width: 0;
    flex-direction: column;
    padding-top: 2px;
  }

  .last-backup > span {
    color: var(--text-faint, #5f6c65);
    font-size: 8px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .last-backup strong {
    margin-top: 5px;
    color: var(--text-secondary, #c6d2cc);
    font-size: 9.5px;
    font-weight: 560;
  }

  .last-backup small {
    margin-top: 3px;
    overflow: hidden;
    color: var(--text-faint, #5f6c65);
    font-size: 8px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .backup-button,
  .primary-button {
    color: #06110b;
    background: var(--accent, #43d17f);
    border-color: transparent;
  }

  .backup-button:hover:not(:disabled),
  .primary-button:hover:not(:disabled) {
    background: var(--accent-bright, #5be493);
  }

  .backup-button {
    justify-self: start;
  }

  .restore-button {
    justify-self: start;
  }

  .privacy-note {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 14px 16px;
    background: var(--accent-soft, rgba(67, 209, 127, 0.07));
    border: 1px solid var(--accent-border, rgba(67, 209, 127, 0.19));
    border-radius: 10px;
  }

  .privacy-note strong {
    color: var(--text-secondary, #c6d2cc);
    font-size: 10.5px;
    font-weight: 620;
  }

  .privacy-note p {
    max-width: 700px;
    margin: 3px 0 0;
    color: var(--text-muted, #84938b);
    font-size: 8.8px;
    line-height: 1.45;
  }

  .local-status {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--accent, #43d17f);
    font-size: 8.5px;
    font-weight: 650;
  }

  .local-status i {
    width: 5px;
    height: 5px;
    background: currentColor;
    border-radius: 50%;
  }

  .save-bar {
    position: sticky;
    bottom: -30px;
    z-index: 2;
    display: flex;
    min-height: 60px;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 10px 13px 10px 16px;
    background: rgba(13, 19, 16, 0.96);
    border: 1px solid var(--border-strong, #26362e);
    border-radius: 10px;
    box-shadow: 0 -8px 30px rgba(0, 0, 0, 0.16);
  }

  .save-bar > span {
    color: var(--text-muted, #84938b);
    font-size: 9.5px;
  }

  .save-bar > div {
    display: flex;
    gap: 7px;
  }

  @media (max-width: 880px) {
    .two-column,
    .backup-layout {
      grid-template-columns: 1fr;
    }

    .backup-meta {
      border-top: 1px solid var(--border-subtle, #1b2922);
      border-left: 0;
    }
  }

  @media (max-width: 650px) {
    .settings-hero {
      align-items: flex-start;
      flex-direction: column;
    }

    .local-chip {
      width: 100%;
    }

    .field-grid {
      grid-template-columns: 1fr;
    }

    .field-grid .wide {
      grid-column: auto;
    }

    .card-header {
      grid-template-columns: 38px minmax(0, 1fr);
    }

    .card-header > :last-child {
      grid-column: 2;
      justify-self: start;
    }

    .privacy-note {
      grid-template-columns: 38px minmax(0, 1fr);
    }

    .privacy-note .local-status {
      grid-column: 2;
    }

    .save-bar {
      bottom: -42px;
      align-items: stretch;
      flex-direction: column;
    }

    .save-bar > div {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
      animation-duration: 0.01ms !important;
      animation-iteration-count: 1 !important;
      transition-duration: 0.01ms !important;
    }
  }
</style>
