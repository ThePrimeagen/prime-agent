import "dotenv/config";
import { Agent } from "@cursor/sdk";
import { extractPrUrl } from "./pr.ts";
import { filterActiveAgents } from "./sessions.ts";

function githubHttpsUrl(remote: string): string {
  if (remote.startsWith("git@github.com:")) {
    return `https://github.com/${remote.slice("git@github.com:".length).replace(/\.git$/, "")}`;
  }
  return remote.replace(/\.git$/, "");
}

export type CreateOptions = {
  prUrl?: string;
  name?: string;
  /** Override repo URL (e.g. from a selected cloud session). Defaults to local origin. */
  repoUrl?: string;
  startingRef?: string;
};

export async function create(opts: CreateOptions = {}) {
  const cwd = process.cwd();
  const remote =
    opts.repoUrl ??
    (await Bun.$`git remote get-url origin`.cwd(cwd).text()).trim();
  const startingRef =
    opts.startingRef ??
    (await Bun.$`git rev-parse --abbrev-ref HEAD`.cwd(cwd).text()).trim();

  const repo: { url: string; startingRef?: string; prUrl?: string } = {
    url: githubHttpsUrl(remote),
  };
  // When reviewing a PR, prefer the PR as the checkout source.
  if (opts.prUrl) repo.prUrl = opts.prUrl;
  else repo.startingRef = startingRef;

  const sdkAgent = await Agent.create({
    apiKey: process.env.CURSOR_API_KEY,
    name: opts.name,
    model: {
      id: "grok-4.5",
      params: [
        { id: "effort", value: "high" },
        { id: "fast", value: "true" },
      ],
    },
    cloud: {
      repos: [repo],
    },
  });

  return {
    agentId: sdkAgent.agentId,
    async prompt(p: string, promptOpts: { model?: string } = {}): Promise<string> {
      const run = await sdkAgent.send(
        p,
        promptOpts.model ? { model: { id: promptOpts.model } } : undefined,
      );
      const result = await run.wait();

      if (result.status === "error") {
        throw new Error(result.error?.message ?? "agent run failed");
      }

      if (result.status === "cancelled") {
        throw new Error("agent run was cancelled");
      }

      return result.result ?? "";
    },
  };
}

export async function getPr(agentId: string): Promise<string> {
  const { items } = await Agent.listRuns(agentId, {
    runtime: "cloud",
    limit: 20,
    apiKey: process.env.CURSOR_API_KEY,
  });

  return extractPrUrl(items);
}

export async function get() {
  const { items } = await Agent.list({
    runtime: "cloud",
    limit: 10,
    includeArchived: false,
    apiKey: process.env.CURSOR_API_KEY,
  });

  const mapped = items.map((item) => ({
    agentId: item.agentId,
    name: item.name,
    summary: item.summary,
    lastModified: item.lastModified,
    status: item.status,
    createdAt: item.createdAt,
    archived: item.archived,
    env: item.runtime === "cloud" ? item.env : undefined,
    repos: item.runtime === "cloud" ? item.repos : undefined,
  }));

  // Belt-and-suspenders: never surface archived sessions in the picker.
  return filterActiveAgents(mapped);
}
