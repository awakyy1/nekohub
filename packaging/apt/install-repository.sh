#!/bin/sh
set -eu

repository="https://awakyy1.github.io/nekohub"
keyring="/usr/share/keyrings/nekohub-archive-keyring.gpg"
source_list="/etc/apt/sources.list.d/nekohub.list"

test "$(id -u)" -eq 0 || {
  echo "Run this installer as root." >&2
  exit 1
}

command -v curl >/dev/null 2>&1 || {
  apt-get update
  apt-get install -y curl
}

curl --fail --silent --show-error \
  "$repository/nekohub-archive-keyring.gpg" \
  --output "$keyring"
printf '%s\n' \
  "deb [arch=amd64 signed-by=$keyring] $repository stable main" \
  > "$source_list"
apt-get update

echo "nekoHub repository configured. Install with: sudo apt install nekohub"
