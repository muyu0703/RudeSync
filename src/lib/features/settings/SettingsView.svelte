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
    ["system", "跟随系统"],
    ["always", "始终开启"],
    ["reduced", "关闭"],
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
    (service.isDesktop ? "尚未选择文件夹" : "浏览器下载");
  $: backupWarning = !service.isDesktop || !settings.backupEnabled
    ? ""
    : settings.backupSetupRequired || !settings.backupDirectory
      ? "请选择一个专用文件夹以启用每日自动备份。"
      : settings.lastBackupError
        ? `自动备份需要处理：${settings.lastBackupError}`
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
        "无法加载设置。",
      );
    } finally {
      loading = false;
    }
  }

  function validate(): void {
    const currency = settings.defaultCurrency.trim().toUpperCase();
    if (!/^[A-Z]{3}$/.test(currency)) {
      throw new Error("默认币种必须是 3 位字母代码。");
    }
    if (
      !Number.isSafeInteger(settings.backupRetentionCount) ||
      settings.backupRetentionCount < 1 ||
      settings.backupRetentionCount > 365
    ) {
      throw new Error("备份保留数量必须在 1 到 365 份之间。");
    }
    if (profile.email && profile.email.length > 240) {
      throw new Error("邮箱或联系方式过长。");
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
      successMessage = "设置已保存。";
    } catch (error) {
      errorMessage = errorText(error, "无法保存设置。");
    } finally {
      saving = false;
    }
  }

  function discardChanges(): void {
    if (saving) return;
    settings = JSON.parse(savedSettings) as AppSettings;
    profile = JSON.parse(savedProfile) as InvoiceProfile;
    errorMessage = "";
    successMessage = "已放弃未保存的修改。";
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
            "备份文件夹已保存，每日自动备份已启用。";
        }
        if (!service.isDesktop) {
          successMessage =
            "浏览器预览已选择文件夹名称，备份将通过浏览器下载。";
        }
      }
    } catch (error) {
      errorMessage = errorText(error, "无法打开文件夹选择器。");
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
      errorMessage = errorText(error, "无法打开 Logo 选择器。");
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
      successMessage = `备份完成：${latestBackup.fileName}`;
    } catch (error) {
      errorMessage = errorText(error, "备份未能完成。");
    } finally {
      backingUp = false;
    }
  }

  async function restoreFromBackup(): Promise<void> {
    if (restoring || saving || backingUp) return;
    if (
      !window.confirm(
        "恢复 RudeSync 备份吗？当前数据会先保存一份安全副本，然后重新加载此页面。",
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
      successMessage = `恢复完成。安全副本：${restored.safetyBackupPath}`;
      window.setTimeout(() => window.location.reload(), 250);
    } catch (error) {
      errorMessage = errorText(error, "无法恢复备份。");
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
    if (!value) return "暂无备份记录";
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
  aria-label="RudeSync 设置"
  aria-busy={loading}
>
  <div class="intro-header">
    <SectionHeader
      title="设置你的 RudeSync"
      subtext="设置发票、日常使用方式和备份；工作数据始终保存在本机。"
    >
      <svelte:fragment slot="actions">
        <span class="status-pill" title="RudeSync 无需云端连接也能使用">
          本地优先 · {service.isDesktop ? "桌面数据库" : "浏览器预览存储"}
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
      >{choosingFolder ? "正在打开…" : "选择文件夹"}</button>
    </div>
  {/if}

  {#if errorMessage}
    <div class="message error" role="alert">
      <Icon name="circle" size={14} />
      <span>{errorMessage}</span>
      <button
        class="dismiss"
        type="button"
        aria-label="关闭错误提示"
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
        aria-label="关闭提示"
        on:click={() => (successMessage = "")}
      ><Icon name="x" size={13} /></button>
    </div>
  {/if}

  {#if loading}
    <div class="skeleton-list" aria-label="正在加载设置">
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
            ? "你有尚未保存的修改。"
            : "所有设置均已保存。"}
        </span>
        <button
          class="text-button"
          type="button"
          disabled={!hasChanges || saving}
          on:click={discardChanges}
        >放弃</button>
        <button
          class="primary-button"
          type="submit"
          disabled={!hasChanges || saving}
        >
          <Icon name="check" size={14} strokeWidth={2.2} />
          {saving ? "保存中…" : "保存修改"}
        </button>
      </div>

      <Card>
        <SectionHeader
          slot="header"
          title="发票资料"
          subtext="这些信息会显示在可打印的发票上。"
        >
          <svelte:fragment slot="actions">
            {#if profileDirty}<span class="unsaved-badge">未保存</span>{/if}
          </svelte:fragment>
        </SectionHeader>

        <div class="field-grid">
          <label class="field">
            <span class="field-label">你的姓名 <b>发票必填</b></span>
            <input
              class="field-input"
              bind:value={profile.displayName}
              type="text"
              maxlength="160"
              autocomplete="name"
              placeholder="你的完整姓名"
            />
          </label>

          <label class="field">
            <span class="field-label">公司 / 店铺名称 <b>可选</b></span>
            <input
              class="field-input"
              bind:value={profile.businessName}
              type="text"
              maxlength="160"
              autocomplete="organization"
              placeholder="工作室或公司名称"
            />
          </label>

          <label class="field wide">
            <span class="field-label">地址</span>
            <textarea
              class="field-textarea"
              bind:value={profile.address}
              rows="3"
              maxlength="1000"
              autocomplete="street-address"
              placeholder="显示在发票抬头中的地址"
            ></textarea>
          </label>

          <label class="field">
            <span class="field-label">邮箱或联系方式</span>
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
            <label class="field-label" for="invoice-logo">Logo 路径 <b>可选</b></label>
            <div class="path-control">
              <input
                id="invoice-logo"
                class="field-input"
                bind:value={profile.logoPath}
                type="text"
                maxlength="1000"
                placeholder="未选择 Logo"
              />
              <button
                class="compact-button"
                type="button"
                disabled={choosingLogo}
                on:click={chooseLogo}
              >{choosingLogo ? "正在打开…" : "浏览"}</button>
            </div>
          </div>

          <label class="field wide">
            <span class="field-label">付款说明</span>
            <textarea
              class="field-textarea"
              bind:value={profile.paymentInstructions}
              rows="4"
              maxlength="4000"
              placeholder="给客户的银行、转账或付款信息"
            ></textarea>
          </label>
        </div>
      </Card>

      <div class="settings-grid">
        <Card>
          <SectionHeader slot="header" title="财务与日历" subtext="默认设置" />

          <div class="field-stack">
            <label class="field">
              <span class="field-label">默认币种</span>
              <input
                class="field-input short-input"
                bind:value={settings.defaultCurrency}
                type="text"
                maxlength="3"
                inputmode="text"
                aria-describedby="currency-help"
              />
              <small id="currency-help" class="field-hint">默认 USD；每个客户可单独设置。</small>
            </label>

            <label class="field">
              <span class="field-label">默认付款期限</span>
              <select class="field-input" bind:value={settings.defaultInvoiceTerm}>
                <option value="immediate">立即到期</option>
                <option value="7-days">7天后到期</option>
                <option value="14-days">14天后到期</option>
                <option value="30-days">30天后到期</option>
              </select>
            </label>

            <label class="field">
              <span class="field-label">日期显示</span>
              <select class="field-input" bind:value={settings.dateFormat}>
                <option value="MMMM d, yyyy">July 23, 2026</option>
                <option value="MM/dd/yyyy">07/23/2026</option>
                <option value="yyyy-MM-dd">2026-07-23</option>
              </select>
            </label>

            <label class="field">
              <span class="field-label">每周开始日</span>
              <select class="field-input" bind:value={settings.weekStartsOn}>
                <option value={1}>周一</option>
                <option value={0}>周日</option>
              </select>
            </label>
          </div>
        </Card>

        <Card>
          <SectionHeader slot="header" title="启动与提醒" subtext="Windows 行为" />

          <div class="toggle-list">
            <label class="toggle-row">
              <span>
                <strong>Windows 启动时自动运行</strong>
                <small>登录后自动准备好今日计划。</small>
              </span>
              <input
                bind:checked={settings.autostartEnabled}
                type="checkbox"
              />
              <i aria-hidden="true"></i>
            </label>

            <label class="toggle-row">
              <span>
                <strong>关闭到系统托盘</strong>
                <small>点击 X 隐藏到托盘；退出会完全关闭。</small>
              </span>
              <input bind:checked={settings.closeToTray} type="checkbox" />
              <i aria-hidden="true"></i>
            </label>

            <label class="toggle-row">
              <span>
                <strong>Windows 通知</strong>
                <small>任务、发票日期和个人借款到期提醒。</small>
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
                ? "开机启动已启用。"
                : "保存设置后将启用开机启动。"}
            </div>
          {/if}
        </Card>

        <Card>
          <SectionHeader
            slot="header"
            title="外观"
            subtext="设置 RudeSync 在本机上的动画效果。"
          >
            <svelte:fragment slot="actions">
              <span class="soft-badge" title="此项会自动保存，无需点击下方保存按钮。">立即生效</span>
            </svelte:fragment>
          </SectionHeader>

          <div class="field">
            <span class="field-label">动画</span>
            <div class="segmented" aria-label="动画">
              {#each MOTION_OPTIONS as option}
                <button
                  class:active={motionPreference === option[0]}
                  type="button"
                  aria-pressed={motionPreference === option[0]}
                  on:click={() => selectMotionPreference(option[0])}
                >{option[1]}</button>
              {/each}
            </div>
            <small class="field-hint">设置会立即生效并仅保存在本机；“跟随系统”会使用 Windows 的动画设置。</small>
          </div>
        </Card>
      </div>

      <Card>
        <SectionHeader
          slot="header"
          title="本地备份"
          subtext="每天自动保留一份备份，并安全清理较旧副本。"
        >
          <svelte:fragment slot="actions">
            <span class={settings.backupEnabled ? "status-pill" : "soft-badge"}>
              {settings.backupEnabled ? "每天" : "已暂停"}
            </span>
          </svelte:fragment>
        </SectionHeader>

        <div class="backup-layout">
          <div class="backup-main">
            <label class="toggle-row standalone">
              <span>
                <strong>每日自动备份</strong>
                <small>每天自动创建一份本地备份。</small>
              </span>
              <input bind:checked={settings.backupEnabled} type="checkbox" />
              <i aria-hidden="true"></i>
            </label>

            <div class="field">
              <label class="field-label" for="backup-folder">备份文件夹</label>
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
                >{choosingFolder ? "正在打开…" : "选择文件夹"}</button>
              </div>
              <small id="backup-folder-help" class="field-hint">
                {service.isDesktop
                  ? "自动备份会保存在此文件夹；手动备份可另存到其他位置。"
                  : "浏览器预览模式会下载可迁移的 JSON 备份文件。"}
              </small>
            </div>
          </div>

          <div class="backup-meta">
            <label class="field retention-field">
              <span class="field-label">保留最近备份</span>
              <div class="number-control">
                <input
                  class="field-input"
                  bind:value={settings.backupRetentionCount}
                  type="number"
                  min="1"
                  max="365"
                  step="1"
                />
                <span>份</span>
              </div>
            </label>

            <div class="last-backup">
              <span>{latestBackup ? "Latest manual backup" : "最近一次自动备份"}</span>
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
              {backingUp ? "正在备份…" : "创建手动备份"}
            </button>

            {#if service.isDesktop}
              <button
                class="compact-button"
                type="button"
                disabled={restoring || saving || backingUp}
                on:click={restoreFromBackup}
              >
                <Icon name="shield" size={14} />
                {restoring ? "正在恢复…" : "从备份恢复"}
              </button>
            {/if}
          </div>
        </div>
      </Card>

      <DataExports />

      <aside class="local-card" aria-label="本地数据状态">
        <Icon name="shield" size={18} />
        <div>
          <strong>你的工作区以本地数据为主。</strong>
          <span>
            任务、项目、发票和借款计划都保存在本机。此版本未启用云同步。
          </span>
        </div>
        <span class="on-device">
          <i aria-hidden="true"></i>
          保存在本机
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
