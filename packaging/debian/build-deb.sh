#!/bin/sh
set -eu

version="${NEKOHUB_VERSION:-0.1.0}"
architecture="${NEKOHUB_ARCHITECTURE:-amd64}"
binary="${NEKOHUB_BINARY:-target/release/nekohub}"
output="${NEKOHUB_OUTPUT:-dist/nekohub_${version}_${architecture}.deb}"
root="target/debian/nekohub"

test -x "$binary"
rm -rf "$root"
mkdir -p "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/doc/nekohub"
sed \
  -e "s/^Version: .*/Version: $version/" \
  -e "s/^Architecture: .*/Architecture: $architecture/" \
  packaging/debian/control > "$root/DEBIAN/control"
install -m 0755 "$binary" "$root/usr/bin/nekohub"
install -m 0644 README.md "$root/usr/share/doc/nekohub/README.md"
install -m 0644 LICENSE-MIT "$root/usr/share/doc/nekohub/copyright"
mkdir -p "$(dirname "$output")"
dpkg-deb --root-owner-group --build "$root" "$output"
