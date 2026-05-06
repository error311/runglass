# Examples

These example workloads are here to give RunGlass a few real commands worth wrapping.

Build RunGlass first:

```bash
source "$HOME/.cargo/env"
cargo build
```

## Compose Stack

`examples/compose-stack` is a small Docker Compose workload.

```bash
cd examples/compose-stack
../../target/debug/runglass run -- docker compose up -d
../../target/debug/runglass report latest --open
docker compose down -v
```

Use this when you want a receipt that lights up:
- Docker changes
- published ports
- network activity
- process tree

For the strongest all-panels receipt in this repo, use the scripted stack workflow instead:

```bash
cd examples/compose-stack
chmod +x reset-stack.sh provision-stack.sh
./reset-stack.sh
../../target/debug/runglass run -- ./provision-stack.sh
../../target/debug/runglass report latest --open
./reset-stack.sh
```

That single receipt is designed to light up:
- files and diff
- live output
- process tree
- network activity
- Docker changes
- timeline
- risk notes

## Install Script

`examples/install-script` is a harmless local installer-style script.

```bash
cd examples/install-script
chmod +x install.sh
../../target/debug/runglass run -- ./install.sh
../../target/debug/runglass report latest --open
rm -rf bin config
```

Use this when you want a receipt focused on:
- file changes
- text diffs
- created executables
- revert preview and apply

## NPM App

`examples/npm-app` is a tiny package project.

```bash
cd examples/npm-app
../../target/debug/runglass run -- npm install is-even --save-exact
../../target/debug/runglass report latest --open
rm -rf node_modules package-lock.json .npm-cache
```

Use this when you want a receipt that highlights:
- package-manager file changes
- outbound registry traffic
- child process activity

## Codex Agent Task

`examples/codex-agent` is a deterministic AI-agent-style JavaScript task. It ships with a local `bin/codex` shim so the example can produce a repeatable receipt without requiring live Codex credentials.

```bash
cd examples/codex-agent
chmod +x reset-agent-task.sh bin/codex
./reset-agent-task.sh
PATH="$PWD/bin:$PATH" ../../target/debug/runglass run -- codex exec "fix this failing js script"
../../target/debug/runglass report latest --open
```

Use this when you want a receipt for:
- non-interactive agent execution
- agent-written code changes
- before/after test output
- exported receipts you can paste into issues, docs, or handoffs

## CI Receipts

`examples/ci` includes starter GitHub Actions and GitLab CI jobs for running a command through `runglass ci` and uploading the generated receipt directory as an artifact.

Use these when you want remote runners or agent jobs to leave behind:
- HTML, Markdown, and JSON receipts
- a compact CI summary
- a failing job status when the wrapped command fails

Receipts themselves are not committed here. RunGlass generates them locally because they depend on your machine, Docker state, installed tools, working directory, and command output.
