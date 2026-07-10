const BRAILLE_FRAMES = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"] as const;
const TICK_MS = 400;

function sleep(ms: number): Promise<"tick"> {
  return new Promise((resolve) => setTimeout(() => resolve("tick"), ms));
}

export async function wait<T>(promise: Promise<T>): Promise<T> {
  let i = 0;
  // Attach early so a rejection isn't an unhandled rejection while we race ticks.
  const settled = promise.then(
    (value) => ({ ok: true as const, value }),
    (error: unknown) => ({ ok: false as const, error }),
  );

  while (true) {
    process.stdout.write(`\r${BRAILLE_FRAMES[i % BRAILLE_FRAMES.length]}`);
    i++;

    const winner = await Promise.race([settled, sleep(TICK_MS)]);
    if (winner === "tick") continue;

    process.stdout.write("\r\x1b[K");
    if (!winner.ok) throw winner.error;
    return winner.value;
  }
}
