#!/bin/sh
set -eu

die() {
    echo "error: $*" >&2
    exit 1
}

version=${1:-}
binary=${2:-}
[ -n "$version" ] || die "usage: verify-package.sh VERSION BINARY [OUTPUT_DIR]"
[ -n "$binary" ] || die "usage: verify-package.sh VERSION BINARY [OUTPUT_DIR]"

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/../.." && pwd)
output_dir=${3:-"$repo_root/dist"}
arch=${RUNGLASS_DEB_ARCH:-amd64}
package_file="$output_dir/runglass_${version}_${arch}.deb"

"$script_dir/build-package.sh" "$version" "$binary" "$output_dir"

[ "$(dpkg-deb -f "$package_file" Package)" = "runglass" ] || die "incorrect package name"
[ "$(dpkg-deb -f "$package_file" Version)" = "$version" ] || die "incorrect package version"
[ "$(dpkg-deb -f "$package_file" Architecture)" = "$arch" ] || die "incorrect architecture"

contents=$(dpkg-deb --contents "$package_file")
printf '%s\n' "$contents"

for path in \
    ./usr/bin/runglass \
    ./usr/share/doc/runglass/README.md \
    ./usr/share/doc/runglass/CHANGELOG.md \
    ./usr/share/doc/runglass/copyright \
    ./usr/share/man/man1/runglass.1.gz \
    ./usr/share/man/man1/runglass-run.1.gz
do
    printf '%s\n' "$contents" | grep -Fq "$path" || die "package is missing $path"
done

echo "Verified Debian package: $package_file"
