# Changelog

## Unreleased

### Added
- Linux `deep` mode using `strace` for stronger short-lived process and outbound socket visibility.
- Live browser-triggered runs with SSE updates and stop/cancel support.
- File revert support for observed receipts, including revert preview, selected-file revert, reverse patch export, and changed-since-receipt conflict detection.
- Markdown export in both the CLI and browser UI.
- Example workloads for Docker Compose, installer scripts, package installs, and AI-agent CLI flows.
- Core unit tests for revert behavior, JSON export, Markdown rendering, timeline wording, and risk derivation.
- Browser automation coverage for summary navigation, live runs, export routes, and revert UI states.

### Changed
- Renamed the product framing throughout the UI from generic reports to receipts.
- Improved receipt wording for summaries, timeline events, risk notes, and Markdown export output.
- Improved browser UX with stronger focus feedback, styled help overlays, better disabled states, and download-oriented export actions.
- Split large Rust and embedded frontend files into smaller modules for maintainability.

### Notes
- `normal` mode remains the default fast path.
- `deep` mode is Linux-focused and currently depends on `strace`.
