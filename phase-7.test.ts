import { test, expect } from "bun:test";
import {
  assertResumeMode,
  parsePhase7Args,
  parsePhaseNumber,
  pipelineStepsFrom,
} from "./phase-7.ts";

test("parsePhase7Args detects --resume", () => {
  expect(parsePhase7Args(["--resume"])).toEqual({ resume: true });
  expect(parsePhase7Args(["node", "phase-7.ts", "--resume"])).toEqual({
    resume: true,
  });
});

test("parsePhase7Args ignores absence of --resume", () => {
  expect(parsePhase7Args([])).toEqual({ resume: false });
  expect(parsePhase7Args(["node", "phase-7.ts"])).toEqual({ resume: false });
});

test("parsePhase7Args rejects unknown flags", () => {
  expect(() => parsePhase7Args(["--foo"])).toThrow();
  expect(() => parsePhase7Args(["--resume", "--foo"])).toThrow();
});

test("parsePhaseNumber accepts 1-5 and 7", () => {
  expect(parsePhaseNumber("1")).toBe(1);
  expect(parsePhaseNumber("2")).toBe(2);
  expect(parsePhaseNumber("3")).toBe(3);
  expect(parsePhaseNumber("4")).toBe(4);
  expect(parsePhaseNumber("5")).toBe(5);
  expect(parsePhaseNumber("7")).toBe(7);
  expect(parsePhaseNumber(" 3 ")).toBe(3);
});

test("parsePhaseNumber rejects empty, non-numeric, 6, and out-of-range", () => {
  expect(() => parsePhaseNumber("")).toThrow();
  expect(() => parsePhaseNumber("   ")).toThrow();
  expect(() => parsePhaseNumber("abc")).toThrow();
  expect(() => parsePhaseNumber("6")).toThrow();
  expect(() => parsePhaseNumber("0")).toThrow();
  expect(() => parsePhaseNumber("8")).toThrow();
  expect(() => parsePhaseNumber("1.5")).toThrow();
});

test("assertResumeMode allows review for 2/3/4/7", () => {
  for (const phase of [2, 3, 4, 7] as const) {
    expect(() => assertResumeMode(phase, "review")).not.toThrow();
  }
});

test("assertResumeMode rejects review for 1/5", () => {
  expect(() => assertResumeMode(1, "review")).toThrow();
  expect(() => assertResumeMode(5, "review")).toThrow();
});

test("assertResumeMode always allows phase", () => {
  for (const phase of [1, 2, 3, 4, 5, 7] as const) {
    expect(() => assertResumeMode(phase, "phase")).not.toThrow();
  }
});

test("pipelineStepsFrom fresh start from phase 1", () => {
  expect(pipelineStepsFrom(1, "phase")).toEqual([
    { kind: "phase", phase: 1 },
    { kind: "phase", phase: 2 },
    { kind: "review", phase: 2 },
    { kind: "phase", phase: 3 },
    { kind: "review", phase: 3 },
    { kind: "phase", phase: 4 },
    { kind: "review", phase: 4 },
    { kind: "phase", phase: 5 },
    { kind: "phase5-followup", phase: 5 },
    { kind: "phase", phase: 7 },
    { kind: "review", phase: 7 },
  ]);
});

test("pipelineStepsFrom mid-phase starts at that phase", () => {
  expect(pipelineStepsFrom(3, "phase")).toEqual([
    { kind: "phase", phase: 3 },
    { kind: "review", phase: 3 },
    { kind: "phase", phase: 4 },
    { kind: "review", phase: 4 },
    { kind: "phase", phase: 5 },
    { kind: "phase5-followup", phase: 5 },
    { kind: "phase", phase: 7 },
    { kind: "review", phase: 7 },
  ]);
});

test("pipelineStepsFrom mid-review skips phase work", () => {
  expect(pipelineStepsFrom(3, "review")).toEqual([
    { kind: "review", phase: 3 },
    { kind: "phase", phase: 4 },
    { kind: "review", phase: 4 },
    { kind: "phase", phase: 5 },
    { kind: "phase5-followup", phase: 5 },
    { kind: "phase", phase: 7 },
    { kind: "review", phase: 7 },
  ]);
});

test("pipelineStepsFrom includes phase5 follow-up after phase 5", () => {
  expect(pipelineStepsFrom(5, "phase")).toEqual([
    { kind: "phase", phase: 5 },
    { kind: "phase5-followup", phase: 5 },
    { kind: "phase", phase: 7 },
    { kind: "review", phase: 7 },
  ]);
});

test("pipelineStepsFrom end at phase 7 review", () => {
  expect(pipelineStepsFrom(7, "review")).toEqual([
    { kind: "review", phase: 7 },
  ]);
});
