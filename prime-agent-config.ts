import fs from "fs";
import path from "path";

export type PrimeAgentConfig = {
  project: string;
  context: string[];
};

export function parsePrimeAgentConfig(raw: string): PrimeAgentConfig {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error("invalid .prime-agent JSON");
  }

  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("invalid .prime-agent: expected an object");
  }

  const obj = parsed as Record<string, unknown>;

  if (typeof obj.project !== "string" || obj.project.trim() === "") {
    throw new Error("invalid .prime-agent: project must be a non-empty string");
  }

  if (!Array.isArray(obj.context)) {
    throw new Error("invalid .prime-agent: context must be an array of strings");
  }

  if (!obj.context.every((entry) => typeof entry === "string")) {
    throw new Error("invalid .prime-agent: context must be an array of strings");
  }

  return {
    project: obj.project.trim(),
    context: obj.context,
  };
}

export function loadPrimeAgentConfig(): PrimeAgentConfig {
  const configPath = path.join(process.cwd(), ".prime-agent");

  let raw: string;
  try {
    raw = fs.readFileSync(configPath, "utf8");
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    throw new Error(`missing .prime-agent: ${message}`);
  }

  return parsePrimeAgentConfig(raw);
}
