# Platform Support

RunGlass is Linux-first for live command observation.

The product boundary is still one command, one receipt, but the collectors behind that receipt are platform-specific. Linux is the primary supported platform because current observation uses `/proc`, socket sampling helpers, Docker Engine state, and optional `strace` deep mode.

## Support Matrix

| Platform | Install/build | Inspect/export/validate | Live command observation |
| --- | --- | --- | --- |
| Linux x86_64 | Supported. Release archives are published for tagged releases. | Supported. | Supported. `normal` uses `/proc` and socket sampling; `deep` can use `strace`. |
| Linux other architectures | Expected to build from crates.io/source. | Supported. | Expected when Linux collector tools are available. |
| macOS | Experimental. The CLI is built in CI and release artifacts may be published for the hosted runner architecture. | Supported for existing receipts. | Not supported yet. `runglass run` and `runglass ci` exit with a Linux-first message. |
| Windows | Not supported. | Not supported. | Not supported. |

## macOS Boundary

macOS support in this release is for receipt inspection workflows:

- `runglass --help`
- `runglass doctor`
- `runglass demo`
- `runglass open <receipt>`
- `runglass report <receipt>`
- `runglass export <receipt>`
- `runglass validate <receipt>`

The live observation commands remain Linux-only for now:

- `runglass run ...`
- `runglass ci ...`

This is intentional. RunGlass should not claim process, network, Docker, or file-change fidelity on a platform until the collectors have been designed and tested there.

## CI Coverage

The main CI workflow runs full formatting, clippy, tests, and packaging checks on Linux.

The macOS readiness job builds the workspace, runs cross-platform CLI tests, builds the release binary, verifies non-observation commands, and checks that live observation exits with the Linux-first guard.
