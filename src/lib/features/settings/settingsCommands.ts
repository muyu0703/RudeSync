import type { AppSettings, InvoiceProfile } from "./types";

/**
 * The Settings UI keeps its assumed native command names in this one place.
 * This makes a backend contract change a small, explicit edit.
 */
export const SETTINGS_COMMANDS = {
  getSettings: "get_settings",
  updateSettings: "update_settings",
  getInvoiceProfile: "get_invoice_profile",
  updateInvoiceProfile: "update_invoice_profile",
  runBackup: "run_backup",
  runManualBackup: "run_manual_backup",
  restoreBackup: "restore_backup",
} as const;

export const settingsCommandArgs = {
  getSettings: () => ({}),
  updateSettings: (input: AppSettings) => ({ input }),
  getInvoiceProfile: () => ({}),
  updateInvoiceProfile: (input: InvoiceProfile) => ({ input }),
  runBackup: () => ({}),
  runManualBackup: (destinationPath: string) => ({ destinationPath }),
  restoreBackup: (sourcePath: string) => ({ sourcePath }),
};
