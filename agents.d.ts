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

export function create(): Promise<CursorAgent>;
export function get(): Promise<CursorCloudAgent[]>;
export type PromptOptions = {
  model?: string;
};

export interface CursorAgent {
  // carries on a conversation with the same agent in the same chat
  prompt(p: string, opts: PromptOptions): Promise<string>;
}
