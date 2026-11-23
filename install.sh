#!/bin/sh
# Installs recdr on macOS or Linux:  curl -fsSL https://recdr.dev/install.sh | sh
#
# RECDR_VERSION       install this version (0.1.0 or v0.1.0) instead of the newest
# RECDR_INSTALL_DIR   where the binary goes (default ~/.local/bin)
# RECDR_RELEASE_BASE  where releases are read from (default GitHub releases)
#
# `recdr update` runs this script too, with all three set.
set -eu

fail() { printf 'recdr install: %s\n' "$1" >&2; exit 1; }

base=${RECDR_RELEASE_BASE:-https://github.com/dzikrisyairozi/recdr/releases}
base=${base%/}
dir=${RECDR_INSTALL_DIR:-$HOME/.local/bin}

os=$(uname -s)
arch=$(uname -m)
case "$arch" in
  x86_64 | amd64) cpu=x86_64 ;;
  aarch64 | arm64) cpu=aarch64 ;;
  *) fail "no recdr build for $os $arch" ;;
esac
case "$os" in
  Linux) target="$cpu-unknown-linux-musl" ;;
  Darwin)
    # A shell under Rosetta reports x86_64 on an Apple Silicon Mac.
    if [ "$cpu" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
      cpu=aarch64
    fi
    target="$cpu-apple-darwin"
    ;;
  *) fail "no recdr build for $os $arch" ;;
esac

command -v curl >/dev/null || fail "curl was not found"
command -v tar >/dev/null || fail "tar was not found"
if command -v sha256sum >/dev/null; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null; then
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  fail "no sha256sum or shasum found"
fi

if [ -n "${RECDR_VERSION:-}" ]; then
  version=${RECDR_VERSION#v}
else
  text=$(curl -fsSL "$base/latest/download/version.txt") ||
    fail "could not read $base/latest/download/version.txt"
  version=$(printf '%s\n' "$text" | head -n1 | tr -d '\r')
fi
[ -n "$version" ] || fail "$base/latest/download/version.txt is empty"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/recdr-install.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

archive="recdr-$target.tar.gz"
url="$base/download/v$version/$archive"
curl -fsSL -o "$tmp/$archive" "$url" || fail "could not download $url"
curl -fsSL -o "$tmp/$archive.sha256" "$url.sha256" || fail "could not download $url.sha256"

expected=$(cut -d' ' -f1 "$tmp/$archive.sha256" | tr 'A-F' 'a-f')
actual=$(sha256 "$tmp/$archive")
[ "$expected" = "$actual" ] || fail "checksum of $archive does not match; nothing was installed"

mkdir "$tmp/unpacked"
tar -xzf "$tmp/$archive" -C "$tmp/unpacked" || fail "could not unpack $archive"
[ -f "$tmp/unpacked/recdr" ] || fail "$archive holds no recdr binary"

mkdir -p "$dir" || fail "could not create $dir"
rm -f "$dir/recdr.old"
# Renaming first leaves a running recdr untouched; the new file takes the name.
if [ -e "$dir/recdr" ]; then
  mv "$dir/recdr" "$dir/recdr.old" || fail "could not move the old $dir/recdr aside"
fi
if ! mv "$tmp/unpacked/recdr" "$dir/recdr"; then
  [ -e "$dir/recdr.old" ] && mv "$dir/recdr.old" "$dir/recdr"
  fail "could not write $dir/recdr"
fi
chmod 755 "$dir/recdr"
rm -f "$dir/recdr.old"

echo "installed recdr $version to $dir/recdr"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "$dir is not on your PATH; add it to use recdr by name" ;;
esac
if [ "$os" = Darwin ]; then
  major=$(sw_vers -productVersion 2>/dev/null | cut -d. -f1)
  if [ -n "$major" ] && [ "$major" -lt 14 ]; then
    echo "desktop recording needs macOS 14; tab recording works"
  fi
  echo "the first desktop recording asks for the Screen Recording permission, for your terminal"
fi
if [ "$os" = Linux ]; then
  command -v ffmpeg >/dev/null || echo "ffmpeg is not on PATH; MP4 needs it"
  echo "on Linux recdr records browser tabs only"
fi
echo "claude mcp add recdr -- recdr mcp"
