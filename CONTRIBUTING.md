# Contributing to TotalFloat

Thanks for helping. TotalFloat is built by humans and AI agents working from the same playbook: read [AGENTS.md](AGENTS.md) first. It applies to you too.

## Setup

- Rust stable (pinned by `rust-toolchain.toml`) and [`just`](https://github.com/casey/just).
- `just check` runs formatting, clippy and tests. It must pass before you open a PR.
- Dev and test builds carry line tables only, enough for backtraces with file:line. To debug with gdb or lldb, build with full debug info: `CARGO_PROFILE_DEV_DEBUG=true cargo test --no-run` (or `cargo build`).

## Project skills

TotalFloat's workflows are project skills, versioned with this repository:

| Workflow | Claude invocation | Codex invocation |
|---|---|---|
| Work the next issue | `/next-issue` | `$next-issue` |
| Plan a milestone | `/plan-milestone` | `$plan-milestone` |
| Write or amend a rule | `/write-rule` | `$write-rule` |
| Test a build by hand on Linux or Windows | `/platform-test` | `$platform-test` |

Claude discovers these bundles in `.claude/skills/`; Codex discovers matching
bundles in `.agents/skills/` ([Codex skill locations](https://developers.openai.com/codex/skills/)).
An agent can also read the relevant `SKILL.md` directly when the host does not
expose a skill selector.

When adding, updating or removing a project skill in either directory, make the
same change in the other directory, including supporting files. `just check`
checks that the complete sets of files and their contents match in both
directions. Regular files keep discovery working on Windows without symlink
configuration. Machine-wide, system and plugin skills are managed separately.

## Fuzzing

Schedule files are untrusted input, so the parsers are fuzzed. The targets live in `fuzz/`, a separate crate outside the workspace (it needs nightly Rust and libFuzzer):

| Target | What it checks |
|---|---|
| `raw_reader` | Any bytes either fail cleanly or write back byte-identical |
| `typed_and_model` | Typed decoding, calendar resolution and model mapping never panic; patching an unmodified schedule changes nothing |
| `calendar_data` | The `clndr_data` parser never panics on any text |

```sh
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run raw_reader corpus/raw_reader ../fixtures/xer -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run typed_and_model corpus/typed_and_model ../fixtures/xer -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run calendar_data corpus/calendar_data seeds/calendar_data -- -max_total_time=600 -max_len=16384
```

The first directory is the working corpus (gitignored); the others are seeds. You can copy small corpus files into `fuzz/corpus/<target>/` for better coverage. That directory is gitignored, so never move its files anywhere tracked. A crash leaves its input in `fuzz/artifacts/<target>/`: fix the bug and add a regression test built from that input in the crate's unit tests. Commit the input only if it contains no real schedule data. CI runs each target briefly every week.

## Docs site

The Markdown in `docs/` is also a website, built with [mdBook](https://rust-lang.github.io/mdBook/) and published to GitHub Pages when `docs/` changes on `main`. `docs/SUMMARY.md` is its table of contents, and `docs/README.md` its landing page. A new guide page goes in `docs/SUMMARY.md` in the same place as in `docs/guide/README.md`; `just check` fails until it does. Pages left out of the table of contents, such as `docs/ARCHITECTURE.md`, are linked on GitHub.

To build the site into `target/site/` and check its links, install mdBook (the version `.github/workflows/pages.yml` names) and run `cargo xtask site`, or `cargo xtask site --mdbook <path>` when it isn't on your `PATH`.

## Workflow

1. Find or open an issue. Issues labeled `agent-ready` are fully specified; `good first issue` ones are a gentle start.
2. Branch as `<issue-number>-<slug>`, keep one issue per PR, and fill in the PR template.
3. Use Conventional Commits and sign off every commit (`git commit -s`) under the [Developer Certificate of Origin](https://developercertificate.org/).

## Ground rules

- Never commit real-world schedule files. Fixtures are synthetic or donated with permission to publish, listed in `fixtures/*/MANIFEST.md`; real files are linked from [the corpus](#the-corpus).
- Follow the [clean room](#clean-room) rules.
- Don't weaken tests to make them pass.

## Clean room

TotalFloat reads and writes other products' file formats. Keep it free of other parties' code and copyrighted material:

- Don't contribute code, documentation text or assets copied from proprietary products.
- Don't decompile or disassemble proprietary software.
- Don't copy code from copyleft projects. Reading them for facts, such as field names, is fine.
- Contribute only what you have the right to share.

## Calculation rules and their sources

Each calculation rule is TotalFloat's own, written in our words in [`docs/rules/`](docs/rules/README.md) with its sources ([ARCHITECTURE.md](docs/ARCHITECTURE.md#scheduling-rules)). A rule may rest on these, preferred in this order:

1. public CPM standards and literature, such as the GAO Schedule Assessment Guide, DCMA and NDIA guidance, AACE recommended practices, and published CPM papers and textbooks;
2. vendors' openly licensed documentation and published file schemas, cited and never copied;
3. what real schedule files store: [the corpus](#the-corpus), and files users donate from their own work;
4. our own design choice, with its reasoning.

Format facts, such as a file format's table, field and code names, its encodings and its units, may also come from any corpus file, other tools' test files included, and from open-source projects' code read for facts and never copied. A rule cites them as `format-fact`. They never decide how a schedule is calculated.

`cargo xtask rules` checks every rule's sources and the tests it lists ([docs/rules/README.md](docs/rules/README.md)).

Tests are written from the rule, with synthetic fixtures and expected values taken from the rule. [docs/PROVENANCE.md](docs/PROVENANCE.md) lists where every kind of knowledge in the code comes from, and what checks it.

## The corpus

TotalFloat is tested on real schedule files posted publicly. [`corpus/manifest.toml`](corpus/README.md) lists every one: where it's posted, the license stated there, the date we downloaded it and its SHA-256. The files themselves aren't in this repository, since most carry no license to redistribute them. Download them with:

```sh
export TOTALFLOAT_CORPUS=~/totalfloat-corpus   # any folder outside this repository
cargo xtask corpus fetch
```

`fetch` checks each file against its SHA-256 and reports any that have moved or changed. With `TOTALFLOAT_CORPUS` set, `cargo test` runs the corpus tests too; without it they skip, and the synthetic fixtures carry every test that ships.

- The corpus never holds a vendor's own sample or demo projects.
- Files another tool's authors wrote to test or demonstrate that tool are tagged `codec-only`. They're used for format facts and to test reading and writing files, never as evidence for how a schedule is calculated.
- If you posted a file listed here and want it taken off the list, open an issue and we'll remove it.

To add a public file, run `cargo xtask corpus add FILE --url URL --source NAME --license LICENSE --obtained YYYY-MM-DD` and commit the manifest change.

## Proposing a design change

The design decisions in [ARCHITECTURE.md](docs/ARCHITECTURE.md#design-decisions) and the rules on this page are settled. To change one, open an issue with the `needs-design` label: what you'd change, why, and what it affects. Once it's agreed, the PR that makes the change updates ARCHITECTURE.md or this page in the same commit.

## Donating schedules

Real schedule files are among the most valuable contributions. Please don't attach them to issues: open an issue with the `corpus` label and we'll arrange a transfer, anonymizing the file first if you like. We take a donated file only with your permission to publish it as a fixture under Apache-2.0, so donate only files from your own work that you're free to share.
