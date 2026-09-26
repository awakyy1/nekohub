#!/bin/sh
set -eu

repository="${1:?usage: build-repository.sh REPOSITORY_DIR DEB_FILE...}"
shift
test "$#" -gt 0
source_dir="${NEKOHUB_SOURCE_DIR:-$(pwd)}"
repository="$(cd "$repository" && pwd)"
pool="$repository/pool/main/n/nekohub"
distribution="$repository/dists/stable"
packages="$distribution/main/binary-amd64/Packages"

mkdir -p "$pool" "$(dirname "$packages")"
for package in "$@"; do
  package="$(cd "$(dirname "$package")" && pwd)/$(basename "$package")"
  cp "$package" "$pool/"
done

cd "$repository"
dpkg-scanpackages --multiversion pool /dev/null > "$packages"
gzip -9 -c "$packages" > "$packages.gz"
apt-ftparchive \
  -c "$source_dir/packaging/apt/release.conf" \
  release dists/stable > "$distribution/Release"
gpg --batch --yes --armor --detach-sign \
  --output "$distribution/Release.gpg" "$distribution/Release"
gpg --batch --yes --clearsign \
  --output "$distribution/InRelease" "$distribution/Release"
gpg --batch --yes --armor \
  --output nekohub-archive-key.asc --export "nekoHub APT Repository"
gpg --batch --yes \
  --output nekohub-archive-keyring.gpg --export "nekoHub APT Repository"
