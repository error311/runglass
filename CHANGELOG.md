# Changelog

## v0.1.2 - 05/07/2026

- Added `runglass open` as a shortcut for opening the latest receipt in the local browser UI.
- `runglass open <receipt-id>` and `runglass open --port 0` now mirror the receipt-serving path used by `runglass report`.
- Documented the shorter `runglass run <command...>` form while keeping `runglass run -- <command...>` available for unambiguous command wrapping.
- Improved CLI help and README examples around receipt opening and command wrapping.

## v0.1.1 - 05/06/2026

- Added clearer Linux-first platform messaging for command observation.
- `runglass run ...` now exits cleanly on unsupported platforms instead of attempting partial observation.
- `runglass doctor` now reports unsupported platforms explicitly while keeping non-observation diagnostics available.
- Added `runglass ci` for GitHub Actions, GitLab CI, and generic remote runners.
- CI mode writes HTML, Markdown, JSON, and summary artifacts before returning the wrapped command's exit code.
- Added starter GitHub Actions and GitLab CI receipt workflow examples.
- Refined the web UI with a more restrained receipt-tool visual style and fewer decorative gradients/glows.

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
