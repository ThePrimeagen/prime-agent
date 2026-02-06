# Agent Guidance

## Build, Lint, Test Discipline

- Always run `cargo clippy --all-targets --all-features -- -D warnings`, fix any issues.
- Always run `cargo build`, fix any issues.
- Always run `cargo test`, fix any issues.
- For each step, check `AGENTS.md` and `README.md` in the target repo for any specified test, lint, or formatting requirements and follow them.

## Versioning

- Bump the patch version on every change and print it on every run of `prime-agent`.
- Do not write version files to target project directories at runtime.
