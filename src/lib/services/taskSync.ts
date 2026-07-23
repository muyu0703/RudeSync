export const TASKS_CHANGED_EVENT = "tasks-changed";
export const OPEN_TASK_EVENT = "open-task";

function inTauri(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

export async function emitTasksChanged(): Promise<void> {
  if (!inTauri()) return;
  const { emit } = await import("@tauri-apps/api/event");
  await emit(TASKS_CHANGED_EVENT);
}

export async function onTasksChanged(
  handler: () => void,
): Promise<() => void> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen(TASKS_CHANGED_EVENT, () => handler());
}

export async function emitOpenTask(taskId: string): Promise<void> {
  if (!inTauri()) return;
  const { emit } = await import("@tauri-apps/api/event");
  await emit(OPEN_TASK_EVENT, taskId);
}

export async function onOpenTask(
  handler: (taskId: string) => void,
): Promise<() => void> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>(OPEN_TASK_EVENT, (event) => handler(event.payload));
}
