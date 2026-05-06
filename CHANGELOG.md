# Changelog

## v0.1.0 - Initial Release

RunGlass is now available as a Linux-first local command receipt tool.

This initial release includes:

- A Rust CLI for running one command and generating a receipt of what happened.
- An embedded local web UI for inspecting receipts without a separate frontend server.
- Live command runs with stdout/stderr previews, status updates, and stop support.
- Working-directory file snapshots, text diffs, and receipt-aware file revert previews.
- Process tree, network, listening port, Docker, timeline, and risk sections.
- `normal` observation mode for fast local receipts.
- Linux `deep` mode using `strace` for stronger short-lived process and socket visibility.
- Export support for HTML, Markdown, JSON, bundles, and reverse patches.
- Recent receipt browsing, search, deletion, and snapshot controls.
- Example workloads for AI-agent commands, Docker Compose, npm installs, and installer scripts.
- Brand assets, showcase captures, and GitHub CI for format, clippy, tests, and package checks.

Notes:

- RunGlass is Linux-first for this release.
- `deep` mode currently depends on `strace`.
- Docker visibility is based on Docker Engine before/after state.
- File changes are scoped to the command working directory.
