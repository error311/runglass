# Arch Linux And AUR Packaging

RunGlass uses the `runglass-bin` package name because this recipe installs the prebuilt, checksum-pinned Linux release archive. Arch packaging rules reserve the plain `runglass` name for a package built from source.

The package installs:

- `/usr/bin/runglass`
- `/usr/share/man/man1/runglass*.1`
- `/usr/share/licenses/runglass-bin/LICENSE`
- `/usr/share/doc/runglass/README.md`
- `/usr/share/doc/runglass/CHANGELOG.md`

`docker`, `git`, `iproute2`, and `strace` are optional dependencies because RunGlass degrades gracefully when their corresponding collectors are unavailable.

## Prepare And Verify

Run this from the repository root after the GitHub release assets for the version exist:

```bash
./packaging/arch/prepare-package.sh 0.3.2
cd packaging/arch/build/runglass-bin
makepkg -sf
pacman -Qlp runglass-bin-0.3.2-1-x86_64.pkg.tar.zst
```

For the complete package-content smoke test:

```bash
./packaging/arch/verify-package.sh 0.3.2
```

The preparation script downloads the release's published SHA-256 file, renders `PKGBUILD` from `PKGBUILD.in`, and generates `.SRCINFO` with `makepkg`. Generated package files remain under the ignored `packaging/arch/build/` directory.

For an offline or release-workflow invocation that has already verified the checksum, set `RUNGLASS_ARCHIVE_SHA256` to the 64-character archive digest. The generated recipe still pins that checksum normally.

To install the locally built package:

```bash
sudo pacman -U packaging/arch/build/runglass-bin/runglass-bin-0.3.2-1-x86_64.pkg.tar.zst
runglass --version
man runglass
```

## First AUR Publication

Do the first publication only after the corresponding GitHub release exists and the local verification above passes.

1. Create or sign in to an AUR account.
2. Create a dedicated SSH key with `ssh-keygen -t ed25519 -f ~/.ssh/runglass-aur`.
3. Add the public key from `~/.ssh/runglass-aur.pub` to the AUR account.
4. Add the private key to the GitHub repository as an Actions secret named `AUR_SSH_PRIVATE_KEY`.
5. Run the `Publish AUR Package` workflow with the released version and leave `publish` disabled for the first verification run.
6. Inspect the uploaded `runglass-bin-aur-metadata` workflow artifact.
7. Run the workflow again with `publish` enabled.

The workflow will clone `ssh://aur@aur.archlinux.org/runglass-bin.git`, replace only `PKGBUILD` and `.SRCINFO`, commit the version update, and push to the AUR `master` branch. The private key is never accepted by RunGlass itself or placed in package/release artifacts.

## Updating

For each later RunGlass release:

1. Publish and verify the GitHub release archive.
2. Run `verify-package.sh VERSION` locally.
3. Dispatch the AUR workflow once without publication.
4. Inspect its generated metadata artifact.
5. Dispatch it with publication enabled.

Automatic publication directly from the GitHub release event is intentionally deferred until the package has been maintained successfully through several releases. This keeps a release-workflow failure or incorrect upstream asset from immediately updating AUR users.
