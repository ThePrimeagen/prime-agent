export type AgentListItem = {
  agentId: string;
  name: string;
  summary: string;
  lastModified: number;
  status?: "running" | "finished" | "error";
  createdAt?: number;
  archived?: boolean;
  env?: {
    type: "cloud" | "pool" | "machine";
    name?: string;
  };
  repos?: string[];
};

export function filterActiveAgents<T extends { archived?: boolean }>(
  agents: T[],
): T[] {
  return agents.filter((agent) => agent.archived !== true);
}

export function formatAgentTitle(agent: AgentListItem): string {
  const shortId = agent.agentId.slice(0, 12);
  const parts = [agent.name, `[${shortId}]`];
  if (agent.status) parts.push(`[${agent.status}]`);
  return parts.join(" ");
}
