import fs from "fs";

export type ContextFile = {
  path: string;
  content: string;
};

export type GrabContextOptions = {
  stdin?: NodeJS.ReadableStream;
  stdout?: NodeJS.WritableStream;
  readFile?: (path: string) => string;
};

export type GrabbedContext = {
  extraPrompt: string;
  files: ContextFile[];
};

export type LoadContextFilesOptions = {
  readFile?: (path: string) => string;
};

export type CraftReviewPromptInput = {
  prUrl: string;
  files: ContextFile[];
  extraPrompt?: string;
};

export type CraftTakeOverPromptInput = {
  branch: string;
  files: ContextFile[];
  details?: string;
};

export type CraftKickOffPromptInput = {
  prompt: string;
  files: ContextFile[];
};

async function readLine(
  stdin: NodeJS.ReadableStream,
  stdout: NodeJS.WritableStream,
  label: string,
): Promise<string> {
  stdout.write(label);
  return await new Promise<string>((resolve, reject) => {
    let buffer = "";

    const cleanup = () => {
      stdin.off("data", onData);
      stdin.off("error", onError);
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

    stdin.on("data", onData);
    stdin.on("error", onError);
  });
}

export async function grabContext(
  opts: GrabContextOptions = {},
): Promise<GrabbedContext> {
  const stdin = opts.stdin ?? process.stdin;
  const stdout = opts.stdout ?? process.stdout;
  const readFile =
    opts.readFile ?? ((path: string) => fs.readFileSync(path, "utf8"));

  const extraPrompt = await readLine(
    stdin,
    stdout,
    "prompt to add (blank for none): ",
  );

  const files = await grabContextFiles({ stdin, stdout, readFile });

  return { extraPrompt, files };
}

export async function grabContextFiles(
  opts: GrabContextOptions = {},
): Promise<ContextFile[]> {
  const stdin = opts.stdin ?? process.stdin;
  const stdout = opts.stdout ?? process.stdout;
  const readFile =
    opts.readFile ?? ((path: string) => fs.readFileSync(path, "utf8"));

  const files: ContextFile[] = [];
  while (true) {
    const path = await readLine(
      stdin,
      stdout,
      "file to add (blank to finish): ",
    );
    if (path.trim() === "") break;
    const trimmed = path.trim();
    const content = readFile(trimmed);
    files.push({ path: trimmed, content });
  }

  return files;
}

export function loadContextFiles(
  paths: string[],
  opts: LoadContextFilesOptions = {},
): ContextFile[] {
  const readFile =
    opts.readFile ?? ((path: string) => fs.readFileSync(path, "utf8"));

  return paths.map((filePath) => ({
    path: filePath,
    content: readFile(filePath),
  }));
}

export function craftReviewPrompt(input: CraftReviewPromptInput): string {
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

export function craftTakeOverPrompt(input: CraftTakeOverPromptInput): string {
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

export function craftKickOffPrompt(input: CraftKickOffPromptInput): string {
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
