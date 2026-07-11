#!/usr/bin/env bun

import fs from "fs";
import path from "path";
import { create, get, getPr, resume } from "./agents.ts";
import {
  craftReviewPrompt,
  grabContextFiles,
  type ContextFile,
} from "./context.ts";
import { loadPrimeAgentConfig } from "./prime-agent-config.ts";
import { question } from "./question.ts";
import { formatAgentTitle } from "./sessions.ts";
import { wait } from "./wait.ts";

const PHASES_DIR = path.join(import.meta.dir, "phases");
const REVIEW_PHASES = new Set([2, 3, 4, 7]);
const VALID_PHASES = new Set([1, 2, 3, 4, 5, 7]);

export type ResumeMode = "phase" | "review";
export type PhaseNumber = 1 | 2 | 3 | 4 | 5 | 7;

export type PipelineStep =
  | { kind: "phase"; phase: PhaseNumber }
  | { kind: "review"; phase: PhaseNumber }
  | { kind: "phase5-followup"; phase: 5 };

type Steve = Awaited<ReturnType<typeof create>>;

export function parsePhase7Args(argv: string[]): { resume: boolean } {
  let resumeFlag = false;
  for (const arg of argv) {
    if (arg === "--resume") {
      resumeFlag = true;
      continue;
    }
    if (arg.startsWith("-")) {
      throw new Error(`unknown flag: ${arg}`);
    }
  }
  return { resume: resumeFlag };
}

export function parsePhaseNumber(raw: string): PhaseNumber {
  const trimmed = raw.trim();
  if (!/^\d+$/.test(trimmed)) {
    throw new Error(`invalid phase: ${raw}`);
  }
  const n = Number(trimmed);
  if (!VALID_PHASES.has(n)) {
    throw new Error(`invalid phase: ${raw}`);
  }
  return n as PhaseNumber;
}

export function assertResumeMode(phase: PhaseNumber, mode: ResumeMode): void {
  if (mode === "review" && !REVIEW_PHASES.has(phase)) {
    throw new Error(`phase ${phase} has no review step`);
  }
}

export function pipelineStepsFrom(
  phase: PhaseNumber,
  mode: ResumeMode,
): PipelineStep[] {
  const order: PhaseNumber[] = [1, 2, 3, 4, 5, 7];
  const startIdx = order.indexOf(phase);
  const steps: PipelineStep[] = [];

  for (let i = startIdx; i < order.length; i++) {
    const p = order[i]!;
    const includePhaseWork = !(i === startIdx && mode === "review");

    if (includePhaseWork) {
      steps.push({ kind: "phase", phase: p });
      if (p === 5) {
        steps.push({ kind: "phase5-followup", phase: 5 });
      }
    }

    if (REVIEW_PHASES.has(p)) {
      if (i === startIdx && mode === "review") {
        steps.push({ kind: "review", phase: p });
      } else if (includePhaseWork) {
        steps.push({ kind: "review", phase: p });
      }
    }
  }

  return steps;
}

async function readLine(label: string): Promise<string> {
  process.stdout.write(label);
  return await new Promise<string>((resolve, reject) => {
    let buffer = "";

    const cleanup = () => {
      process.stdin.off("data", onData);
      process.stdin.off("error", onError);
    };

    const onError = (err: unknown) => {
      cleanup();
      reject(err);
    };

    const onData = (chunk: string | Buffer) => {
      buffer += typeof chunk === "string" ? chunk : chunk.toString("utf8");
      const nl = buffer.indexOf("\n");
      if (nl === -1) return;
      const line = buffer.slice(0, nl).replace(/\r$/, "");
      cleanup();
      resolve(line);
    };

    process.stdin.on("data", onData);
    process.stdin.on("error", onError);
  });
}

function loadPhase(phase: number): ContextFile {
  const filePath = path.join(
    PHASES_DIR,
    `${String(phase).padStart(3, "0")}.md`,
  );
  return {
    path: filePath,
    content: fs.readFileSync(filePath, "utf8"),
  };
}

function fileBlocks(files: ContextFile[]): string {
  if (files.length === 0) return "(no files provided)";
  return files
    .map((f) => `<File path="${f.path}">\n${f.content}\n</File>`)
    .join("\n\n");
}

function taskPrompt(task: string, files: ContextFile[]): string {
  return `<Context>
${fileBlocks(files)}
</Context>

<YourTask>
${task}
</YourTask>
`;
}

function phaseTask(phase: PhaseNumber, startingPrompt?: string): string {
  if (phase === 1) {
    return startingPrompt
      ? `start 7 phase planning\n\n${startingPrompt}`
      : "start 7 phase planning";
  }
  if (phase === 7) return "skip phase 6, start phase 7";
  return `start phase ${phase}`;
}

async function reviewAndFix(
  steve: Steve,
  phase: number,
  phaseFile: ContextFile,
  reviewContextFiles: ContextFile[],
  repoUrl: string,
) {
  if (!REVIEW_PHASES.has(phase)) return;

  let prUrl: string;
  try {
    prUrl = await wait(getPr(steve.agentId));
  } catch {
    console.log(`phase ${phase}: no github pr, skipping review`);
    return;
  }

  console.log(`phase ${phase}: reviewing ${prUrl}`);

  const reviewer = await create({
    name: `adversarial review: phase ${phase}`,
    prUrl,
    repoUrl,
  });

  const reviewOutput = await wait(
    reviewer.prompt(
      craftReviewPrompt({
        prUrl,
        files: [phaseFile, ...reviewContextFiles],
      }),
    ),
  );

  console.log(`phase ${phase}: asking Steve to fix PR feedback`);
  await wait(
    steve.prompt(`<YourTask>
Fix the PR feedback from the phase ${phase} review.

<ReviewFeedback>
${reviewOutput.trim()}
</ReviewFeedback>
</YourTask>
`),
  );
}

async function runPipeline(
  steve: Steve,
  steps: PipelineStep[],
  phaseContextFiles: ContextFile[],
  reviewContextFiles: ContextFile[],
  repoUrl: string,
  startingPrompt?: string,
) {
  for (const step of steps) {
    if (step.kind === "phase") {
      const phaseFile = loadPhase(step.phase);
      if (step.phase === 1) {
        console.log("phase 1: start 7 phase planning");
      } else if (step.phase === 5) {
        console.log("phase 5: start (no review)");
      } else if (step.phase === 7) {
        console.log("skip phase 6, start phase 7");
      } else {
        console.log(`phase ${step.phase}: start`);
      }
      await wait(
        steve.prompt(
          taskPrompt(phaseTask(step.phase, startingPrompt), [
            phaseFile,
            ...phaseContextFiles,
          ]),
        ),
      );
      continue;
    }

    if (step.kind === "phase5-followup") {
      console.log("phase 5 follow-up: phase 2-4 changes from feedback");
      await wait(
        steve.prompt(
          taskPrompt(
            "make any additional phase 2 through 4 changes needed after phase 5 feedback.",
            phaseContextFiles,
          ),
        ),
      );
      continue;
    }

    const phaseFile = loadPhase(step.phase);
    await reviewAndFix(
      steve,
      step.phase,
      phaseFile,
      reviewContextFiles,
      repoUrl,
    );
  }
}

async function main() {
  const args = parsePhase7Args(process.argv.slice(2));
  const config = loadPrimeAgentConfig();
  console.log(`project: ${config.project}`);

  let steve: Steve;
  let steps: PipelineStep[];
  let startingPrompt: string | undefined;

  if (args.resume) {
    const agents = await wait(get());
    if (agents.length === 0) {
      console.error("no cloud agents found");
      process.exit(1);
    }

    const selected = await question(
      agents.map((agent) => ({
        title: formatAgentTitle(agent),
        data: agent,
      })),
    );

    let phase: PhaseNumber;
    try {
      phase = parsePhaseNumber(await readLine("phase: "));
    } catch (err) {
      console.error(err instanceof Error ? err.message : String(err));
      process.exit(1);
    }

    const mode = await question([
      { title: "phase", data: "phase" as const },
      { title: "review", data: "review" as const },
    ]);

    try {
      assertResumeMode(phase, mode);
    } catch (err) {
      console.error(err instanceof Error ? err.message : String(err));
      process.exit(1);
    }

    steve = await resume(selected.agentId);
    console.log(`Steve: ${steve.agentId}`);
    steps = pipelineStepsFrom(phase, mode);
  } else {
    startingPrompt = (await readLine("starting prompt: ")).trim();
    if (!startingPrompt) {
      console.error("starting prompt is required");
      process.exit(1);
    }

    steve = await create({
      name: `Steve: ${startingPrompt.slice(0, 60)}`,
      repoUrl: config.project,
    });
    console.log(`Steve: ${steve.agentId}`);
    steps = pipelineStepsFrom(1, "phase");
  }

  console.log("phase context files:");
  const phaseContextFiles = await grabContextFiles();

  console.log("review context files:");
  const reviewContextFiles = await grabContextFiles();

  await runPipeline(
    steve,
    steps,
    phaseContextFiles,
    reviewContextFiles,
    config.project,
    startingPrompt,
  );

  console.log(`done. Steve: ${steve.agentId}`);
}

if (import.meta.main) {
  await main();
}
