#!/bin/sh
set -eu

usage() {
    cat <<'USAGE'
Usage:
  install-release.sh [release-dir|release.tar.gz|release-url]

Environment:
  PREFIX  Install prefix. Defaults to $HOME/.local.

Examples:
  ./scripts/install-release.sh
  PREFIX=/tmp/runglass-install ./scripts/install-release.sh ./runglass-0.3.1-x86_64-unknown-linux-gnu.tar.gz
  PREFIX="$HOME/.local" ./scripts/install-release.sh https://github.com/error311/runglass/releases/download/v0.3.1/runglass-0.3.1-x86_64-unknown-linux-gnu.tar.gz
USAGE
}

die() {
    echo "error: $*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

checksum_file() {
    path=$1
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$path" | awk '{print $1}'
    else
        die "missing sha256sum or shasum for checksum verification"
    fi
}

verify_checksum() {
    archive=$1
    checksum=$2
    expected=$(awk '{print $1; exit}' "$checksum")
    [ -n "$expected" ] || die "checksum file is empty: $checksum"
    actual=$(checksum_file "$archive")
    [ "$expected" = "$actual" ] || die "checksum mismatch for $archive"
    echo "Verified checksum: $checksum"
}

download() {
    url=$1
    out=$2
    need_cmd curl
    curl -fsSL "$url" -o "$out"
}

find_release_root() {
    root=$1
    [ -x "$root/runglass" ] && {
        printf '%s\n' "$root"
        return
    }

    found=$(find "$root" -maxdepth 3 -type f -name runglass -perm -111 | head -n 1)
    [ -n "$found" ] || die "could not find runglass binary under $root"
    dirname "$found"
}

install_from_root() {
    root=$(find_release_root "$1")
    prefix=${PREFIX:-"$HOME/.local"}
    bin_dir="$prefix/bin"
    man_dir="$prefix/share/man/man1"
    doc_dir="$prefix/share/doc/runglass"

    install -d "$bin_dir" "$man_dir" "$doc_dir"
    install -m 755 "$root/runglass" "$bin_dir/runglass"

    if [ -d "$root/share/man/man1" ]; then
        for page in "$root"/share/man/man1/*.1; do
            [ -e "$page" ] || continue
            install -m 644 "$page" "$man_dir/"
        done
    else
        echo "warning: no man pages found at $root/share/man/man1" >&2
    fi

    for doc in README.md CHANGELOG.md LICENSE; do
        if [ -f "$root/$doc" ]; then
            install -m 644 "$root/$doc" "$doc_dir/"
        fi
    done

    cat <<EOF
Installed RunGlass
  binary:    $bin_dir/runglass
  man pages: $man_dir
  docs:      $doc_dir

Try:
  PATH="$bin_dir:\$PATH" runglass --version
  MANPATH="$prefix/share/man:\${MANPATH:-}" man runglass

Uninstall:
  rm -f "$bin_dir/runglass"
  rm -f "$man_dir"/runglass*.1
  rm -rf "$doc_dir"
EOF
}

main() {
    case "${1:-}" in
        -h|--help)
            usage
            exit 0
            ;;
    esac

    need_cmd install
    need_cmd tar
    need_cmd find
    need_cmd awk

    source=${1:-}
    tmp=${TMPDIR:-/tmp}/runglass-install.$$
    trap 'rm -rf "$tmp"' EXIT INT TERM

    if [ -z "$source" ]; then
        script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
        install_from_root "$(dirname "$script_dir")"
        exit 0
    fi

    case "$source" in
        http://*|https://*)
            mkdir -p "$tmp"
            archive="$tmp/runglass.tar.gz"
            checksum="$archive.sha256"
            echo "Downloading $source"
            download "$source" "$archive"
            if download "$source.sha256" "$checksum" 2>/dev/null; then
                verify_checksum "$archive" "$checksum"
            else
                echo "warning: checksum file not available at $source.sha256" >&2
            fi
            ;;
        *.tar.gz|*.tgz)
            archive=$source
            [ -f "$archive" ] || die "archive not found: $archive"
            if [ -f "$archive.sha256" ]; then
                verify_checksum "$archive" "$archive.sha256"
            elif [ -f "$(dirname "$archive")/$(basename "$archive").sha256" ]; then
                verify_checksum "$archive" "$(dirname "$archive")/$(basename "$archive").sha256"
            else
                echo "warning: no checksum file found for $archive" >&2
            fi
            ;;
        *)
            [ -d "$source" ] || die "source must be a directory, .tar.gz archive, or URL"
            install_from_root "$source"
            exit 0
            ;;
    esac

    extract_dir="$tmp/extract"
    mkdir -p "$extract_dir"
    tar -xzf "$archive" -C "$extract_dir"
    install_from_root "$extract_dir"
}

main "$@"
