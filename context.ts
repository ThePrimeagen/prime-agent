import fs from "fs";

export type ContextFile = {
  path: string;
  content: string;
};

export type GrabbedContext = {
  extraPrompt: string;
  files: ContextFile[];
};

async function readLine(label: string): Promise<string> {
  process.stdout.write(label);
  return await new Promise<string>((resolve, reject) => {
    let buffer = "";

    const cleanup = () => {
      process.stdin.off("data", onData);
      process.stdin.off("error", onError);
    };

    const onError = (err: unknown) => {
      cleanup();
      reject(err);
    };

    const onData = (chunk: string | Buffer) => {
      buffer += typeof chunk === "string" ? chunk : chunk.toString("utf8");
      const nl = buffer.indexOf("\n");
      if (nl === -1) return;
      const line = buffer.slice(0, nl).replace(/\r$/, "");
      cleanup();
      resolve(line);
    };

    process.stdin.on("data", onData);
    process.stdin.on("error", onError);
  });
}

export async function grabContext(): Promise<GrabbedContext> {
  const extraPrompt = await readLine("prompt to add (blank for none): ");
  const files = await grabContextFiles();
  return { extraPrompt, files };
}

export async function grabContextFiles(): Promise<ContextFile[]> {
  const files: ContextFile[] = [];
  while (true) {
    const path = await readLine("file to add (blank to finish): ");
    if (path.trim() === "") break;
    const trimmed = path.trim();
    files.push({
      path: trimmed,
      content: fs.readFileSync(trimmed, "utf8"),
    });
  }
  return files;
}

export function loadContextFiles(paths: string[]): ContextFile[] {
  return paths.map((filePath) => ({
    path: filePath,
    content: fs.readFileSync(filePath, "utf8"),
  }));
}

export function craftReviewPrompt(input: {
  prUrl: string;
  files: ContextFile[];
  extraPrompt?: string;
}): string {
  if (!input.prUrl.trim()) {
    throw new Error("craftReviewPrompt requires a prUrl");
  }

  const fileBlocks = input.files
    .map((f) => `<File path="${f.path}">\n${f.content}\n</File>`)
    .join("\n\n");

  const extra = input.extraPrompt?.trim()
    ? `\n\n<AdditionalPrompt>\n${input.extraPrompt.trim()}\n</AdditionalPrompt>`
    : "";

  return `<Context>
${fileBlocks || "(no files provided)"}
</Context>
${extra}

<YourGoal>
You are to do an adversarial review of ${input.prUrl}.  You need to thoroughly check the code for quality and correctness.  If there is an associated linear ticket, please check details on the ticket.
</YourGoal>
`;
}

export function craftTakeOverPrompt(input: {
  branch: string;
  files: ContextFile[];
  details?: string;
}): string {
  if (!input.branch.trim()) {
    throw new Error("craftTakeOverPrompt requires a branch");
  }

  const fileBlocks = input.files
    .map((f) => `<File path="${f.path}">\n${f.content}\n</File>`)
    .join("\n\n");

  const details = input.details?.trim() ?? "";

  return `<Context>
${fileBlocks || "(no files provided)"}
</Context>

<Git>
${input.branch.trim()}
</Git>
<YourTask>
Your task is to finish the work started on this branch.

<Details>
${details}
</Details>
</YourTask>
`;
}

export function craftKickOffPrompt(input: {
  prompt: string;
  files: ContextFile[];
}): string {
  if (!input.prompt.trim()) {
    throw new Error("craftKickOffPrompt requires a prompt");
  }

  const fileBlocks = input.files
    .map((f) => `<File path="${f.path}">\n${f.content}\n</File>`)
    .join("\n\n");

  return `<Context>
${fileBlocks || "(no files provided)"}
</Context>

<YourTask>
${input.prompt.trim()}
</YourTask>
`;
}
