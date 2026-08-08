# Debian And Ubuntu Packaging

RunGlass publishes an `amd64` Debian package alongside the Linux release tarball. The package installs:

- `/usr/bin/runglass`
- `/usr/share/man/man1/runglass*.1.gz`
- `/usr/share/doc/runglass/README.md`
- `/usr/share/doc/runglass/CHANGELOG.md`
- `/usr/share/doc/runglass/copyright`

The release binary is built on Ubuntu 22.04 and the package declares its `libc6` and `libgcc-s1` runtime dependencies. Docker, Git, `ss`, and `strace` remain optional because RunGlass reports collector availability and degrades gracefully.

## Install A Release

Download the `.deb` and its `.sha256` file from the corresponding GitHub release, verify the checksum, and install the package:

```bash
sha256sum --check runglass_0.3.3_amd64.deb.sha256
sudo apt install ./runglass_0.3.3_amd64.deb
runglass --version
man runglass
```

Remove it with:

```bash
sudo apt remove runglass
```

This removes packaged files but intentionally leaves user receipt data in the standard RunGlass data directory.

## Build And Inspect

On Debian or Ubuntu, build the release binary and package from the repository root:

```bash
cargo build --release --locked -p runglass
./packaging/debian/verify-package.sh 0.3.3 target/release/runglass
dpkg-deb --info dist/runglass_0.3.3_amd64.deb
dpkg-deb --contents dist/runglass_0.3.3_amd64.deb
```

The GitHub CI job performs the stronger end-to-end check on Ubuntu 22.04: build, package, install, version check, man-page rendering, package file inspection, removal, and post-removal file checks.
