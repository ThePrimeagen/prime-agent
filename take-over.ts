#!/usr/bin/env bun

import { create } from "./agents.ts";
import { craftTakeOverPrompt, grabContext } from "./context.ts";
import { wait } from "./wait.ts";

async function main() {
  const cwd = process.cwd();
  const branch = (
    await Bun.$`git rev-parse --abbrev-ref HEAD`.cwd(cwd).text()
  ).trim();

  if (!branch) {
    console.error("could not determine current git branch");
    process.exit(1);
  }

  console.log(`branch: ${branch}`);

  const { extraPrompt, files } = await grabContext();
  const prompt = craftTakeOverPrompt({
    branch,
    files,
    details: extraPrompt,
  });

  const a = await create({
    startingRef: branch,
    name: `take over: ${branch}`,
  });
  const output = await wait(a.prompt(prompt));
  console.log(output);
  console.log(`agent: ${a.agentId}`);
}

if (import.meta.main) {
  await main();
}
