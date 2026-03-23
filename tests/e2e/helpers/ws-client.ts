import type { Page } from "@playwright/test";

import type { WsClientPayload } from "./ws-payloads";

function nextId(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 10)}`;
}

/**
 * Send one WebSocket command and wait for the correlated ack (skips `ui` / `fs_changed` broadcasts,
 * including payloads with `generations`).
 */
export async function wsCommand(
  page: Page,
  payload: WsClientPayload,
): Promise<{ id: string; ok: boolean; location?: string; error?: string }> {
  return page.evaluate(
    async (p) => {
      return new Promise((resolve, reject) => {
        const protocol = location.protocol === "https:" ? "wss:" : "ws:";
        const ws = new WebSocket(`${protocol}//${location.host}/ws`);
        const t = window.setTimeout(() => {
          ws.close();
          reject(new Error("ws timeout"));
        }, 15_000);
        ws.onopen = () => {
          ws.send(JSON.stringify(p));
        };
        ws.onmessage = (ev) => {
          let j: {
            id?: string;
            ok?: boolean;
            location?: string;
            error?: string;
            type?: string;
          };
          try {
            j = JSON.parse(String(ev.data)) as typeof j;
          } catch {
            return;
          }
          if (j.type === "ui" || j.type === "fs_changed") {
            return;
          }
          if (j.id === p.id && (j.ok === true || j.ok === false)) {
            window.clearTimeout(t);
            ws.close();
            resolve({
              id: j.id,
              ok: j.ok === true,
              location: j.location,
              error: j.error,
            });
          }
        };
        ws.onerror = () => {
          window.clearTimeout(t);
          reject(new Error("ws error"));
        };
      });
    },
    payload,
  );
}

export async function wsCreateSkill(
  page: Page,
  name: string,
  prompt: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "create_skill",
    id,
    name,
    prompt,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsCreatePipeline(
  page: Page,
  name: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "create_pipeline",
    id,
    name,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsRenamePipeline(
  page: Page,
  oldName: string,
  newName: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "rename_pipeline",
    id,
    old_name: oldName,
    new_name: newName,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsUpdateSkill(
  page: Page,
  oldName: string,
  name: string,
  prompt: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "update_skill",
    id,
    old_name: oldName,
    name,
    prompt,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsCreateStep(
  page: Page,
  pipeline: string,
  title: string,
  prompt: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "create_step",
    id,
    pipeline,
    title,
    prompt,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsUpdateStep(
  page: Page,
  pipeline: string,
  stepId: number,
  title: string,
  prompt: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "update_step",
    id,
    pipeline,
    step_id: stepId,
    title,
    prompt,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsDeleteStep(
  page: Page,
  pipeline: string,
  stepId: number,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "delete_step",
    id,
    pipeline,
    step_id: stepId,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsReorderStep(
  page: Page,
  pipeline: string,
  stepId: number,
  targetStepId: number,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "reorder_step",
    id,
    pipeline,
    step_id: stepId,
    target_step_id: targetStepId,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsAddStepSkill(
  page: Page,
  pipeline: string,
  stepId: number,
  skillId: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "add_step_skill",
    id,
    pipeline,
    step_id: stepId,
    skill_id: skillId,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}

export async function wsDeleteStepSkill(
  page: Page,
  pipeline: string,
  stepId: number,
  skillName: string,
): Promise<{ ok: boolean; location?: string; error?: string }> {
  const id = nextId();
  const r = await wsCommand(page, {
    op: "delete_step_skill",
    id,
    pipeline,
    step_id: stepId,
    skill_name: skillName,
  });
  return { ok: r.ok, location: r.location, error: r.error };
}
