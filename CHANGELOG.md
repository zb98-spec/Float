# Changelog

## Unreleased

### Added

- A [Start here](docs/guide/start-here.md) page for people who schedule projects and don't write code: what TotalFloat does, how to install it, and the first things to try with an AI agent, the local app or the command line.
- One-command install. On Windows, `irm https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.ps1 | iex` in PowerShell; on Linux, `curl -fsSL https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.sh | sh`. Each downloads a release, checks it against `SHA256SUMS`, and installs it; on Windows it also adds the program's folder to the user `Path`. Running it again upgrades. `TOTALFLOAT_VERSION` picks a release. The install guide has an Uninstall section.

### Changed

- Each release also publishes its archives under names without the version, `totalfloat-x86_64-pc-windows-msvc.zip` and `totalfloat-x86_64-unknown-linux-gnu.tar.gz`, so the docs site's download buttons always get the latest release.
- The docs site's front page starts with the one-line install commands.
- The install scripts end by pointing to the [Start here](https://ijd5004.github.io/totalfloat/guide/start-here.html) page.
- The Windows zip holds `totalfloat.exe`, `LICENSE` and `README.md` at its root. Explorer's **Extract All** puts them in one folder named after the zip, where the v0.1.0 zip gave a folder inside a folder of the same name.

## v0.1.0

The first release. TotalFloat is a single program that reads, schedules, checks, edits and writes Primavera P6 `.xer` files, as well as Microsoft Project XML files, from the command line or through an MCP server for AI agents. P6 doesn't need to be installed. Its calculation rules are its own, written down with their public sources. This release is for Linux (x86-64) and Windows (x86-64); a macOS build is coming soon.

### What it does

- **Reads and writes XER files losslessly.** A file read and written back is byte-identical, and an edit changes only the cells it touches.
- **Schedules.** `schedule` calculates dates, float and the critical path of a file's projects, honouring the scheduling settings it holds:
  - calendars;
  - constraints, with unstarted work held at the data date when a mandatory date falls before it;
  - the Retained Logic and Progress Override out-of-sequence modes;
  - level-of-effort and WBS-summary activities, and resource-dependent ones on a single resource calendar;
  - linked projects;
  - multiple float paths, numbered by TotalFloat's own rule, which `schedule --float-paths` also asks for in one run.
- **Explains dates.** For any date, it says which link, constraint or calendar set it.
- **Edits and builds schedules.** You can:
  - add, change and delete activities, links, constraints and assignments, and add resources;
  - give a resource a limit in units per hour, from the command line, JSON, MCP, spreadsheets or schedule text;
  - give a role a limit the same way, which `portfolio capacity` measures against;
  - spread an assignment's units evenly, by one of the file's resource curves, or by units typed per day, week or month;
  - book assignment and expense costs to cost accounts;
  - change many activities at once with expressions such as `remaining_duration=original_duration*0.5` or `constraint_date=early_start+5d`;
  - record progress and recalculate at a new data date, where a units percent splits the units of each labor and nonlabor assignment;
  - write timesheets of each team member's work for a period, and apply the hours and progress they fill in;
  - export a sheet, change it in a spreadsheet, and read the changes back with `edit --from-sheet`;
  - start a new schedule, or build one from a spreadsheet.
- **Checks quality.** It runs the DCMA 14-point assessment plus further checks, including P6's stored thresholds. `check --html` writes a page to send on.
- **Compares two updates.** It reports what changed, what moved and what's worth a look. `variance` also writes Markdown and an HTML page.
- **Moves schedules to and from Microsoft Project XML.** Files written by other software import with their tasks, links, resources and assignments. Exports are checked against Microsoft's published schema and read back through MPXJ.
- **Reports and portfolios.**
  - Report templates, resource usage, and costs by cost account.
  - Filters with "any of" groups, in `query`, MCP, saved filters and layouts.
  - An EPS view of several projects.
  - A Monte Carlo schedule risk analysis.
- **Keeps history.** It records changes, branches, proposals and merges, and keeps schedules as text in git.
- **Works with your agent.** `totalfloat mcp` serves the same reports to any MCP client. The core never calls a model and never phones home.
- **A local browser app, as a preview.** `totalfloat serve` shows a schedule in your browser, and the release binary has the app built in. Its views are Schedule (a grid, a Gantt chart and an inspector), Network, Resources, Risk and History, each with its own key, and a Menu button holds every action. In the app you can:
  - edit names, durations, calendars, constraints and actual dates by typing in the grid, with a Changes panel listing every edit waiting and what they do together;
  - build a schedule: add and delete activities, add, rename and move to WBS nodes, and add, change and remove relationships in the inspector or by drawing them on the Gantt;
  - see everything about the selected activity in the inspector, and why its dates are what they are, step by step;
  - show driving paths, logic lines and the whole project's activity network;
  - group by up to four levels, kept in layouts, and filter with "any of" groups;
  - see a resource's units per week or month against its limit, and change the limit;
  - see branches, merges, baselines and proposals as a graph in the History view;
  - run a Monte Carlo schedule risk analysis.

  The [serve guide](docs/guide/serve.md) says who can reach the app and which keys do what. Deleting a WBS node, and editing an activity's codes, notes and resources in the inspector, aren't in the app yet.

### How far to trust it

- **Confirm dates before you commit to them.** TotalFloat's dates can differ from the ones another scheduler stored in the same file; `schedule` lists where. The [limitations](docs/guide/limitations.md) say what isn't handled yet, and what TotalFloat refuses rather than guess.
- **Its rules are written down.** Each calculation rule written so far is in [docs/rules](docs/rules/README.md), with the sources it rests on.
- **Resource leveling isn't there yet.** Resources can have limits, and the Usage panel marks periods over them, but nothing levels.
- **XER is the stable format.** TotalFloat's own files (history stores, `.tfs` schedule text, layouts and saved filters) and its JSON output may change before 1.0.
- **Files pass XER Tools' integrity checks** ([record](docs/validation/xertools-2.2b.md)). Importing them into other tools hasn't been verified.
- **[What TotalFloat supports](docs/guide/supported.md)** lists its features by area, with what each doesn't do yet.

### Install

Download the Linux (x86-64) or Windows (x86-64) archive from the release, check it against `SHA256SUMS`, and put `totalfloat` on your `PATH`. The Windows build needs no Visual C++ Redistributable. On macOS, build from source for now; the source build passes the tests on all three. See the [install guide](docs/guide/install.md).

Oracle, Primavera and Primavera P6 are trademarks of Oracle and/or its affiliates. Microsoft and Microsoft Project are trademarks of the Microsoft group of companies. TotalFloat is an independent project and is not affiliated with or endorsed by Oracle or Microsoft.
