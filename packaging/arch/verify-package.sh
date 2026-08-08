#!/bin/sh
set -eu

die() {
    echo "error: $*" >&2
    exit 1
}

version=${1:-}
[ -n "$version" ] || die "usage: verify-package.sh VERSION"

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
package_dir="$script_dir/build/runglass-bin"

"$script_dir/prepare-package.sh" "$version" "$package_dir"

(
    cd "$package_dir"
    makepkg --cleanbuild --force --syncdeps --noconfirm
)

package_file="$package_dir/runglass-bin-${version}-1-x86_64.pkg.tar.zst"
[ -f "$package_file" ] || die "makepkg did not produce $package_file"

file_list=$(pacman -Qlp "$package_file")
printf '%s\n' "$file_list"

for path in \
    usr/bin/runglass \
    usr/share/licenses/runglass-bin/LICENSE \
    usr/share/man/man1/runglass.1.gz \
    usr/share/doc/runglass/README.md \
    usr/share/doc/runglass/CHANGELOG.md
do
    printf '%s\n' "$file_list" | grep -Fq "$path" || die "package is missing $path"
done

release_binary="$package_dir/src/runglass-${version}-$(uname -m)-unknown-linux-gnu/runglass"
"$release_binary" --version | grep -F "runglass $version"

echo "Verified package: $package_file"
