# 100% vibe coded

i have not looked at ANY code.  I have not looked at any tests, i have no idea what the hell is happening here.  Just accept AI Love into your heart and die from the greatest increase of productivity ever

# prime-agent

Skill-driven AGENTS.md builder and synchronizer for managing reusable
instructions stored as Markdown files.

## Commands

- `prime-agent get <skill1,skill2,...>`: Build `AGENTS.md` from skills.
- `prime-agent set <name> <path>`: Store a skill file as `skills/<name>/SKILL.md`.
- `prime-agent list`: List available skills (blank line between entries).
- `prime-agent list <fragment>`: List matching skills on one line for `get`.
- `prime-agent local`: List local skills with out-of-sync status.
- `prime-agent sync`: Two-way sync between `AGENTS.md` and skills, with local git commit.
- `prime-agent sync-remote`: Sync, commit, then `git pull --rebase` in skills repo.
- `prime-agent delete <name>`: Remove a skill section from `AGENTS.md`.
- `prime-agent delete-globally <name>`: Remove section and skill file.
- `prime-agent config`: Print required and optional config values.
- `prime-agent config get <name>`: Print a config value.
- `prime-agent config set <name> <value>`: Set a config value and print all values.
- `prime-agent pipelines run <name> --prompt <text>` or `--file <path>`: Run pipeline stages (see `.prime-agent/config.json`).

## `.prime-agent/config.json` (pipelines)

| Key | Meaning |
| --- | --- |
| `model` | Model string passed to `cursor-agent --model`. |
| `clirunner` | CLI to invoke; must be `cursor-agent` (legacy key `cli` is accepted as an alias). |
| `stdout_lines` | Reserved for future CLI display options (parsed; default `3`; must be at least `1`). |

`--no-tui` and `PRIME_AGENT_NO_TUI=1` are accepted for compatibility and have no effect (pipelines run uses plain stdout progress only).

## Skills Directory

- Default: `<data-dir>/skills` where `<data-dir>` is `--data-dir` if passed, otherwise the current working directory (so typically `./skills`).
- Override only with `--skills-dir` or `--config skills-dir:<path>` (no environment-variable fallbacks).

## Naming Rules

- New or updated skill and pipeline names must match `[a-z0-9-]+`.
- Use lowercase letters, digits, and dashes only (no spaces or underscores).
- Existing legacy skill names can still be referenced (`get`, `delete`), but any write/update must use the new format.

## Website tests (Playwright)

Run the full UI suite in two passes from the repository root:

1. `npm run test:e2e` — uses `playwright.config.ts` (live reload disabled via `PRIME_AGENT_DISABLE_LIVE_RELOAD=1`; excludes `tests/e2e/live/`).
2. `npm run test:e2e:live` — uses `playwright.live.config.ts` (live reload enabled; only `tests/e2e/live/`).

Both must pass before considering website changes complete.

## CLI `sync` / git (manual edge cases)

Automated tests cover the happy path for `sync`, `sync-remote`, and `commit` in a git repo. Failures in `git pull --rebase`, `git add`, `git commit`, or `git status` depend on hooks, permissions, and remotes; verify those manually in a repo where you can force each failure (for example, a failing pre-commit hook or a remote that rejects the push).
