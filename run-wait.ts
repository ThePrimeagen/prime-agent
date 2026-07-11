import type { RunResult } from "@cursor/sdk";

export type WaitableRun = {
  id: string;
  wait(): Promise<RunResult>;
};

const STREAM_UNAVAILABLE_RE = /stream is no longer available/i;

export function isStreamUnavailable(message: string | undefined): boolean {
  if (!message) return false;
  return STREAM_UNAVAILABLE_RE.test(message);
}

function errorMessage(err: unknown): string | undefined {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return undefined;
}

/**
 * Wait for a cloud run, transparently reconnecting when the live stream dies
 * while the run may still be in progress on the server.
 */
export async function waitWithReconnect(
  run: WaitableRun,
  reconnect: (runId: string) => Promise<WaitableRun>,
): Promise<RunResult> {
  let current = run;

  for (;;) {
    let result: RunResult;
    try {
      result = await current.wait();
    } catch (err) {
      if (!isStreamUnavailable(errorMessage(err))) throw err;
      current = await reconnect(current.id);
      continue;
    }

    if (
      result.status === "error" &&
      isStreamUnavailable(result.error?.message)
    ) {
      current = await reconnect(current.id);
      continue;
    }

    return result;
  }
}
