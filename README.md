![RunGlass](https://raw.githubusercontent.com/error311/runglass/main/assets/branding/runglass_wordmark.svg)

# RunGlass

**Run one command. Get a receipt for what it actually did.**

RunGlass wraps a command, watches it run, and produces a local receipt you can inspect, export, share, and in many file-change cases revert. It is built for the commands where terminal history is not enough: AI agents, install scripts, package managers, Docker Compose, deploy scripts, and anything else you want an audit trail for.

![RunGlass full-stack receipt](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.gif)

[MP4 version](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.mp4)

## What You Get

RunGlass turns one command into a receipt with:

- stdout, stderr, exit code, duration, and timeline
- working-directory file changes with text diffs
- process tree and observed child processes
- best-effort network hosts and listening ports
- Docker containers, images, volumes, networks, and published ports
- risk notes for things worth reviewing
- HTML, Markdown, JSON, bundle, and reverse-patch exports
- file revert previews and apply flow for supported receipts

Example receipts:

- AI/code-change flow: [GIF](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/live-receipt-build.gif), [MP4](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/live-receipt-build.mp4)
- Docker Compose flow: [GIF](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/docker-compose-up.gif), [MP4](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/docker-compose-up.mp4)
- Full-stack flow: [GIF](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.gif), [MP4](https://raw.githubusercontent.com/error311/runglass/main/assets/showcase/provision-stack.mp4)

## Install

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
runglass run -- npm install
```

Open the receipt:

```bash
runglass report latest --open
```

Use deep mode when you want stronger process and network fidelity:

```bash
runglass run --deep -- docker compose up -d
```

Wrap an AI agent command:

```bash
runglass run --deep -- codex exec "fix this failing test"
```

## Why It Exists

Terminal history tells you what you typed.

RunGlass tells you what changed because you typed it.

That matters when a command can create files, edit config, pull images, open ports, phone home, spawn child processes, or leave behind state that is easy to miss.

## Common Workflows

RunGlass is strongest when one command has one clear boundary:

- `runglass run -- codex exec "fix this failing test"`
- `runglass run -- docker compose up -d`
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
```

Inspect and revert file changes:

```bash
runglass report latest --open
```

The web UI can preview file reverts, warn when files changed after the receipt ended, and apply supported reversions.

## Observation Modes

RunGlass currently supports two Linux observation modes:

- `normal`: adaptive `/proc` polling plus `ss` sampling. Fast, lightweight, and dependency-light.
- `deep`: normal mode plus `strace`-based exec and socket tracing for better short-lived process and outbound network visibility.

Docker changes are captured from Docker Engine before/after state. File changes are captured from a scoped working-directory snapshot and diff.

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
