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
Usage: prepare-package.sh VERSION [OUTPUT_DIR]

Downloads the published Linux release checksum, renders PKGBUILD, and
generates .SRCINFO. VERSION must be a published RunGlass version without a
leading v, for example 0.3.1.
USAGE
}

case "${1:-}" in
    -h|--help)
        usage
        exit 0
        ;;
esac

version=${1:-}
[ -n "$version" ] || die "VERSION is required"
case "$version" in
    *[!0-9.]*|.*|*.|*..*) die "invalid version: $version" ;;
esac

need_cmd awk
need_cmd curl
need_cmd makepkg
need_cmd mktemp
need_cmd sed

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
output_dir=${2:-"$script_dir/build/runglass-bin"}
asset="runglass-${version}-x86_64-unknown-linux-gnu.tar.gz"
checksum_url="https://github.com/error311/runglass/releases/download/v${version}/${asset}.sha256"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/runglass-arch-package.XXXXXX")
trap 'rm -rf "$tmp"' EXIT INT TERM

checksum=${RUNGLASS_ARCHIVE_SHA256:-}
if [ -z "$checksum" ]; then
    echo "Downloading $checksum_url"
    curl --connect-timeout 15 --max-time 120 --retry 3 --retry-all-errors -fsSL "$checksum_url" -o "$tmp/checksum"
    checksum=$(awk '{print $1; exit}' "$tmp/checksum")
else
    echo "Using RUNGLASS_ARCHIVE_SHA256 for v$version"
fi
case "$checksum" in
    ''|*[!0-9a-fA-F]*) die "release checksum is not a SHA-256 digest" ;;
esac
[ "${#checksum}" -eq 64 ] || die "release checksum is not 64 characters"

mkdir -p "$output_dir"
sed \
    -e "s/@PKGVER@/$version/g" \
    -e "s/@SHA256@/$checksum/g" \
    "$script_dir/PKGBUILD.in" > "$output_dir/PKGBUILD"

(
    cd "$output_dir"
    makepkg --printsrcinfo > .SRCINFO
)

cat <<EOF
Prepared Arch package metadata:
  $output_dir/PKGBUILD
  $output_dir/.SRCINFO

Build and inspect:
  cd "$output_dir"
  makepkg -sf
  pacman -Qlp runglass-bin-${version}-1-x86_64.pkg.tar.zst
EOF
