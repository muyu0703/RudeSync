import { invoke } from "@tauri-apps/api/core";
import { SETTINGS_COMMANDS, settingsCommandArgs } from "./settingsCommands";
import {
  DEFAULT_INVOICE_PROFILE,
  DEFAULT_SETTINGS,
  type AppSettings,
  type BackupResult,
  type DateFormat,
  type InvoiceProfile,
  type InvoiceTerm,
  type RestoreResult,
  type SettingsService,
  type WeekStart,
} from "./types";

const SETTINGS_STORAGE_KEY = "rudesync.settings.v1";
const PROFILE_STORAGE_KEY = "rudesync.invoice-profile.v1";

interface TauriWindow extends Window {
  __TAURI_INTERNALS__?: unknown;
  __TAURI__?: unknown;
}

interface DirectoryPickerWindow extends Window {
  showDirectoryPicker?: () => Promise<{ name: string }>;
}

function isTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  const candidate = window as TauriWindow;
  return Boolean(candidate.__TAURI_INTERNALS__ || candidate.__TAURI__);
}

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}

function field(
  value: Record<string, unknown>,
  camel: string,
  snake: string,
): unknown {
  return value[camel] ?? value[snake];
}

function textValue(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function nullableText(value: unknown): string | null {
  if (value == null) return null;
  const valueAsText = String(value).trim();
  return valueAsText || null;
}

function booleanValue(value: unknown, fallback: boolean): boolean {
  if (typeof value === "boolean") return value;
  if (value === 1 || value === "1") return true;
  if (value === 0 || value === "0") return false;
  return fallback;
}

function integerValue(value: unknown, fallback: number): number {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : fallback;
}

function parseInvoiceTerm(value: unknown): InvoiceTerm {
  const normalized = String(value ?? "").replaceAll("_", "-");
  if (
    normalized === "immediate" ||
    normalized === "7-days" ||
    normalized === "14-days" ||
    normalized === "30-days"
  ) {
    return normalized;
  }
  const days = Number(value);
  if (days === 0) return "immediate";
  if (days === 7) return "7-days";
  if (days === 30) return "30-days";
  return "14-days";
}

function parseDateFormat(value: unknown): DateFormat {
  if (
    value === "MMMM d, yyyy" ||
    value === "MM/dd/yyyy" ||
    value === "yyyy-MM-dd"
  ) {
    return value;
  }
  return DEFAULT_SETTINGS.dateFormat;
}

function parseWeekStart(value: unknown): WeekStart {
  return Number(value) === 0 ? 0 : 1;
}

export function parseSettings(value: unknown): AppSettings {
  const raw = record(value);
  return {
    defaultCurrency: textValue(
      field(raw, "defaultCurrency", "default_currency"),
      DEFAULT_SETTINGS.defaultCurrency,
    )
      .trim()
      .toUpperCase(),
    defaultInvoiceTerm: parseInvoiceTerm(
      field(raw, "defaultInvoiceTerm", "default_invoice_term") ??
        field(raw, "defaultInvoiceTermDays", "default_invoice_term_days"),
    ),
    dateFormat: parseDateFormat(field(raw, "dateFormat", "date_format")),
    weekStartsOn: parseWeekStart(
      field(raw, "weekStartsOn", "week_starts_on"),
    ),
    autostartEnabled: booleanValue(
      field(raw, "autostartEnabled", "autostart_enabled"),
      DEFAULT_SETTINGS.autostartEnabled,
    ),
    autostartRegistered: booleanValue(
      field(raw, "autostartRegistered", "autostart_registered"),
      false,
    ),
    closeToTray: booleanValue(
      field(raw, "closeToTray", "close_to_tray"),
      DEFAULT_SETTINGS.closeToTray,
    ),
    notificationsEnabled: booleanValue(
      field(raw, "notificationsEnabled", "notifications_enabled"),
      DEFAULT_SETTINGS.notificationsEnabled,
    ),
    backupDirectory: nullableText(
      field(raw, "backupDirectory", "backup_directory") ??
        field(raw, "directoryPath", "directory_path"),
    ),
    backupEnabled: booleanValue(
      field(raw, "backupEnabled", "backup_enabled") ??
        field(raw, "isEnabled", "is_enabled"),
      DEFAULT_SETTINGS.backupEnabled,
    ),
    backupIntervalHours: 24,
    backupRetentionCount: Math.max(
      1,
      integerValue(
        field(raw, "backupRetentionCount", "backup_retention_count") ??
          field(raw, "retentionCount", "retention_count"),
        DEFAULT_SETTINGS.backupRetentionCount,
      ),
    ),
    lastBackupAt: nullableText(
      field(raw, "lastBackupAt", "last_backup_at") ??
        field(raw, "lastSuccessAt", "last_success_at"),
    ),
    lastBackupAttemptAt: nullableText(
      field(raw, "lastBackupAttemptAt", "last_backup_attempt_at"),
    ),
    lastBackupError: nullableText(
      field(raw, "lastBackupError", "last_backup_error"),
    ),
    backupSetupRequired: booleanValue(
      field(raw, "backupSetupRequired", "backup_setup_required"),
      !nullableText(
        field(raw, "backupDirectory", "backup_directory") ??
          field(raw, "directoryPath", "directory_path"),
      ),
    ),
  };
}

export function parseInvoiceProfile(value: unknown): InvoiceProfile {
  const raw = record(value);
  return {
    id: nullableText(raw.id),
    profileName: textValue(
      field(raw, "profileName", "profile_name"),
      DEFAULT_INVOICE_PROFILE.profileName,
    ),
    displayName: textValue(
      field(raw, "displayName", "display_name"),
    ),
    businessName: textValue(
      field(raw, "businessName", "business_name"),
    ),
    address: textValue(raw.address),
    email: textValue(raw.email),
    logoPath: textValue(field(raw, "logoPath", "logo_path")),
    paymentInstructions: textValue(
      field(raw, "paymentInstructions", "payment_instructions"),
    ),
  };
}

function parseBackupResult(value: unknown): BackupResult {
  if (typeof value === "string") {
    const normalized = value.replaceAll("\\", "/");
    const pieces = normalized.split("/");
    return {
      destinationPath: value,
      fileName: pieces[pieces.length - 1] || "rudesync-backup.db",
      completedAt: new Date().toISOString(),
      sizeBytes: null,
    };
  }

  const raw = record(value);
  return {
    destinationPath: textValue(
      field(raw, "destinationPath", "destination_path") ??
        field(raw, "path", "path"),
      "Backup folder",
    ),
    fileName: textValue(
      field(raw, "fileName", "file_name"),
      "rudesync-backup.db",
    ),
    completedAt: textValue(
      field(raw, "completedAt", "completed_at") ??
        field(raw, "createdAt", "created_at"),
      new Date().toISOString(),
    ),
    sizeBytes:
      field(raw, "sizeBytes", "size_bytes") == null
        ? null
        : Math.max(
            0,
            integerValue(field(raw, "sizeBytes", "size_bytes"), 0),
          ),
  };
}

function parseRestoreResult(value: unknown): RestoreResult {
  const raw = record(value);
  return {
    safetyBackupPath: textValue(
      field(raw, "safetyBackupPath", "safety_backup_path"),
    ),
    restoredAt: textValue(
      field(raw, "restoredAt", "restored_at"),
      new Date().toISOString(),
    ),
  };
}

function manualBackupFileName(now = new Date()): string {
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  const hour = String(now.getHours()).padStart(2, "0");
  const minute = String(now.getMinutes()).padStart(2, "0");
  const second = String(now.getSeconds()).padStart(2, "0");
  return `RudeSync-manual-backup-${year}-${month}-${day}-${hour}${minute}${second}.sqlite3`;
}

function readStored<T>(key: string, fallback: T): T {
  try {
    const stored = window.localStorage.getItem(key);
    return stored ? (JSON.parse(stored) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeStored(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    throw new Error("Browser storage is unavailable.");
  }
}

function cloneSettings(value: AppSettings): AppSettings {
  return { ...value };
}

function cloneProfile(value: InvoiceProfile): InvoiceProfile {
  return { ...value };
}

async function chooseWithTauriDialog(
  options: Parameters<
    typeof import("@tauri-apps/plugin-dialog").open
  >[0],
): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open(options);
  return typeof selected === "string" ? selected : null;
}

async function chooseBrowserDirectory(
  currentPath: string | null,
): Promise<string | null> {
  const browserWindow = window as DirectoryPickerWindow;
  if (!browserWindow.showDirectoryPicker) {
    return currentPath || "Browser downloads";
  }
  try {
    const handle = await browserWindow.showDirectoryPicker();
    return handle.name ? `Browser downloads / ${handle.name}` : null;
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") {
      return null;
    }
    return currentPath || "Browser downloads";
  }
}

function chooseBrowserLogo(): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/webp,image/svg+xml";
    input.addEventListener(
      "change",
      () => resolve(input.files?.item(0)?.name ?? null),
      { once: true },
    );
    input.click();
  });
}

class BrowserSettingsService implements SettingsService {
  readonly isDesktop = false;

  async getSettings(): Promise<AppSettings> {
    return parseSettings(
      readStored(SETTINGS_STORAGE_KEY, DEFAULT_SETTINGS),
    );
  }

  async updateSettings(settings: AppSettings): Promise<AppSettings> {
    const updated = parseSettings(settings);
    writeStored(SETTINGS_STORAGE_KEY, updated);
    return cloneSettings(updated);
  }

  async getInvoiceProfile(): Promise<InvoiceProfile> {
    return parseInvoiceProfile(
      readStored(PROFILE_STORAGE_KEY, DEFAULT_INVOICE_PROFILE),
    );
  }

  async updateInvoiceProfile(
    profile: InvoiceProfile,
  ): Promise<InvoiceProfile> {
    const updated = parseInvoiceProfile(profile);
    writeStored(PROFILE_STORAGE_KEY, updated);
    return cloneProfile(updated);
  }

  async chooseBackupDirectory(
    currentPath: string | null,
  ): Promise<string | null> {
    return chooseBrowserDirectory(currentPath);
  }

  async chooseLogoPath(): Promise<string | null> {
    return chooseBrowserLogo();
  }

  async runManualBackup(): Promise<BackupResult> {
    const timestamp = new Date();
    const fileName = manualBackupFileName(timestamp).replace(
      /\.sqlite3$/,
      ".json",
    );
    const payload = JSON.stringify(
      {
        format: "rudesync-browser-backup-v1",
        exportedAt: timestamp.toISOString(),
        settings: await this.getSettings(),
        invoiceProfile: await this.getInvoiceProfile(),
      },
      null,
      2,
    );
    const blob = new Blob([payload], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = fileName;
    anchor.click();
    URL.revokeObjectURL(url);

    return {
      destinationPath: "Browser downloads",
      fileName,
      completedAt: timestamp.toISOString(),
      sizeBytes: blob.size,
    };
  }

  async restoreFromBackup(): Promise<RestoreResult | null> {
    throw new Error(
      "Database restore is available only in the RudeSync desktop app.",
    );
  }
}

class TauriSettingsService implements SettingsService {
  readonly isDesktop = true;

  async getSettings(): Promise<AppSettings> {
    const result = await invoke<unknown>(
      SETTINGS_COMMANDS.getSettings,
      settingsCommandArgs.getSettings(),
    );
    return parseSettings(result);
  }

  async updateSettings(settings: AppSettings): Promise<AppSettings> {
    const result = await invoke<unknown>(
      SETTINGS_COMMANDS.updateSettings,
      settingsCommandArgs.updateSettings(settings),
    );
    return parseSettings(result);
  }

  async getInvoiceProfile(): Promise<InvoiceProfile> {
    const result = await invoke<unknown>(
      SETTINGS_COMMANDS.getInvoiceProfile,
      settingsCommandArgs.getInvoiceProfile(),
    );
    return parseInvoiceProfile(result);
  }

  async updateInvoiceProfile(
    profile: InvoiceProfile,
  ): Promise<InvoiceProfile> {
    const result = await invoke<unknown>(
      SETTINGS_COMMANDS.updateInvoiceProfile,
      settingsCommandArgs.updateInvoiceProfile(profile),
    );
    return parseInvoiceProfile(result);
  }

  async chooseBackupDirectory(): Promise<string | null> {
    return chooseWithTauriDialog({
      title: "Choose RudeSync backup folder",
      directory: true,
      multiple: false,
    });
  }

  async chooseLogoPath(): Promise<string | null> {
    return chooseWithTauriDialog({
      title: "Choose invoice logo",
      directory: false,
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "webp", "svg"],
        },
      ],
    });
  }

  async runManualBackup(): Promise<BackupResult | null> {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const destinationPath = await save({
      title: "Create a manual RudeSync backup",
      defaultPath: manualBackupFileName(),
      filters: [
        {
          name: "RudeSync database",
          extensions: ["sqlite3", "db"],
        },
      ],
    });
    if (!destinationPath) return null;
    const result = await invoke<unknown>(
      SETTINGS_COMMANDS.runManualBackup,
      settingsCommandArgs.runManualBackup(destinationPath),
    );
    return parseBackupResult(result);
  }

  async restoreFromBackup(): Promise<RestoreResult | null> {
    const sourcePath = await chooseWithTauriDialog({
      title: "Choose a RudeSync backup to restore",
      directory: false,
      multiple: false,
      filters: [
        {
          name: "RudeSync database",
          extensions: ["sqlite3", "db"],
        },
      ],
    });
    if (!sourcePath) return null;
    const result = await invoke<unknown>(
      SETTINGS_COMMANDS.restoreBackup,
      settingsCommandArgs.restoreBackup(sourcePath),
    );
    return parseRestoreResult(result);
  }
}

export function createSettingsService(): SettingsService {
  return isTauriRuntime()
    ? new TauriSettingsService()
    : new BrowserSettingsService();
}
