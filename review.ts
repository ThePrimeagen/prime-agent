#!/usr/bin/env bun

import { create, get, getPr } from "./agents.ts";
import { craftReviewPrompt, grabContext } from "./context.ts";
import { parseGithubPrUrl } from "./pr.ts";
import { question } from "./question.ts";
import { formatAgentTitle } from "./sessions.ts";
import { wait } from "./wait.ts";

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

async function resolveFromCloudAgent(): Promise<{
  prUrl: string;
  repoUrl?: string;
  name: string;
}> {
  const agents = await wait(get());

  if (agents.length === 0) {
    console.log("no cloud agents found");
    process.exit(1);
  }

  const selected = await question(
    agents.map((agent) => ({
      title: formatAgentTitle(agent),
      data: agent,
    })),
  );

  const prUrl = await wait(getPr(selected.agentId));
  return {
    prUrl,
    repoUrl: selected.repos?.[0],
    name: `adversarial review: ${selected.name}`,
  };
}

async function resolveFromPrLink(): Promise<{
  prUrl: string;
  repoUrl: string;
  name: string;
}> {
  const raw = await readLine("github pr url: ");
  const parsed = parseGithubPrUrl(raw);
  return {
    prUrl: parsed.prUrl,
    repoUrl: parsed.repoUrl,
    name: `adversarial review: ${parsed.owner}/${parsed.repo}#${parsed.number}`,
  };
}

async function main() {
  const source = await question([
    { title: "cloud agent", data: "cloud" as const },
    { title: "github pr", data: "pr" as const },
  ]);

  const target =
    source === "pr" ? await resolveFromPrLink() : await resolveFromCloudAgent();

  console.log(`pr: ${target.prUrl}`);

  const { extraPrompt, files } = await grabContext();
  const prompt = craftReviewPrompt({
    prUrl: target.prUrl,
    files,
    extraPrompt,
  });

  const a = await create({
    prUrl: target.prUrl,
    repoUrl: target.repoUrl,
    name: target.name,
  });
  const output = await wait(a.prompt(prompt));
  console.log(output);
  console.log(`agent: ${a.agentId}`);
}

if (import.meta.main) {
  await main();
}
