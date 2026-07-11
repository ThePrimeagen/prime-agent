export type QuestionItem<T> = {
  title: string;
  data: T;
};

type RawStdin = NodeJS.ReadableStream & {
  setRawMode?: (mode: boolean) => void;
  isTTY?: boolean;
  isRaw?: boolean;
  resume?: () => void;
  setEncoding?: (encoding: BufferEncoding) => void;
};

function render(items: QuestionItem<unknown>[], selected: number): string {
  return items
    .map((item, i) => `${i === selected ? ">" : " "} ${item.title}`)
    .join("\n");
}

function clearLines(count: number) {
  if (count <= 0) return;
  process.stdout.write("\r");
  for (let i = 0; i < count; i++) {
    process.stdout.write("\x1b[2K");
    if (i < count - 1) process.stdout.write("\x1b[1A");
  }
  process.stdout.write("\r");
}

export async function question<T>(items: QuestionItem<T>[]): Promise<T> {
  if (items.length === 0) {
    throw new Error("question requires at least one item");
  }

  const stdin = process.stdin as RawStdin;
  let selected = 0;
  let lineCount = 0;

  const draw = () => {
    clearLines(lineCount);
    process.stdout.write(render(items, selected));
    lineCount = items.length;
  };

  const wasRaw = stdin.isRaw === true;
  const canRaw = typeof stdin.setRawMode === "function" && stdin.isTTY;

  if (canRaw) stdin.setRawMode!(true);
  if (typeof stdin.resume === "function") stdin.resume();
  if (typeof stdin.setEncoding === "function") stdin.setEncoding("utf8");

  draw();

  return await new Promise<T>((resolve, reject) => {
    let buffer = "";

    const cleanup = () => {
      stdin.off("data", onData);
      stdin.off("error", onError);
      if (canRaw) stdin.setRawMode!(wasRaw);
      clearLines(lineCount);
      lineCount = 0;
    };

    const finish = (value: T) => {
      cleanup();
      resolve(value);
    };

    const fail = (err: unknown) => {
      cleanup();
      reject(err);
    };

    const onError = (err: unknown) => fail(err);

    const onData = (chunk: string | Buffer) => {
      buffer += typeof chunk === "string" ? chunk : chunk.toString("utf8");

      while (buffer.length > 0) {
        if (buffer.startsWith("\x1b[A") || buffer.startsWith("\x1bOA")) {
          buffer = buffer.slice(3);
          selected = Math.max(0, selected - 1);
          draw();
          continue;
        }
        if (buffer.startsWith("\x1b[B") || buffer.startsWith("\x1bOB")) {
          buffer = buffer.slice(3);
          selected = Math.min(items.length - 1, selected + 1);
          draw();
          continue;
        }
        if (buffer.startsWith("\r") || buffer.startsWith("\n")) {
          buffer = buffer.slice(1);
          finish(items[selected]!.data);
          return;
        }
        if (buffer.startsWith("\x03")) {
          fail(new Error("question cancelled"));
          return;
        }
        if (buffer.startsWith("\x1b") && buffer.length < 3) return;
        buffer = buffer.slice(1);
      }
    };

    stdin.on("data", onData);
    stdin.on("error", onError);
  });
}
