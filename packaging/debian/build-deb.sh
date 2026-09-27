#!/bin/sh
set -eu

version="${NEKOHUB_VERSION:-0.5.1}"
architecture="${NEKOHUB_ARCHITECTURE:-amd64}"
binary="${NEKOHUB_BINARY:-target/release/nekohub}"
agent_binary="${NEKOHUB_AGENT_BINARY:-target/release/nekohub-agent}"
output_dir="${NEKOHUB_OUTPUT_DIR:-dist}"
root="target/debian/nekohub-package"
agent_root="target/debian/nekohub-agent-package"

test -x "$binary"
test -x "$agent_binary"
rm -rf "$root" "$agent_root"
mkdir -p "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/doc/nekohub"
sed \
  -e "s/^Version: .*/Version: $version/" \
  -e "s/^Architecture: .*/Architecture: $architecture/" \
  -e "s/nekohub-agent (= [^)]*)/nekohub-agent (= $version)/" \
  packaging/debian/control > "$root/DEBIAN/control"
install -m 0755 "$binary" "$root/usr/bin/nekohub"
install -m 0644 README.md "$root/usr/share/doc/nekohub/README.md"
install -m 0644 LICENSE-MIT "$root/usr/share/doc/nekohub/copyright"

mkdir -p "$agent_root/DEBIAN" "$agent_root/usr/bin" \
  "$agent_root/usr/lib/systemd/system" "$agent_root/usr/share/doc/nekohub-agent"
sed \
  -e "s/^Version: .*/Version: $version/" \
  -e "s/^Architecture: .*/Architecture: $architecture/" \
  packaging/debian/control-agent > "$agent_root/DEBIAN/control"
install -m 0755 "$agent_binary" "$agent_root/usr/bin/nekohub-agent"
install -m 0644 packaging/debian/nekohub-agent.service \
  "$agent_root/usr/lib/systemd/system/nekohub-agent.service"
install -m 0755 packaging/debian/agent-postinst "$agent_root/DEBIAN/postinst"
install -m 0755 packaging/debian/agent-prerm "$agent_root/DEBIAN/prerm"
install -m 0755 packaging/debian/agent-postrm "$agent_root/DEBIAN/postrm"
install -m 0644 README.md "$agent_root/usr/share/doc/nekohub-agent/README.md"
install -m 0644 LICENSE-MIT "$agent_root/usr/share/doc/nekohub-agent/copyright"

mkdir -p "$output_dir"
dpkg-deb --root-owner-group --build "$agent_root" \
  "$output_dir/nekohub-agent_${version}_${architecture}.deb"
dpkg-deb --root-owner-group --build "$root" \
  "$output_dir/nekohub_${version}_${architecture}.deb"
