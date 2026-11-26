@AGENTS.md

## Claude Code specifics

- Project skills live in `.claude/skills/`:
  - `/next-issue`: run one full issue cycle (pick, implement, PR, merge, record). Pass a number to work up to that many issues in sequence.
  - `/plan-milestone <milestone>`: rolling-wave breakdown of a milestone epic into agent-ready issues.
  - `/write-rule`: write or amend a `docs/rules/` rule with its sources, then implement it.
  - `/platform-test`: hands-on test of a build on Linux or Windows: install, the guide's commands, and a sweep of the whole app.
- Before merging a PR sized `size:M` or larger, run `/code-review` on the branch and address real findings.
- Use the `playwright-cli` skill for UI verification once the web UI exists (M7).
