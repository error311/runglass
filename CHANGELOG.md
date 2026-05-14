# Changelog

## v0.2.4 - 05/13/2026

- Added macOS readiness CI that builds the workspace, tests the cross-platform CLI surface, smokes receipt inspection commands, and verifies live observation exits with the Linux-first guard.
- Added macOS release artifact packaging alongside Linux x86_64 release archives.
- Improved `runglass doctor` and unsupported-platform messaging to clarify that receipt inspect/export/validate workflows can still work when live observation is unavailable.
- Updated the README platform matrix and added detailed platform support documentation.

## v0.2.3 - 05/12/2026

- Added a guarded GitHub PR helper in the web UI for previewing and explicitly posting/updating a compact receipt comment from the current receipt.
- Added web UI copy helpers for local GitHub dry runs, CI auto-comment commands, and a ready-to-use GitHub Actions workflow snippet.
- Improved `runglass github detect` with PR URL, token readiness, and next-step diagnostics.
- Made `runglass github comment --auto` print the detected GitHub context before posting.
- Hardened GitHub docs around web UI behavior, token handling, and PR comment permissions.

## v0.2.2 - 05/11/2026

- Added `runglass validate [latest|receipt-id|receipt.json|receipt-dir]` to check receipt JSON, CI artifact layout, and revert snapshot availability.
- Expanded `runglass doctor` with architecture, working-directory write access, reports-directory write access, shell, and `git` diagnostics.
- Added collector confidence notes to Markdown and AI receipt exports so file, process, network, and Docker fidelity are explicit.
- Documented the RunGlass trust model and validation workflow.

## v0.2.1 - 05/11/2026

- Made `cargo install runglass --locked` the primary README install path and documented local checkout installation separately.
- Added a Linux-first platform support matrix with explicit macOS and Windows observation limitations.
- Added a RunGlass dogfood GitHub Actions workflow that runs workspace tests through `runglass ci`, uploads the receipt artifact, and comments on pull requests when permissions allow.
- Tightened GitHub Actions examples to avoid comment attempts on forked pull requests without write permission and to keep receipt artifact workflows green when PR comment permissions are unavailable.
- Documented GitHub release binary archives and SHA-256 checksum artifacts for Linux x86_64 releases, and made the release artifact workflow install Rust explicitly before building.
- Focused the README demo section around one canonical receipt while keeping secondary demo links available.

## v0.2.0 - 05/09/2026

- Added a stable CI receipt directory layout with HTML, Markdown, JSON, compact summary, AI summary, reverse patch, stdout/stderr captures, diff artifacts, file snapshots, metadata, and `bundle.tar`.
- Added CI metadata to receipt JSON and the browser UI so CI receipts can show provider, repository, PR, commit, run URL, and artifact name when available.
- Updated GitHub PR comments to read receipts from `runglass-receipt/receipt.json` and use context-aware artifact wording for local dry runs versus CI runs.
- Added ready-to-copy GitHub Actions and GitLab CI workflow examples under `docs/examples/`.
- Documented first-class CI/PR receipt workflows, artifact upload behavior, exit-code behavior, and local smoke tests.
- Refreshed README copy and showcase media for the updated receipt UI.

## v0.1.5 - 05/08/2026

- Added `runglass github detect` for local and GitHub Actions repository/PR context detection.
- Added `runglass github comment --receipt latest --repo owner/name --pr N --dry-run` for previewing compact PR receipt comments.
- Added GitHub API comment create/update support using a stable RunGlass marker to avoid duplicate PR comments.
- Added token discovery from `GITHUB_TOKEN`, `GH_TOKEN`, or `gh auth token` without storing tokens in receipt artifacts or accepting tokens as CLI arguments.
- Documented GitHub Actions permissions and token handling.

## v0.1.4 - 05/08/2026

- Added compact Markdown receipt summaries for PR comments, CI summaries, issue reports, and logs.
- Added deterministic AI-friendly receipt summaries via `runglass report latest --ai` and `runglass export latest --format ai`.
- Added `runglass export latest --format summary-md` and comma-delimited `--format` aliases while keeping existing export flags.
- CI receipt mode now writes `ai-summary.txt` alongside `summary.md` and includes both compact summaries in bundle exports.
- Documented using RunGlass summaries around coding-agent commands and CI artifacts.

## v0.1.3 - 05/08/2026

- Made `runglass revert latest` preview-only by default and added explicit `--preview` and `--apply` flags for safer file reverts.
- Kept `--dry-run` as a compatibility alias for revert preview and added clearer errors for conflicting revert options.
- Renamed reverse patch exports to `reverse.patch` and hardened bundle exports around a portable `runglass-receipt-<id>/` layout.
- Updated revert UI and docs to say supported file changes instead of implying a whole-command undo.
- Documented what RunGlass file revert can and cannot undo.

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
