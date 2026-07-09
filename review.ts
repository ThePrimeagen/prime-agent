#!/usr/bin/env bun

import { create } from "./agents.ts";
import { wait } from "./wait.ts";
import fs from "fs";

async function main() {
  if (process.argv.length < 3) {
    console.log("usage: bun run review.ts path/to/prompt");
    process.exit(1);
  }

  const f = process.argv[2]!;
  const prompt = fs.readFileSync(f).toString();

  const a = await create();
  const output = await wait(a.prompt(prompt, {}));
  console.log(output);
}

if (import.meta.main) {
  await main();
}
