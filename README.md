# RunGlass

[![CI](https://github.com/error311/runglass/actions/workflows/ci.yml/badge.svg)](https://github.com/error311/runglass/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/runglass.svg)](https://crates.io/crates/runglass)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Linux first](https://img.shields.io/badge/platform-linux--first-2ea043.svg)](#install)

**Run a command. Get a receipt for what changed.**

RunGlass is a Linux-first CLI for commands where terminal history is not enough: AI agents, install scripts, package managers, Docker Compose, deploy scripts, and anything else you want an audit trail for.

```bash
runglass run --deep codex exec "fix this failing test"
```

Terminal history shows what you ran. RunGlass shows what happened:

- files created, modified, and deleted
- stdout, stderr, exit code, duration, and timeline
- child processes and process tree
- outbound network hosts and listening ports
- Docker containers, images, volumes, networks, and published ports
- risk notes for changes worth reviewing
- HTML, Markdown, JSON, compact summary, AI summary, bundle, and reverse-patch exports

Example CI/PR use case:

```bash
runglass ci --provider github --deep --out runglass-receipt -- codex exec "fix this failing test"
```

## What You Get

RunGlass turns one command boundary into a local receipt:

```text
Command: codex exec "fix this failing test"
Files:   3 created, 2 modified, 0 deleted
Runtime: 7 child processes, 2 outbound hosts, 0 listening ports
Docker:  0 containers, 0 images, 0 volumes
Risk:    low
Exports: receipt.html, receipt.md, receipt.json
```

Supported receipts can also preview and apply file reverts when RunGlass has stored the needed before-run snapshots.

## Example Receipt

![RunGlass full-stack receipt](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.gif)

[MP4 version](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.mp4)

Example receipts:

- AI/code-change flow: [GIF](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/live-receipt-build.gif), [MP4](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/live-receipt-build.mp4)
- Docker Compose flow: [GIF](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/docker-compose-up.gif), [MP4](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/docker-compose-up.mp4)
- Full-stack flow: [GIF](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.gif), [MP4](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.mp4)

## Install

RunGlass is Linux-first in this release. `normal` mode uses Linux process and socket sources, and `deep` mode uses `strace` when available. macOS and Windows command observation is not supported yet.

Build locally:

```bash
source "$HOME/.cargo/env"
cargo build
```

Install the CLI into your Cargo bin directory:

```bash
source "$HOME/.cargo/env"
cargo install --path crates/runglass-cli
```

`normal` mode is a single-binary experience. The embedded web UI, local server, collectors, exports, and revert flow ship inside the Rust `runglass` binary.

`deep` mode is still the same binary, but on Linux it uses `strace` when available for better short-lived process and socket visibility.

## Quick Start

Run a command:

```bash
runglass run npm install
```

Open the receipt:

```bash
runglass open
```

Use deep mode when you want stronger process and network fidelity:

```bash
runglass run --deep docker compose up -d
```

Wrap an AI agent command:

```bash
runglass run --deep codex exec "fix this failing test"
```

## Why It Exists

Terminal history shows what you ran.

RunGlass shows what happened.

That matters when a command can create files, edit config, pull images, open ports, phone home, spawn child processes, or leave behind state that is easy to miss.

## Common Workflows

RunGlass is strongest when one command has one clear boundary:

- `runglass run -- codex exec "fix this failing test"`
- `runglass run codex exec "fix this failing test"`
- `runglass run -- docker compose up -d`
- `runglass run docker compose up -d`
- `runglass run -- npm install`
- `runglass run -- ./install.sh`
- `runglass run -- ./deploy-preview.sh`

The repository includes repeatable workloads under [examples](https://github.com/error311/runglass/tree/main/examples), but they are support material. RunGlass itself is meant to wrap your real commands.

## Revert And Export

Export the latest receipt:

```bash
runglass export latest --html
runglass export latest --markdown
runglass export latest --json
runglass export latest --reverse-patch
runglass export latest --bundle
runglass export latest --format summary-md
runglass export latest --format ai
```

Preview and apply supported file reverts:

```bash
runglass revert latest --preview
runglass revert latest --apply
runglass revert latest --apply --skip-changed
runglass open
```

The web UI can preview file reverts, warn when files changed after the receipt ended, and apply supported reversions.

### What Revert Can And Cannot Undo

RunGlass reverts supported file changes from a receipt. It compares the current workspace to the receipt's recorded after-state before writing anything.

It can:

- restore modified files when stored before-run snapshots are available
- delete files that were created by the wrapped command and have not changed since
- restore files that were deleted by the wrapped command when stored before-run snapshots are available
- skip or force files that changed after the receipt finished

It does not automatically roll back non-file side effects such as Docker changes, network calls, external database writes, package manager global changes, external service mutations, or commands run outside the watched working directory.

RunGlass still helps with those side effects by showing the observed Docker before/after state, outbound hosts, listening ports, process activity, command output, and risk notes. For example, a receipt can show which containers, images, volumes, networks, or ports appeared during a command so you can decide on the right cleanup action deliberately.

Bundle exports are portable tar archives named `runglass-receipt-<id>.tar` with a `runglass-receipt-<id>/` directory containing `receipt.html`, `receipt.md`, `summary.md`, `ai-summary.txt`, `receipt.json`, `reverse.patch`, and an `artifacts/` folder.

## Compact Summaries

Use compact summaries when the full HTML receipt is too much for a PR comment, CI log, issue report, or AI-agent feedback loop:

```bash
runglass export latest --format summary-md
runglass report latest --ai
runglass export latest --format ai
```

`summary-md` is a short Markdown report with impact counts and review-next items. `ai` is a deterministic text block designed to paste back into a coding agent without dumping full logs by default.

Example agent workflow:

```bash
runglass run -- codex exec "fix this failing test"
runglass export latest --format ai
```

## CI Receipts

Use `runglass ci` when an agent, install script, or remote runner should leave reviewable artifacts behind:

```bash
runglass ci --provider github --deep --out runglass-receipt -- codex exec "fix this failing test"
```

The command writes `receipt.html`, `receipt.md`, `receipt.json`, `summary.md`, and `ai-summary.txt` to the output directory. In CI mode, RunGlass returns the wrapped command's exit code after artifacts are written, so failing commands still fail the job while keeping the receipt available.

In GitHub Actions, `runglass ci --provider github` appends the compact Markdown summary to `$GITHUB_STEP_SUMMARY` when that file is available. In generic CI, publish the output directory as an artifact or append `summary.md` to your platform's job summary:

```bash
runglass ci --out runglass-receipt -- npm test
cat runglass-receipt/summary.md >> "$GITHUB_STEP_SUMMARY"
```

Starter workflows are included for [GitHub Actions](examples/ci/github-actions.yml) and [GitLab CI](examples/ci/gitlab-ci.yml).

## Observation Modes

RunGlass currently supports two Linux observation modes:

- `normal`: adaptive `/proc` polling plus `ss` sampling. Fast, lightweight, and dependency-light.
- `deep`: normal mode plus `strace`-based exec and socket tracing for better short-lived process and outbound network visibility.

Docker changes are captured from Docker Engine before/after state. File changes are captured from a scoped working-directory snapshot and diff.

## How It Works

RunGlass keeps one command as the unit of review:

- snapshots the working directory before and after the command
- samples Linux process and socket state while the command runs
- uses `strace` in `deep` mode when available for better short-lived exec and socket visibility
- compares Docker Engine state before and after the command
- stores stdout, stderr, JSON receipt data, export artifacts, and reversible file snapshots locally

The result is intentionally best-effort rather than magic. RunGlass is designed to answer "what changed because I ran this command?" with useful evidence, while still being clear about snapshot caps, ignored paths, platform support, and tracing limitations.

## Snapshot Controls

RunGlass defaults to a `10 MiB` per-file snapshot cap so one large artifact does not turn a receipt into a heavy crawl.

Override the cap:

```bash
RUNGLASS_MAX_SNAPSHOT_BYTES=26214400 runglass run -- ./install.sh
```

Ignore local paths with `.runglassignore`:

```gitignore
dist/
node_modules/
*.sqlite
tmp/
secret.env
```

See [`.runglassignore.example`](https://github.com/error311/runglass/blob/main/.runglassignore.example) and [`.env.example`](https://github.com/error311/runglass/blob/main/.env.example) for starter settings.

## CLI Helpers

```bash
runglass list --query docker --risk medium --mode deep
runglass open latest
runglass open <receipt-id> --port 0
runglass report latest --ai
runglass prune --keep 50 --dry-run
runglass delete latest
runglass doctor
runglass snapshot --dry-run
```

## Scope

RunGlass is for one command, one boundary, one receipt.

It is not trying to be:

- a full terminal replacement
- a multi-command shell session recorder
- perfect system-wide tracing for every event on the machine

It is a pragmatic local receipt for commands you want to understand before you trust.
