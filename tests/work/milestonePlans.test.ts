import { test } from "node:test";
import assert from "node:assert/strict";
import {
  kickoffCompletion,
  evenWeekly,
  phases,
  planTotalMinor,
} from "../../src/lib/features/work/milestonePlans.ts";

test("kickoffCompletion splits 50/50 with remainder on completion", () => {
  const ms = kickoffCompletion(300001);
  assert.equal(ms.length, 2);
  // Pin which milestone absorbs the odd minor unit, not just the sum:
  // kickoff floors, completion takes the remainder.
  assert.equal(ms[0].amountMinor, 150000);
  assert.equal(ms[1].amountMinor, 150001);
  assert.equal(ms[0].amountMinor + ms[1].amountMinor, 300001);
  assert.equal(ms[0].kind, "kickoff");
  assert.equal(ms[1].kind, "completion");
});

test("evenWeekly generates N weekly milestones with the amount", () => {
  const ms = evenWeekly(4, 25000);
  assert.equal(ms.length, 4);
  assert.deepEqual(ms.map((m) => m.amountMinor), [25000, 25000, 25000, 25000]);
  assert.equal(ms[0].label, "Week 1");
  assert.equal(ms[3].kind, "weekly");
});

test("phases seeds `count` phase milestones", () => {
  const ms = phases(3);
  assert.equal(ms.length, 3);
  assert.equal(ms[0].label, "Phase 1");
  assert.equal(ms[2].kind, "phase");
});

test("planTotalMinor sums amounts", () => {
  assert.equal(planTotalMinor(evenWeekly(3, 10000)), 30000);
});
