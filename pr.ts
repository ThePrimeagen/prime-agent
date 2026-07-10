export type RunGitBranch = {
  repoUrl?: string;
  branch?: string;
  prUrl?: string;
};

export type RunLike = {
  git?: {
    branches?: RunGitBranch[];
  };
};

export type ParsedGithubPr = {
  prUrl: string;
  repoUrl: string;
  owner: string;
  repo: string;
  number: number;
};

const GITHUB_PR_RE =
  /^https:\/\/github\.com\/([^/]+)\/([^/]+)\/pull\/(\d+)(?:\/[^?#]*)?(?:[?#].*)?$/;

export function parseGithubPrUrl(input: string): ParsedGithubPr {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("github pr url is required");
  }

  const match = trimmed.match(GITHUB_PR_RE);
  if (!match) {
    throw new Error(`invalid github pr url: ${trimmed}`);
  }

  const owner = match[1]!;
  const repo = match[2]!;
  const number = Number(match[3]);
  const prUrl = `https://github.com/${owner}/${repo}/pull/${number}`;
  const repoUrl = `https://github.com/${owner}/${repo}`;

  return { prUrl, repoUrl, owner, repo, number };
}

export function extractPrUrl(runs: RunLike[]): string {
  for (const run of runs) {
    for (const branch of run.git?.branches ?? []) {
      if (branch.prUrl?.trim()) return branch.prUrl.trim();
    }
  }
  throw new Error("no github pr found on agent runs");
}
