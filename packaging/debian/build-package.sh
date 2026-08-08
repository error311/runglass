#!/bin/sh
set -eu

die() {
    echo "error: $*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

usage() {
    cat <<'USAGE'
Usage: build-package.sh VERSION BINARY [OUTPUT_DIR]

Builds a Debian package from a release binary and the checked-in RunGlass
documentation. VERSION must not include a leading v.
USAGE
}

case "${1:-}" in
    -h|--help)
        usage
        exit 0
        ;;
esac

version=${1:-}
binary=${2:-}
[ -n "$version" ] || die "VERSION is required"
[ -n "$binary" ] || die "BINARY is required"
case "$version" in
    *[!0-9.]*|.*|*.|*..*) die "invalid version: $version" ;;
esac
[ -x "$binary" ] || die "binary is not executable: $binary"

need_cmd awk
need_cmd dpkg-deb
need_cmd du
need_cmd gzip
need_cmd install
need_cmd mktemp
need_cmd sha256sum

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/../.." && pwd)
output_dir=${3:-"$repo_root/dist"}
arch=${RUNGLASS_DEB_ARCH:-amd64}
case "$arch" in
    amd64|arm64) ;;
    *) die "unsupported Debian architecture: $arch" ;;
esac

actual_version=$("$binary" --version)
[ "$actual_version" = "runglass $version" ] || die "binary reports '$actual_version', expected 'runglass $version'"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/runglass-debian-package.XXXXXX")
trap 'rm -rf "$tmp"' EXIT INT TERM
package_root="$tmp/runglass_${version}_${arch}"

install -Dm755 "$binary" "$package_root/usr/bin/runglass"
install -Dm644 "$repo_root/README.md" "$package_root/usr/share/doc/runglass/README.md"
install -Dm644 "$repo_root/CHANGELOG.md" "$package_root/usr/share/doc/runglass/CHANGELOG.md"
install -Dm644 "$repo_root/LICENSE" "$package_root/usr/share/doc/runglass/copyright"

for page in "$repo_root"/docs/man/*.1; do
    name=$(basename "$page")
    mkdir -p "$package_root/usr/share/man/man1"
    gzip -9n -c "$page" > "$package_root/usr/share/man/man1/${name}.gz"
done

installed_size=$(du -sk "$package_root/usr" | awk '{print $1}')
mkdir -p "$package_root/DEBIAN"
cat > "$package_root/DEBIAN/control" <<EOF
Package: runglass
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Maintainer: Ryan <error311@gmail.com>
Installed-Size: $installed_size
Depends: libc6 (>= 2.35), libgcc-s1
Suggests: docker.io, git, iproute2, strace
Homepage: https://github.com/error311/runglass
Description: command receipts for local and CI workflows
 RunGlass runs one command and records output, file changes, process and
 network observations, Docker changes, timeline events, and risk notes in a
 local receipt. Live command observation is Linux-first.
EOF

mkdir -p "$output_dir"
output_dir=$(CDPATH='' cd -- "$output_dir" && pwd)
package_name="runglass_${version}_${arch}.deb"
package_file="$output_dir/$package_name"
dpkg-deb --root-owner-group --build "$package_root" "$package_file"
(
    cd "$output_dir"
    sha256sum "$package_name" > "$package_name.sha256"
)

echo "Built Debian package:"
echo "  $package_file"
echo "  $package_file.sha256"
