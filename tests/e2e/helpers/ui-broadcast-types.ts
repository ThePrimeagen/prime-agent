/** Mirrors `crate::web::render` / `UiBroadcast` JSON from the server. */

export type GenerationSnapshotJson = {
  skills?: Record<string, number>;
  pipelines?: Record<string, number>;
  list_epoch?: number;
};

export type StepSkillVmJson = {
  id: string;
  name: string;
  name_encoded: string;
};

export type SkillVmJson = {
  id: string;
  name: string;
  name_encoded: string;
  prompt: string;
};

export type PipelineVmJson = {
  name: string;
  name_encoded: string;
  broken?: boolean;
};

export type PipelineStepVmJson = {
  id: number;
  title: string;
  prompt: string;
  skill_count: number;
  skills: StepSkillVmJson[];
  skill_summary: string;
};

/** Minimal parse of a client `send` payload for test instrumentation. */
export type WsClientOpField = {
  op?: string;
};

/**
 * Parsed WebSocket JSON frames the e2e tests observe (subset of server messages).
 */
export type WsMessagePayload = {
  type?: string;
  generations?: GenerationSnapshotJson;
  skills?: SkillVmJson[];
  pipelines?: PipelineVmJson[];
  pipeline_steps?: PipelineStepVmJson[];
  selected_skill?: SkillVmJson;
  selected_pipeline?: PipelineVmJson;
  live_reload?: boolean;
};
