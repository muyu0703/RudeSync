const WIDGET_IDLE_OPACITY_KEY = "rudesync.widget.idle-opacity.v1";

export const DEFAULT_WIDGET_IDLE_OPACITY = 50;
export const MIN_WIDGET_IDLE_OPACITY = 30;
export const MAX_WIDGET_IDLE_OPACITY = 90;

export function normalizeWidgetIdleOpacity(value: unknown): number {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return DEFAULT_WIDGET_IDLE_OPACITY;
  return Math.min(
    MAX_WIDGET_IDLE_OPACITY,
    Math.max(MIN_WIDGET_IDLE_OPACITY, Math.round(numeric / 5) * 5),
  );
}

export function getWidgetIdleOpacity(): number {
  if (typeof window === "undefined") return DEFAULT_WIDGET_IDLE_OPACITY;
  try {
    return normalizeWidgetIdleOpacity(
      window.localStorage.getItem(WIDGET_IDLE_OPACITY_KEY),
    );
  } catch {
    return DEFAULT_WIDGET_IDLE_OPACITY;
  }
}

export function setWidgetIdleOpacity(value: number): number {
  const normalized = normalizeWidgetIdleOpacity(value);
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(
        WIDGET_IDLE_OPACITY_KEY,
        String(normalized),
      );
    } catch {
      // Keep the live setting even if local storage is unavailable.
    }
  }
  return normalized;
}
