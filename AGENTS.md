# TotalFloat agent operating manual

This file is the standing brief for any AI agent (or human) working in this repository. Read it fully at the start of every session. It is short on purpose; follow the links when a task needs more.

TotalFloat is an open-source, agent-native CPM scheduler. It reads and writes Primavera P6 XER files and Microsoft Project XML files, and calculates schedules with its own deterministic engine and documented rules ([docs/rules/](docs/rules/README.md)). Rust core, CLI and MCP interfaces first, UI and team server later.

## Read first

1. The open GitHub milestones and issues: where the project is and what's next.
2. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): crates, data model, interfaces, dependency rules, and the [design decisions](docs/ARCHITECTURE.md#design-decisions). Don't relitigate them in code; propose a change in a `needs-design` issue ([CONTRIBUTING.md](CONTRIBUTING.md#proposing-a-design-change)). CONTRIBUTING.md also holds the clean-room rules and where calculation rules may get their sources.
3. [docs/GLOSSARY.md](docs/GLOSSARY.md) and [docs/rules/](docs/rules/): domain terms, and TotalFloat's calculation rules with their sources.

## Non-negotiables

- **Never commit corpus files** or any real-world XER/MPP/XML schedule. The corpus is public files [`corpus/manifest.toml`](corpus/README.md) links to; `cargo xtask corpus fetch` downloads them into `$TOTALFLOAT_CORPUS`. Only synthetic or explicitly permissioned files go in `fixtures/`, each listed in its `MANIFEST.md`. `.gitignore` blocks `*.xer` outside fixture directories; never force-add around it.
- **Clean room and rule sources.** Follow [CONTRIBUTING.md, Clean room](CONTRIBUTING.md#clean-room). Every calculation rule cites its sources ([CONTRIBUTING.md](CONTRIBUTING.md#calculation-rules-and-their-sources)).
- **Never weaken, skip, or delete a test to get green.** If a test is wrong, fix it and say why in the PR.
- **Language models never compute schedule dates.** All dates, floats and paths come from the deterministic engine.
- **The core makes no network calls** and has no telemetry.
- **Output is deterministic:** no `HashMap` iteration order in outputs, no wall-clock time in results unless it is an explicit input.
- `#![forbid(unsafe_code)]` in every crate. Dependencies must be permissively licensed (`cargo deny check` enforces this once CI exists).

## Session protocol

Each session works on **one issue**, end to end, unless the human says otherwise. The `/next-issue` skill (`.claude/skills/next-issue/SKILL.md`) is the detailed version of this protocol.

1. **Orient.** Run `git status`, `git log --oneline -15`, and `gh issue list --label agent-ready --state open --limit 30`.
2. **Verify the baseline.** `git switch main && git pull --ff-only`, then `just check`. If main is broken, fixing it is the task.
3. **Pick work.** Choose the open issue labeled `agent-ready` in the lowest-numbered milestone, highest priority (`P0` > `P1` > `P2`), then lowest issue number. Skip anything labeled `blocked`, `needs-human`, or `needs-design`, or whose "Blocked by" issues are still open. Comment on the issue that you are starting.
4. **Branch.** `git switch -c <issue-number>-<short-slug>`.
5. **Implement** to the issue's acceptance criteria, with tests. Keep the change to the issue's scope; file follow-up issues for anything else you find.
6. **Verify.** `just check` must pass. With `$TOTALFLOAT_CORPUS` set, the corpus tests run too.
7. **Record.** Comment on the issue with what changed and anything the next session should know, and file follow-up issues. Write or amend the `docs/rules/` rule, with its sources, for any calculation rule you implemented or changed (the `write-rule` skill).
8. **PR.** Push and open a PR whose body follows the template and says `Closes #<n>`. Wait for CI. Fix failures.
9. **Merge.** See the merge policy below. After merging: `git switch main && git pull --ff-only`, delete the local branch.

## Merge policy

- You may squash-merge your own PR once CI is green and you have reviewed the full diff yourself, **unless** the PR changes any of: a design decision in `docs/ARCHITECTURE.md`, the rules in `CONTRIBUTING.md`, `AGENTS.md`/`CLAUDE.md`, a public file format or CLI/MCP contract that has shipped in a release, licensing, or CI permissions. Those get the `needs-human` label and wait for review.
- Never force-push `main`, never rewrite published history, never merge with failing checks.

## When to stop and hand off

Label the issue `needs-human`, comment with a precise question and your recommendation, and move to the next eligible issue when:
- the work needs a decision that changes a design decision, scope, or a public contract;
- it needs schedule files we don't have, credentials, money, or a legal judgment;
- CI or the environment is broken in a way you cannot diagnose after a real attempt;
- the acceptance criteria are ambiguous enough that two reasonable readings lead to different code.

Stop the session entirely only when no eligible issue remains or when every remaining path needs the human.

## Checking features

- **Proof on real data:** when `$TOTALFLOAT_CORPUS` is set, run the feature on the corpus files and say in the PR what it showed. A new public file goes in with `cargo xtask corpus add`, which lists it in `corpus/manifest.toml`; the file itself is never committed.
- **Validators:** `cargo xtask validate xer|msproject FILE` checks a file TotalFloat wrote with outside tools ([docs/validation/validators.md](docs/validation/validators.md)). A pass shows file integrity.
- **Reserved for the maintainer,** on top of the hand-off list above: a rule whose public source is in doubt; adding or changing a validator tool.
- When a feature is added or a gap closes, update [docs/guide/supported.md](docs/guide/supported.md) and [docs/guide/limitations.md](docs/guide/limitations.md) in the same PR.

## Creating issues

Follow-up issues must be implementable by an agent with no other context. Use the task template sections: Context, Scope, Acceptance criteria (testable), Out of scope, References, and "Blocked by: #n" when relevant. Apply one `type:*`, one or more `area:*`, one priority, one `size:*`, and a status label. Anything sized `size:L` should be split. Attach it to the right milestone and add it as a sub-issue of that milestone's epic.

## Conventions

- **Rust:** stable toolchain pinned in `rust-toolchain.toml`, edition 2024. Libraries use `thiserror`; binaries may use `anyhow`. No `unwrap`/`expect` in library code outside tests. Public items get rustdoc; document the file field each model field maps to (for example the XER field name).
- **Time:** scheduling math uses integer minutes and civil (time-zone-free) datetimes. See [ARCHITECTURE.md](docs/ARCHITECTURE.md#data-model).
- **Tests:** unit tests beside the code; property tests (`proptest`) for codecs and calendar math; snapshot tests (`insta`) for CLI output; corpus tests read `$TOTALFLOAT_CORPUS` and skip cleanly when it is unset.
- **Commits:** Conventional Commits (`feat(xer): …`, `fix(engine): …`, `docs: …`, `chore(ci): …`), signed off with `git commit -s` (DCO).
- **Branches:** `<issue-number>-<slug>`. One issue per PR.
- **Dev tooling** lives in the `xtask` crate (`cargo xtask …`), not shell scripts, so it runs on every OS.

## Commands

| Command | Purpose |
|---|---|
| `just check` | fmt check, clippy (`-D warnings`), tests. Must pass before every PR. |
| `just test` | Tests only |
| `just fmt` | Format |

If a command in this table doesn't exist yet, the workspace hasn't reached that milestone.

## Repository map

```
crates/        Rust workspace members (see docs/ARCHITECTURE.md#crates)
xtask/         Dev tooling (corpus, fixtures, release helpers)
fixtures/      Synthetic / permissioned test schedules with MANIFEST.md
docs/          Architecture and design decisions, rules, guides
.claude/skills Agent workflows: next-issue, plan-milestone, write-rule, platform-test
.github/       CI, issue and PR templates
```
