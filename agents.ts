import "dotenv/config";
import { Agent } from "@cursor/sdk";

function githubHttpsUrl(remote: string): string {
  if (remote.startsWith("git@github.com:")) {
    return `https://github.com/${remote.slice("git@github.com:".length).replace(/\.git$/, "")}`;
  }
  return remote.replace(/\.git$/, "");
}

export async function create() {
  const cwd = process.cwd();
  const remote = (await Bun.$`git remote get-url origin`.cwd(cwd).text()).trim();
  const startingRef = (
    await Bun.$`git rev-parse --abbrev-ref HEAD`.cwd(cwd).text()
  ).trim();

  const sdkAgent = await Agent.create({
    apiKey: process.env.CURSOR_API_KEY,
    model: {
      id: "grok-4.5",
      params: [
        { id: "effort", value: "high" },
        { id: "fast", value: "true" },
      ],
    },
    cloud: {
      repos: [{ url: githubHttpsUrl(remote), startingRef }],
    },
  });

  return {
    async prompt(p: string, opts: { model?: string }): Promise<string> {
      const run = await sdkAgent.send(
        p,
        opts.model ? { model: { id: opts.model } } : undefined,
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

export async function get() {
  const { items } = await Agent.list({
    runtime: "cloud",
    limit: 10,
    apiKey: process.env.CURSOR_API_KEY,
  });

  return items.map((item) => ({
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
}
