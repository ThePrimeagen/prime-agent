export type CursorCloudAgent = {
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

export type CreateOptions = {
  prUrl?: string;
  name?: string;
  repoUrl?: string;
  startingRef?: string;
};

export function create(opts?: CreateOptions): Promise<CursorAgent>;
export function resume(agentId: string): Promise<CursorAgent>;
export function get(): Promise<CursorCloudAgent[]>;
export function getPr(agentId: string): Promise<string>;
export type PromptOptions = {
  model?: string;
};

export interface CursorAgent {
  agentId: string;
  // carries on a conversation with the same agent in the same chat
  prompt(p: string, opts?: PromptOptions): Promise<string>;
}
