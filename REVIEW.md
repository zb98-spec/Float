# Codebase review

A review of the TotalFloat codebase as imported into this repository (upstream commit `1523c82`). Security findings are logged separately in [OPEN-ITEMS.md](OPEN-ITEMS.md).

## Purpose

TotalFloat is an open-source critical path method (CPM) scheduler for large projects (construction, energy, infrastructure). It opens Primavera P6 `.xer` files and Microsoft Project XML files, calculates dates and float with its own deterministic engine, explains why every date is what it is, checks schedule quality, compares updates, edits schedules and writes clean files back out.

It is built for people and AI agents to work on the same schedules: everything is available from a command-line tool with stable JSON output, from an MCP server for agents, and from a local browser app. Its central rule is that language models never calculate dates. They call the engine and propose edits that people can review.

## How it is built

One Rust workspace (edition 2024, toolchain 1.99.1, `#![forbid(unsafe_code)]` everywhere) of 14 crates plus `xtask`, and a React/TypeScript app in `ui/`.

| Crate | Lines | What it does |
|---|---:|---|
| `model` | 4,017 | The one schedule model every format maps into: activities, relationships, calendars, resources, costs, options. Integer minutes and time-zone-free dates. No I/O. |
| `calendar` | 857 | Work-time arithmetic: add or count working minutes, shifts, holidays, inherited exceptions. |
| `xer` | 25,375 | The P6 XER codec in three layers (raw bytes, typed tables, model). Byte-identical round trips; a patch writer that changes only edited cells; a fresh writer; anonymization. |
| `engine` | 9,986 | CPM forward and backward passes, float, constraints, progress and out-of-sequence modes, longest path, multiple float paths, and provenance for every date. Pure functions. |
| `check` | 1,383 | Schedule quality checks: the DCMA 14-point assessment and six more. |
| `diff` | 1,318 | Semantic comparison of two schedules. |
| `edit` | 13,468 | Typed edit commands (`AddActivity`, `SetDuration`, ...), each applied or refused with a reason. |
| `evm` | 369 | Earned value: PV, EV, AC, SPI, CPI, EAC. |
| `risk` | 1,796 | Monte Carlo schedule risk with three-point estimates, correlation and risk events. |
| `interop` | 3,106 | MS Project XML import and export, with a report of what didn't translate. |
| `text` | 7,437 | A line-based schedule text format (`.tfs`) with a reader and a canonical formatter. |
| `store` | 2,020 | Edit history in one SQLite file: changesets, branches, baselines, proposals, compressed snapshots. |
| `server` | 1,384 | The HTTP shell for the local app: loopback only, session token and cookie, host and origin checks, security headers, server-sent events. |
| `cli` | 53,452 | The `totalfloat` binary: every command, the MCP server and the local app's API. |

Interfaces all call the same service layer, so nothing is possible through only one of them:

- **CLI:** `inspect`, `schedule`, `explain`, `query`, `check`, `diff`, `variance`, `edit`, `new`, `import`, `export`, `calendar`, `risk`, `portfolio`, `history`, `timesheet`, `report`, `anonymize`, `textconv`, `fmt`, `roundtrip`, `serve`, `mcp`. Every command has `--json`, edits have `--dry-run`, and exit codes are fixed (`0` ok, `1` error, `2` usage, `3` findings).
- **MCP:** `totalfloat mcp` serves the same operations over stdio, with opened files referenced by handle and results paged.
- **Local app:** `totalfloat serve` runs the React app on `127.0.0.1`, with a grid, Gantt chart, network view, explain panel and history review.

## Quality

- **Tests:** 1,540 Rust tests pass on this import (`cargo test --workspace --locked`, run here on 2026-11-26), with property tests, snapshot tests and three fuzz targets (`raw_reader`, `typed_and_model`, `calendar_data`).
- **Engineering discipline:** strict clippy (pedantic, no `unwrap` in libraries), deterministic output, every calculation rule written down with sources (`docs/rules/`), `cargo deny` for licenses and advisories, release attestations and an SBOM.
- **Defensive by default:** output files are never overwritten (`persist_noclobber`), CSV and Excel cells that would run as formulas are neutralized, HTML output is escaped, XER parsing reports problems as diagnostics instead of panicking, and the local server is carefully locked down.
- **Maturity:** v0.1 with 8 commits upstream. Resource leveling, the Actual Dates out-of-sequence mode and baselines in XER files are not supported yet, and dates can differ from P6's ([limitations](docs/guide/limitations.md)).

## Features to develop

Five features chosen to fit the existing design (deterministic, file-first, no network) and to be reviewable one at a time. Each is on its own branch off `claude/hopeful-knuth-dg9q7b`.

| # | Feature | Branch | Why |
|---|---|---|---|
| 1 | **Merge hotspot check.** A `check` that flags unfinished activities with many predecessors, where merge bias makes the finish date optimistic. | `feature/merge-hotspot-check` | A standard schedule-quality concern (GAO Schedule Assessment Guide) that the current checks don't cover. |
| 2 | **Redundant logic check.** A `check` that flags relationships another path through the network already implies. | `feature/redundant-logic-check` | Redundant links clutter the network and hide the real driving logic; reviewers ask for them to be removed. |
| 3 | **Graphviz network export.** `export --to dot` writes the activity network as a Graphviz file, with the critical path highlighted. | `feature/export-dot` | Lets people render or diff the logic with standard tools, outside the app. |
| 4 | **Calendar export.** `export --to ics` writes activities and milestones as an iCalendar file. | `feature/export-ics` | Puts milestones and look-ahead work into Outlook, Google Calendar or a phone. |
| 5 | **Float bands.** A `float` command that counts unfinished activities in total-float bands (critical, near-critical, ...). | `feature/float-bands` | A quick read of schedule pressure that planners build by hand in spreadsheets today. |

## Status of the features

All five are built, tested and pushed, each on its own branch, and none is merged. Each branch passes `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` (1,540 tests before; each branch adds its own).

| Branch | Tests added | Notes for review |
|---|---:|---|
| `feature/merge-hotspot-check` | 3 | New thresholds `merge_predecessors` (10) and `merge_hotspot_percent` (5%). Check count goes from 20 to 21. |
| `feature/redundant-logic-check` | 5 | New threshold `redundant_logic_percent` (0%, like the other further checks). The search has a fixed budget and says `n/a` past it. |
| `feature/export-dot` | 4 | Output checked by rendering it with Graphviz (`dot -Tsvg`). |
| `feature/export-ics` | 5 | Output checked with the Python `icalendar` parser. |
| `feature/float-bands` | 6 | New command `float`, MCP tool `float`, guide page, JSON schema `totalfloat.float/1`. |

**Merge conflicts to expect.** Both check branches add a check to the same list, threshold struct and docs table, and both export branches add a variant to the same `Format` enum and docs. Each pair conflicts in a few lines and is resolved by keeping both sides. The check count in the tests becomes 22 once both check branches are in.

## Other findings

- **Test race in update mode.** With `TOTALFLOAT_UPDATE_CONTRACT=1`, `crates/edit/tests/commands.rs` rewrites `crates/edit/tests/golden/command-1.json` in one test while the property test `every_edit_survives_writing_and_reading` reads it in another, so the property test can fail and leave a `commands.proptest-regressions` file. It doesn't happen in normal runs (16,000 cases passed). Fix: only write the golden file when its content changes, or read it once behind a lock.
