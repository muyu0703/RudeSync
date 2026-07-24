import type { ProjectMilestone } from "./types.ts";

export function kickoffCompletion(totalMinor: number): ProjectMilestone[] {
  const kickoff = Math.floor(totalMinor / 2);
  return [
    { label: "Kickoff", amountMinor: kickoff, kind: "kickoff", sortOrder: 0 },
    { label: "Completion", amountMinor: totalMinor - kickoff, kind: "completion", sortOrder: 1 },
  ];
}

export function evenWeekly(weeks: number, amountMinor: number): ProjectMilestone[] {
  const count = Math.max(0, Math.floor(weeks));
  return Array.from({ length: count }, (_, i) => ({
    label: `Week ${i + 1}`,
    amountMinor,
    kind: "weekly" as const,
    sortOrder: i,
  }));
}

export function phases(count: number): ProjectMilestone[] {
  const n = Math.max(0, Math.floor(count));
  return Array.from({ length: n }, (_, i) => ({
    label: `Phase ${i + 1}`,
    amountMinor: 0,
    kind: "phase" as const,
    sortOrder: i,
  }));
}

export function planTotalMinor(ms: ProjectMilestone[]): number {
  return ms.reduce((sum, m) => sum + (m.amountMinor || 0), 0);
}
