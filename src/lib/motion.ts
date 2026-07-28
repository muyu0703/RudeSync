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
 * Usage:
 *   import { motionDuration } from "../../motion"; // adjust to your depth
 *   <div transition:fade={{ duration: motionDuration(140) }}>
 */
import { prefersReducedMotion } from "svelte/motion";

/** Returns `ms`, or `0` when the user prefers reduced motion. */
export function motionDuration(ms: number): number {
  return prefersReducedMotion.current ? 0 : ms;
}
