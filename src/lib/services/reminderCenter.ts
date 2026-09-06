type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

interface TauriWindow extends Window {
  __TAURI_INTERNALS__?: { invoke?: Invoke };
  __TAURI__?: { core?: { invoke?: Invoke } };
}

export type ReminderKind = "alarm" | "timer";
export type ReminderStatus = "active" | "paused" | "ringing";

export interface ReminderItem {
  id: string;
  kind: ReminderKind;
  title: string;
  scheduledAt: string | null;
  durationMinutes: number | null;
  remainingMinutes: number | null;
  status: ReminderStatus;
  recurrenceRule: string | null;
  createdAt: string;
  updatedAt: string;
}

function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const tauriWindow = window as TauriWindow;
  const fn =
    tauriWindow.__TAURI_INTERNALS__?.invoke ??
    tauriWindow.__TAURI__?.core?.invoke;
  if (!fn) {
    return Promise.reject(new Error("提醒功能仅在桌面应用中可用。"));
  }
  return fn<T>(command, args);
}

export const reminderCenter = {
  list(): Promise<ReminderItem[]> {
    return invoke<ReminderItem[]>("list_reminder_center");
  },
  createAlarm(title: string, scheduledAt: string, recurrenceRule: string | null): Promise<ReminderItem> {
    return invoke<ReminderItem>("create_alarm", {
      input: { title, scheduledAt, recurrenceRule },
    });
  },
  createTimer(title: string, minutes: number): Promise<ReminderItem> {
    return invoke<ReminderItem>("create_timer", { input: { title, minutes } });
  },
  pause(id: string): Promise<ReminderItem> {
    return invoke<ReminderItem>("pause_timer", { id });
  },
  resume(id: string): Promise<ReminderItem> {
    return invoke<ReminderItem>("resume_timer", { id });
  },
  reset(id: string): Promise<ReminderItem> {
    return invoke<ReminderItem>("reset_timer", { id });
  },
  acknowledge(id: string): Promise<void> {
    return invoke<void>("acknowledge_reminder", { id });
  },
  snooze(id: string, minutes: number): Promise<ReminderItem> {
    return invoke<ReminderItem>("snooze_reminder", { id, minutes });
  },
  remove(id: string): Promise<void> {
    return invoke<void>("delete_reminder_center", { id });
  },
};
