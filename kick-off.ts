#!/usr/bin/env bun

import { create } from "./agents.ts";
import {
  craftKickOffPrompt,
  grabContextFiles,
  loadContextFiles,
} from "./context.ts";
import { loadPrimeAgentConfig } from "./prime-agent-config.ts";
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

async function main() {
  const config = loadPrimeAgentConfig();
  console.log(`project: ${config.project}`);

  const promptText = (await readLine("prompt: ")).trim();
  if (!promptText) {
    console.error("prompt is required");
    process.exit(1);
  }

  const configFiles = loadContextFiles(config.context);
  const extraFiles = await grabContextFiles();
  const files = [...configFiles, ...extraFiles];

  const prompt = craftKickOffPrompt({
    prompt: promptText,
    files,
  });

  const a = await create({
    repoUrl: config.project,
    name: `kick off: ${promptText.slice(0, 60)}`,
  });
  const output = await wait(a.prompt(prompt));
  console.log(output);
  console.log(`agent: ${a.agentId}`);
}

if (import.meta.main) {
  await main();
}
