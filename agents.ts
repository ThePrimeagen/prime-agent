import "dotenv/config";
import { Agent } from "@cursor/sdk";
import { extractPrUrl } from "./pr.ts";
import { waitWithReconnect } from "./run-wait.ts";
import { filterActiveAgents } from "./sessions.ts";

function githubHttpsUrl(remote: string): string {
  const trimmed = remote.trim().replace(/\.git$/, "");
  if (trimmed.startsWith("git@github.com:")) {
    return `https://github.com/${trimmed.slice("git@github.com:".length)}`;
  }
  if (trimmed.startsWith("https://github.com/") || trimmed.startsWith("http://github.com/")) {
    return trimmed.replace(/^http:\/\//, "https://");
  }
  // .prime-agent project is often "owner/repo" — cloud send requires a full github URL
  if (/^[^/\s]+\/[^/\s]+$/.test(trimmed)) {
    return `https://github.com/${trimmed}`;
  }
  return trimmed;
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

  return wrapSdkAgent(sdkAgent);
}

export async function resume(agentId: string) {
  const sdkAgent = await Agent.resume(agentId, {
    apiKey: process.env.CURSOR_API_KEY,
  });
  return wrapSdkAgent(sdkAgent);
}

type SdkAgent = Awaited<ReturnType<typeof Agent.create>>;

function wrapSdkAgent(sdkAgent: SdkAgent) {
  return {
    agentId: sdkAgent.agentId,
    async prompt(p: string, promptOpts: { model?: string } = {}): Promise<string> {
      const run = await sdkAgent.send(
        p,
        promptOpts.model ? { model: { id: promptOpts.model } } : undefined,
      );
      const result = await waitWithReconnect(run, (runId) =>
        Agent.getRun(runId, {
          runtime: "cloud",
          agentId: sdkAgent.agentId,
          apiKey: process.env.CURSOR_API_KEY,
        }),
      );

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
