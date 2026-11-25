#!/bin/sh
# Installs TotalFloat on Linux (x86-64) from its GitHub release:
#
#   curl -fsSL https://raw.githubusercontent.com/ijd5004/totalfloat/main/install/install.sh | sh
#
# TOTALFLOAT_VERSION picks a release, such as v0.1.0; the default is the latest.
# TOTALFLOAT_INSTALL_DIR picks the folder; the default is ~/.local/bin.
# Run it again to upgrade. See docs/guide/install.md.

set -eu

releases="https://github.com/ijd5004/totalfloat/releases"
start_here="https://ijd5004.github.io/totalfloat/guide/start-here.html"
from_source="https://github.com/ijd5004/totalfloat/blob/main/docs/guide/install.md#build-from-source"

say() { printf 'totalfloat-install: %s\n' "$*"; }
die() { printf 'totalfloat-install: error: %s\n' "$*" >&2; exit 1; }

# Downloads $1 to the file $2.
fetch() {
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL -o "$2" "$1"
    else
        wget -q -O "$2" "$1"
    fi
}

# Prints the tag of the latest release, read from where releases/latest redirects.
latest_tag() {
    if command -v curl > /dev/null 2>&1; then
        url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$releases/latest") || return 1
    else
        url=$(wget -S --spider "$releases/latest" 2>&1 | sed -n 's/^ *[Ll]ocation: *\([^ ]*\).*/\1/p' | tail -n 1)
    fi
    url=$(printf '%s' "$url" | tr -d '\r')
    printf '%s\n' "${url##*/}"
}

sha256() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

main() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os/$arch" in
        Linux/x86_64 | Linux/amd64) target=x86_64-unknown-linux-gnu ;;
        *) die "there's no prebuilt TotalFloat for $os on $arch. To build it from source, see $from_source" ;;
    esac

    command -v curl > /dev/null 2>&1 || command -v wget > /dev/null 2>&1 \
        || die "this needs curl or wget to download TotalFloat"
    command -v sha256sum > /dev/null 2>&1 || command -v shasum > /dev/null 2>&1 \
        || die "this needs sha256sum or shasum to check the download"

    tag=${TOTALFLOAT_VERSION:-}
    if [ -z "$tag" ]; then
        tag=$(latest_tag) || tag=
        [ -n "$tag" ] || die "couldn't find the latest release at $releases/latest"
    fi
    case "$tag" in
        v[0-9]*) ;;
        [0-9]*) tag="v$tag" ;;
        *) die "'$tag' isn't a release version, such as v0.1.0" ;;
    esac

    archive="totalfloat-$tag-$target.tar.gz"
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    trap 'exit 1' INT TERM

    say "downloading $archive"
    fetch "$releases/download/$tag/$archive" "$tmp/$archive" \
        || die "couldn't download $releases/download/$tag/$archive"
    fetch "$releases/download/$tag/SHA256SUMS" "$tmp/SHA256SUMS" \
        || die "couldn't download $releases/download/$tag/SHA256SUMS"

    expected=$(awk -v f="$archive" '$2 == f || $2 == "*" f { print $1; exit }' "$tmp/SHA256SUMS")
    [ -n "$expected" ] || die "SHA256SUMS has no line for $archive. Nothing was installed."
    actual=$(sha256 "$tmp/$archive")
    if [ "$actual" != "$expected" ]; then
        die "$archive doesn't match SHA256SUMS (expected $expected, got $actual). Nothing was installed."
    fi
    say "checksum matches SHA256SUMS"

    mkdir "$tmp/unpacked"
    tar -xzf "$tmp/$archive" -C "$tmp/unpacked"
    # The archive holds the binary in a top-level folder or at its root.
    bin=$(find "$tmp/unpacked" -type f -name totalfloat | head -n 1)
    [ -n "$bin" ] || die "$archive holds no totalfloat binary. Nothing was installed."

    dir=${TOTALFLOAT_INSTALL_DIR:-$HOME/.local/bin}
    mkdir -p "$dir"
    # Copy beside the old binary, then rename over it, so an upgrade works even
    # while TotalFloat is running.
    cp "$bin" "$dir/.totalfloat.new"
    chmod 755 "$dir/.totalfloat.new"
    mv -f "$dir/.totalfloat.new" "$dir/totalfloat"
    say "installed $dir/totalfloat"

    "$dir/totalfloat" --version

    case ":$PATH:" in
        *":$dir:"*) ;;
        *)
            say "$dir isn't on your PATH. Add this line to your shell profile, such as ~/.bashrc, ~/.zshrc or ~/.profile, then open a new terminal:"
            # shellcheck disable=SC2016 # $PATH is for the profile to expand.
            printf '\n    export PATH="%s:$PATH"\n\n' "$dir"
            ;;
    esac
    say "next, open the Start here page: $start_here"
}

main "$@"
