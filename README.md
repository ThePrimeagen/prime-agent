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

## Skills Directory

- Default: `./skills`
- Override with `--skills-dir`, `PRIME_AGENT_SKILLS_DIR`, or `--config skills-dir:<path>`.

## Naming Rules

- New or updated skill and pipeline names must match `[a-z0-9-]+`.
- Use lowercase letters, digits, and dashes only (no spaces or underscores).
- Existing legacy skill names can still be referenced (`get`, `delete`), but any write/update must use the new format.
