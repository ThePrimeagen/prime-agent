/**
 * Client → server WebSocket JSON bodies (`op` tag), aligned with `ClientOp` in Rust.
 */

export type WsClientPayload =
  | { op: "create_skill"; id: string; name: string; prompt: string }
  | { op: "update_skill"; id: string; old_name: string; name: string; prompt: string }
  | { op: "delete_skill"; id: string; name: string }
  | { op: "create_pipeline"; id: string; name: string }
  | { op: "rename_pipeline"; id: string; old_name: string; new_name: string }
  | { op: "create_step"; id: string; pipeline: string; title: string; prompt: string }
  | { op: "update_step"; id: string; pipeline: string; step_id: number; title: string; prompt: string }
  | { op: "delete_step"; id: string; pipeline: string; step_id: number }
  | {
      op: "reorder_step";
      id: string;
      pipeline: string;
      step_id: number;
      /** Invalid payloads may use a non-numeric string (see unhappy-path e2e). */
      target_step_id: number | string;
    }
  | { op: "add_step_skill"; id: string; pipeline: string; step_id: number; skill_id: string }
  | { op: "delete_step_skill"; id: string; pipeline: string; step_id: number; skill_name: string };
