#!/bin/sh
# Prints the Homebrew formula for one release:  sh .github/formula.sh <version> <dir>
# <dir> holds the release's recdr-<target>.tar.gz.sha256 files.
set -eu

version=$1
dir=$2
desc=$(sed -n 's/^description = "\(.*\)"/\1/p' Cargo.toml | head -n1)
url="https://github.com/dzikrisyairozi/recdr/releases/download/v$version"

# Prints the url and sha256 lines for one target, indented for the formula.
entry() {
  file="$dir/recdr-$1.tar.gz.sha256"
  [ -f "$file" ] || { echo "formula.sh: $file is missing" >&2; exit 1; }
  sum=$(cut -d' ' -f1 "$file")
  printf '      url "%s/recdr-%s.tar.gz"\n      sha256 "%s"\n' "$url" "$1" "$sum"
}

mac_arm=$(entry aarch64-apple-darwin)
mac_intel=$(entry x86_64-apple-darwin)
linux_arm=$(entry aarch64-unknown-linux-musl)
linux_intel=$(entry x86_64-unknown-linux-musl)

cat <<FORMULA
class Recdr < Formula
  desc "$desc"
  homepage "https://github.com/dzikrisyairozi/recdr"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
$mac_arm
    end
    on_intel do
$mac_intel
    end
  end

  on_linux do
    on_arm do
$linux_arm
    end
    on_intel do
$linux_intel
    end
  end

  def install
    bin.install "recdr"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/recdr --version")
  end
end
FORMULA
