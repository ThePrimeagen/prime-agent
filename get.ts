#!/usr/bin/env bun

import { get } from "./agents.ts";
import { question } from "./question.ts";
import { formatAgentTitle } from "./sessions.ts";
import { wait } from "./wait.ts";

async function main() {
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

  console.log(selected);
}

if (import.meta.main) {
  await main();
}
