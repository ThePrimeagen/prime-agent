# Agent Guidance

## Build, Lint, Test Discipline

- Always run `cargo clippy --all-targets --all-features -- -D warnings`, fix any issues.
- Always run `cargo build`, fix any issues.
- Always run `cargo test`, fix any issues.
- For each step, check `AGENTS.md` and `README.md` in the target repo for any specified test, lint, or formatting requirements and follow them.

## Versioning

- Bump the patch version on every change and print it on every run of `prime-agent`.
- Do not write version files to target project directories at runtime.

# Website Guidance
* always run all tests before considering the feature complete.
* there can never be a failing test.  If a test fails, its a bug.
  - you must fix the bug in the code, not by altering the test.
  - if you are unable to fix the bug, you must stop the process and alert me with the error and WHY we MUST alter the test.
* e2e tests only.  Do not write unit tests.

## website frontend
* its fine to have javascript on the front
* the website is split into two parts, left nav and main content
  - left nav is 20% of the screen
  - main content is 80% of the screen
