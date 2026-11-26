# TotalFloat

> This repository is a copy of [ijd5004/totalfloat](https://github.com/ijd5004/totalfloat) (Apache-2.0), imported with its full history at commit `1523c82`. Links to releases, install scripts and the docs site still point to the upstream project.

[![CI](https://github.com/ijd5004/totalfloat/actions/workflows/ci.yml/badge.svg)](https://github.com/ijd5004/totalfloat/actions/workflows/ci.yml)

**Open-source, agent-native CPM scheduling.** TotalFloat is an open-source CPM scheduler that reads and writes Primavera P6 XER files and Microsoft Project XML files, calculates schedules with its own deterministic engine and documented rules, and exposes everything through a CLI and an MCP server so humans and AI agents can work on real project schedules together.

> **Status: v0.1, an early release.** It's usable on real schedules, but read the [limitations](docs/guide/limitations.md) before relying on its dates. See the [changelog](CHANGELOG.md).

## Why

Large, complex projects (construction, energy, infrastructure, defense) are planned with critical path method (CPM) schedules, usually kept in commercial desktop tools such as Oracle Primavera P6 and Microsoft Project. Contracts call for schedule files in those formats, and the people who review, update and argue over them need to see why every date is what it is.

TotalFloat is a single program that can open a schedule file, calculate it, explain why every date is what it is, check schedule quality, compare updates, edit or build schedules, and write a clean file back out. Its calculation rules are its own, written down with their public sources ([ARCHITECTURE.md, Scheduling rules](docs/ARCHITECTURE.md#scheduling-rules)). Your existing tool stays your system of record until you decide otherwise. A local browser app comes with it as a preview, and a team server comes later, built on the same engine.

## Install

New here? [Start here](docs/guide/start-here.md): what TotalFloat does with your schedule, how to install it, and the first things to try, written for people who don't write code.

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.ps1 | iex
```

On Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.sh | sh
```

Each downloads the latest release for x86-64, checks it against `SHA256SUMS`, and installs it. The Windows script adds the program's folder to your `Path`; the Linux one installs to `~/.local/bin` and prints the line to add if that folder isn't on your `PATH`. Run it again to upgrade. The [install guide](docs/guide/install.md) covers installing by hand from [Releases](https://github.com/ijd5004/totalfloat/releases) and uninstalling. A build for macOS is coming soon; until then, build from source.

To build from source on any of the three, install [Rust](https://rustup.rs) and run `cargo install --path crates/cli`. That build leaves out the local browser app unless you build it first ([ui/README.md](ui/README.md)).

## Principles

- **Dates you can trace.** Every date and float comes from a deterministic engine, and `explain` says which link, constraint or calendar set it.
- **Lossless file round-trips.** Read an XER, change one thing, write it back: only that thing changes.
- **Deterministic engine, agent-friendly interfaces.** Language models never calculate dates. They call the engine through the CLI or MCP and propose changes humans can review.
- **Bring your own agent and model.** The core never calls an LLM and never phones home.
- **Runs anywhere.** One binary on a laptop, with no database, up to a self-hosted team server.
- **Free and open source** under Apache-2.0. No paid tier.

## Documentation

| Document | What's in it |
|---|---|
| [User guide](docs/guide/README.md) | Installing and using every command |
| [Rules](docs/rules/README.md) | TotalFloat's calculation rules and their sources |
| [Provenance](docs/PROVENANCE.md) | Where TotalFloat's knowledge comes from, and what checks it |
| [Architecture](docs/ARCHITECTURE.md) | How it's built, and the design decisions behind it |
| [Glossary](docs/GLOSSARY.md) | Scheduling terms, with the file fields they map to |
| [Agents](docs/agents.md) | Connecting an agent over MCP |
| [Checks](docs/CHECKS.md) | Schedule quality checks |
| [Releasing](docs/RELEASING.md) | How releases are built and checked |
| [Contributing](CONTRIBUTING.md) | Development setup and workflow |

## License

Apache-2.0. See [LICENSE](LICENSE).

Oracle, Primavera, and Primavera P6 are trademarks of Oracle and/or its affiliates. Microsoft and Microsoft Project are trademarks of the Microsoft group of companies. TotalFloat is an independent project and is not affiliated with or endorsed by Oracle or Microsoft.
