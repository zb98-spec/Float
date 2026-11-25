# TotalFloat

**Open-source, agent-native CPM scheduling.** TotalFloat is an open-source CPM scheduler that reads and writes Primavera P6 XER files and Microsoft Project XML files, calculates schedules with its own deterministic engine and documented rules, and exposes everything through a CLI and an MCP server so humans and AI agents can work on real project schedules together.

## Install

<div class="tf-install">

On **Windows**, open PowerShell, paste this line and press Enter:

```powershell
irm https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.ps1 | iex
```

On **Linux**, run this in a terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.sh | sh
```

</div>

Each command downloads the latest release, checks it against the release's checksums, and installs `totalfloat`. On Windows it also adds TotalFloat to your `Path`; on Linux it prints the line to add if `~/.local/bin` isn't on your `PATH`. Run it again to upgrade. The [install guide](guide/install.md#install-with-one-command) has the details.

To install by hand, download the archive for your system instead:

<div class="tf-downloads">
<a class="tf-button" href="https://github.com/ijd5004/totalfloat/releases/latest/download/totalfloat-x86_64-pc-windows-msvc.zip">Download for Windows</a>
<a class="tf-button" href="https://github.com/ijd5004/totalfloat/releases/latest/download/totalfloat-x86_64-unknown-linux-gnu.tar.gz">Download for Linux</a>
<span class="tf-button tf-button-soon" aria-disabled="true">macOS installer coming soon</span>
</div>

The buttons download the latest release for x86-64: a `.zip` file for Windows and a `.tar.gz` file for Linux. [Install by hand](guide/install.md#install-by-hand) shows how to unpack it and put `totalfloat` on your `PATH`.

On a Mac, you can [build TotalFloat from source](guide/install.md#build-from-source) today: install [Rust](https://rustup.rs) and run `cargo install --path crates/cli` from a copy of the repository.

TotalFloat v0.1 is an early release. It's usable on real schedules, but read the [limitations](guide/limitations.md) before you rely on its dates.

## Where to go next

- [Start here](guide/start-here.md) if you schedule projects and don't write code.
- [Install](guide/install.md) TotalFloat on Windows or Linux.
- [User guide](guide/README.md): every command, with examples.
- [Using TotalFloat from an agent](agents.md): connect an AI agent over MCP.
- [Rules](rules/README.md): TotalFloat's calculation rules and their sources.
- [Source code on GitHub](https://github.com/ijd5004/totalfloat)
