---
name: platform-test
description: Hands-on test of a TotalFloat build on Linux or Windows. Install it as a user would, run the user guide's commands in the platform's shells, and click through every view, command, key and dialog of the local app. Use before a release, after large app changes, or when asked for a Windows test, a Linux test or a UI sweep.
---

# Platform test

The automated checks cover a lot. `crates/cli/tests/guide.rs` runs every command in the user guide, on Linux in CI and weekly on Windows. `ui/e2e/*.mjs` drive scripted paths through the app. This test covers the rest:

- the release archive, installed the way `docs/guide/install.md` says;
- the guide's commands typed in PowerShell 5.1 and cmd, where quoting, paths and code pages differ from the test harness;
- a person's pass over the whole app, looking at each screen in Chromium, Firefox and Edge.

It is slow and done occasionally. Each run is tracked on one GitHub issue.

## Rules

- **Synthetic data only:** `fixtures/`, files the guide test writes, and the 50,000-activity schedule `cargo xtask ui-bench` generates. Never a corpus file.
- **One heavy job at a time:** cargo builds and tests, browsers and the VM each count. Run cargo under `flock -o /tmp/totalfloat-heavy.lock`, and don't build while the VM is up.
- **Scratch** goes in `~/.cache/tf-platform-test/`, and is deleted at the end. `/tmp` is wiped on reboot.
- **Leave the machine as you found it:** the installed `totalfloat`, `PATH`, and the VM's state. On Windows, record every change left in the guest in the VM's notes file.

## 1. Plan the run

1. Open a tracking issue, or use the one you were given. Note on it:
   - the build;
   - the platforms and browsers;
   - which parts you'll run: install, guide commands, app sweep.
2. Choose the build:
   - **Install and Windows command checks** need a published release or pre-release. Its archives come from the release workflow on a version tag, and tags are the maintainer's. Record the archive names and their `SHA256SUMS` lines.
   - **An app sweep between releases** uses a host build of current `main`: `cargo xtask ui`, then `cargo build --release -p totalfloat-cli`. On Windows, Edge in the VM opens the host's server (step 5). That tests the app in Edge, but not the Windows binary.

Done when the issue names the exact build (tag or commit) and the parts you'll run.

## 2. Make the test data

In the scratch folder:

1. Make history stores from fixtures, as the guide does:
   - **plant:** `fixtures/xer/batches/001/rel-fs-positive.xer`, initialised with `totalfloat history init`, with a baseline, a branch, and a proposal from `edit … --propose`;
   - **usage:** `fixtures/xer/usage.xer`, for resources, limits and the Usage panel;
   - **two projects:** `fixtures/xer/two-projects.xer`, for the project picker and a portfolio (`docs/guide/portfolio.md`).
2. Generate the 50,000-activity schedule with `cargo xtask ui-bench` (see `docs/PERFORMANCE.md`), for scrolling and edit speed.
3. For Windows command checks, make the seed folder. Read `seed()` in `crates/cli/tests/guide.rs` and write the same files: it copies fixtures and writes small CSV, TOML, JSON and workbook files.

Done when each store opens with `totalfloat serve`, and the seed folder holds every file `seed()` writes.

## 3. Install checks

Follow `docs/guide/install.md` word for word, including "Check it works" and "Uninstall".

- **Linux:** run the one-line installer with `TOTALFLOAT_INSTALL_DIR` set to a scratch folder, so the machine's own `totalfloat` stays as it is. Do "Install by hand" too.
- **Windows:** in the VM (step 5), run the `irm … | iex` line in Windows PowerShell 5.1, open a new window, and check `totalfloat --version`. Then follow "Install by hand" and the `Path` steps.
  - Before a clean-install check, remove any earlier install the VM's notes file lists.
  - Afterwards, restore the user `Path` as an `ExpandString`.

Done when each documented step either worked as written or has an issue filed.

## 4. Guide commands on Windows

1. List every `totalfloat` line in each guide page's `sh` blocks. That is the same rule `commands()` in `guide.rs` uses.
2. Write one PowerShell script and one cmd script that, for each command:
   - copy the seed folder into a fresh folder per page;
   - run the command there, sending output to `out_NN.txt` and errors to `err_NN.txt`;
   - append `page`, `NN`, the exit code and the command to `index.tsv`.

   `serve` lines need a timeout, since they run until stopped.
3. Copy the scripts and the seed folder in through the VM's exchange folder. Run each script and copy the results out.
4. Compare with the same commands on Linux.
   - Expected differences: line endings and path separators.
   - Anything else is a bug: an exit code that differs, an error, quoting that breaks, or text turned to mojibake.

Done when every command has a row in `index.tsv` for both shells, and every difference is explained or filed.

## 5. App sweep

### Set up the browser

- **Linux:** use the `playwright-cli` skill against `totalfloat serve <store>`.
  - Run the sweep in Chromium first.
  - Then run it in Firefox, by pointing `PLAYWRIGHT_MCP_CONFIG` at `{"browser": {"browserName": "firefox"}}` (`ui/README.md`).
  - Take a screenshot at each step and look at it.
- **Windows:** use a Windows VM, with the maintainer's machine-wide `local-vm` skill. Its Windows reference covers SSH into the guest and serving a host app to the guest's Edge. Drive Edge with screenshots, keys and clicks.
  - **Host build:** run `serve` on the host. Forward its port into the guest with `ssh -R 127.0.0.1:<port>:127.0.0.1:<port>`, then open the link in Edge. The server answers only `127.0.0.1` and `localhost`, so the guest's `10.0.2.2` address won't work.
  - **Release build:** start `serve` inside the guest from SSH, then open its link in Edge. A browser started over SSH doesn't reach the desktop, so open Edge from the Run box.
  - Shut the guest down with `shutdown /s /f /t 0` over SSH when done.

### Build the checklist

Build the checklist fresh each run, so it matches the app being tested:

- **Commands:** every action registered in `ui/src/commands.ts` and the files that register actions through it. Run each one once from the Menu or the command search (Ctrl+K), and once by its keys. Press `?` and compare the key list with the registry.
- **Guide sections:** every section of `docs/guide/serve.md`, with each thing it says the app does. That covers the window, the Gantt, filtering and grouping, the inspector, building logic, Why, a portfolio, resource usage, risk, history and branches, printing, layouts, reviewing proposals, editing in the grid (adding and deleting activities, WBS nodes), and following changes.
- **Dialogs:** open each one, cancel it, then complete it. Try an empty field and a wrong value.
- **States:**
  - a read-only `.xer`;
  - a baseline (read-only);
  - a narrow window;
  - the 50,000-activity schedule, timing a scroll, an edit's preview and a recorded edit against `docs/PERFORMANCE.md`;
  - a second change made from the command line while the app is open, which the app should show live.

### Run it

Work through the checklist in order. For each item, record pass or fail with the screenshot path, and file one issue per bug. Each issue gets its steps, the browser and platform, and what the screenshot showed.

Done when every checklist item has a result in each browser, and each failure has an issue.

## 6. Clean up and report

1. Stop every `serve`, close the browsers, and shut down the VM.
2. Update the VM's notes file with what stays in the guest: installed builds, `Path`, test folders, Edge settings.
3. Remove the scratch install and `~/.cache/tf-platform-test/`.
4. Comment on the tracking issue:
   - the build and archive hashes;
   - the OS, shell and browser versions;
   - counts of passes and failures per part;
   - the issues filed;
   - anything the next run should know.
5. If a platform or browser claim changed, update `docs/guide/supported.md` and `docs/guide/limitations.md` in a PR.

Done when the report is posted and nothing from the run is left running or on disk outside the VM's recorded state.
