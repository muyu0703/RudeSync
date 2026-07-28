/**
 * Shared reduced-motion gate for Svelte transitions.
 *
 * Svelte 5's `transition:` / `in:` / `out:` directives animate through the
 * Web Animations API (`element.animate(...)`), not CSS animations, so they
 * are NOT affected by the `@media (prefers-reduced-motion: reduce)` rule in
 * `src/app.css` (which only zeroes CSS `animation-duration` /
 * `transition-duration`). Every Svelte transition in the app must route its
 * `duration` through `motionDuration()` below so it collapses to 0ms for
 * users who have requested reduced motion at the OS level.
 *
 * On top of the OS signal, the user can override it per-device from
 * Settings → Appearance: "always" forces transitions on even when the OS
 * requests reduced motion, "reduced" forces them off, and "system" (the
 * default) defers to `prefersReducedMotion` exactly as before. The override
 * is a device-level display preference, so it lives in `localStorage`, not
 * the synced SQLite settings table.
 *
 * Usage:
 *   import { motionDuration } from "../../motion"; // adjust to your depth
 *   <div transition:fade={{ duration: motionDuration(140) }}>
 */
import { prefersReducedMotion } from "svelte/motion";

export type MotionPreference = "system" | "always" | "reduced";

const STORAGE_KEY = "rudesync.motion-preference";

function isMotionPreference(value: unknown): value is MotionPreference {
  return value === "system" || value === "always" || value === "reduced";
}

/**
 * Reads the current motion preference from `localStorage`. Falls back to
 * `"system"` when unset, invalid, or when `localStorage` itself is
 * unavailable (private browsing, storage disabled, etc.) — this must never
 * throw, since it can run during app startup.
 */
export function getMotionPreference(): MotionPreference {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return isMotionPreference(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

/**
 * Mirrors the resolved preference onto `<html>` as `data-motion`, so the
 * stylesheet can honour it too.
 *
 * `motionDuration()` covers Svelte transitions, but CSS `transition` and
 * `animation` declarations are gated only by
 * `@media (prefers-reduced-motion: reduce)` — a media query that can see the
 * OS setting and nothing else. Without this attribute, choosing "Off" in
 * Settings silently left every CSS transition running, and choosing "Always
 * on" could not re-enable them for someone whose OS requests reduced motion.
 *
 * `"system"` removes the attribute entirely rather than writing a value, so
 * the media query alone decides — which is exactly what "follow system" means.
 */
export function applyMotionPreference(
  preference: MotionPreference = getMotionPreference(),
): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  if (preference === "always") root.dataset.motion = "full";
  else if (preference === "reduced") root.dataset.motion = "reduced";
  else delete root.dataset.motion;
}

/** Persists the motion preference for this device and applies it immediately. */
export function setMotionPreference(value: MotionPreference): void {
  try {
    localStorage.setItem(STORAGE_KEY, value);
  } catch {
    // Storage unavailable — the preference simply won't survive a reload.
  }
  // Applied even when persistence failed, so the current session still obeys.
  applyMotionPreference(value);
}

/**
 * Returns `ms`, or `0` when motion should be suppressed, resolving the
 * user's override first and falling back to the OS `prefers-reduced-motion`
 * signal when the preference is `"system"`.
 */
export function motionDuration(ms: number): number {
  const preference = getMotionPreference();
  if (preference === "reduced") return 0;
  if (preference === "always") return ms;
  return prefersReducedMotion.current ? 0 : ms;
}
