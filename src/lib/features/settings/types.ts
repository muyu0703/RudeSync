export type InvoiceTerm =
  | "immediate"
  | "7-days"
  | "14-days"
  | "30-days";

export type DateFormat = "MMMM d, yyyy" | "MM/dd/yyyy" | "yyyy-MM-dd";

export type WeekStart = 0 | 1;

export interface AppSettings {
  defaultCurrency: string;
  defaultInvoiceTerm: InvoiceTerm;
  dateFormat: DateFormat;
  weekStartsOn: WeekStart;
  autostartEnabled: boolean;
  autostartRegistered: boolean;
  closeToTray: boolean;
  notificationsEnabled: boolean;
  backupDirectory: string | null;
  backupEnabled: boolean;
  backupIntervalHours: 24;
  backupRetentionCount: number;
  lastBackupAt: string | null;
  lastBackupAttemptAt: string | null;
  lastBackupError: string | null;
  backupSetupRequired: boolean;
}

export interface InvoiceProfile {
  id: string | null;
  profileName: string;
  displayName: string;
  businessName: string;
  address: string;
  email: string;
  logoPath: string;
  paymentInstructions: string;
}

export interface BackupResult {
  destinationPath: string;
  fileName: string;
  completedAt: string;
  sizeBytes: number | null;
}

export interface RestoreResult {
  safetyBackupPath: string;
  restoredAt: string;
}

export interface SettingsService {
  readonly isDesktop: boolean;
  getSettings(): Promise<AppSettings>;
  updateSettings(settings: AppSettings): Promise<AppSettings>;
  getInvoiceProfile(): Promise<InvoiceProfile>;
  updateInvoiceProfile(profile: InvoiceProfile): Promise<InvoiceProfile>;
  chooseBackupDirectory(currentPath: string | null): Promise<string | null>;
  chooseLogoPath(currentPath: string): Promise<string | null>;
  runManualBackup(): Promise<BackupResult | null>;
  restoreFromBackup(): Promise<RestoreResult | null>;
}

export const DEFAULT_SETTINGS: AppSettings = {
  defaultCurrency: "USD",
  defaultInvoiceTerm: "14-days",
  dateFormat: "MMMM d, yyyy",
  weekStartsOn: 1,
  autostartEnabled: true,
  autostartRegistered: false,
  closeToTray: true,
  notificationsEnabled: true,
  backupDirectory: null,
  backupEnabled: true,
  backupIntervalHours: 24,
  backupRetentionCount: 30,
  lastBackupAt: null,
  lastBackupAttemptAt: null,
  lastBackupError: null,
  backupSetupRequired: true,
};

export const DEFAULT_INVOICE_PROFILE: InvoiceProfile = {
  id: null,
  profileName: "Default",
  displayName: "",
  businessName: "",
  address: "",
  email: "",
  logoPath: "",
  paymentInstructions: "",
};
