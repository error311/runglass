# RunGlass Trust Model

RunGlass is a command receipt tool, not a system-wide forensic tracer. It records useful evidence for one wrapped command and labels collector fidelity honestly.

## Collector Confidence

| Area | Confidence | What It Means |
| --- | --- | --- |
| Files | High | RunGlass snapshots the watched working directory before and after the command, then records created, modified, and deleted files within snapshot limits. |
| Docker | High when available | RunGlass compares Docker Engine state before and after the command. It can show containers, images, volumes, networks, and published ports that changed. |
| Processes | Medium in normal mode, higher in deep mode | Normal mode uses adaptive Linux `/proc` polling. Deep mode adds `strace` command-tree exec tracing, which improves short-lived process visibility. |
| Network | Best effort in normal mode, higher in deep mode | Normal mode uses `/proc` socket polling plus `ss` sampling. Deep mode adds `strace` socket tracing. PID attribution and very short-lived sockets can still be incomplete. |

## What RunGlass Does Not Claim

RunGlass does not claim to undo or fully trace:

- Docker changes
- network calls
- database writes
- package manager global changes
- external service mutations
- commands run outside the watched working directory
- all system-wide activity on the machine

Supported file reverts are limited to file changes captured in the receipt with the needed before-run snapshots.

## Validation

Use `runglass validate` to check whether a receipt is structurally useful:

```bash
runglass validate latest
runglass validate <receipt-id>
runglass validate runglass-receipt/receipt.json
runglass validate runglass-receipt/
```

Validation reports:

- receipt JSON parse and required metadata errors
- CI artifact layout warnings
- missing stdout/stderr artifact warnings
- missing file snapshot warnings that may limit supported reverts

Warnings do not mean the command receipt is worthless. They mean some supporting artifacts or reversible snapshots are missing.
