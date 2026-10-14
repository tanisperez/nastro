#!/usr/bin/env bash
# Builds the signed APT and DNF repositories published on the website.
#
#   packaging/linux/repo.sh <apt|rpm|key> <packages-dir> <site-dir>
#
#   apt   needs apt-ftparchive (apt-utils) and gpg. Writes <site-dir>/apt.
#   rpm   needs createrepo_c, rpmsign (rpm-sign) and gpg. Writes <site-dir>/rpm.
#   key   writes the public key to <site-dir>/nastro.asc (shared by both repos).
#
# <packages-dir> holds the .deb and .rpm files to publish (any number of versions).
#
# Environment:
#   GPG_KEY_ID           fingerprint or id of the signing key, already in the keyring (required)
#   GPG_PASSPHRASE_FILE  file with the key passphrase (optional, for a protected key)
#   NASTRO_REPO_URL      public base URL, default https://nastro.tanis.codes (provisional),
#                        written into nastro.repo, tests point it to a local server
set -euo pipefail

usage() {
    sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
}

[ $# -eq 3 ] || usage
what=$1
packages=$(realpath "$2")
mkdir -p "$3"
site=$(realpath "$3")
: "${GPG_KEY_ID:?GPG_KEY_ID is not set}"
# Provisional domain, a pending decision (AGENTS.md).
base_url=${NASTRO_REPO_URL:-https://nastro.tanis.codes}

# gpg with the key, non interactive, passphrase from a file when there is one.
gpg_args=(--batch --yes --local-user "$GPG_KEY_ID")
if [ -n "${GPG_PASSPHRASE_FILE:-}" ]; then
    gpg_args+=(--pinentry-mode loopback --passphrase-file "$GPG_PASSPHRASE_FILE")
fi

build_apt() {
    local repo=$site/apt dist=$site/apt/dists/stable
    local arch_dir=$dist/main/binary-amd64
    rm -rf "$repo"
    mkdir -p "$repo/pool/main" "$arch_dir"
    cp "$packages"/*.deb "$repo/pool/main/"

    (cd "$repo" && apt-ftparchive packages pool > "$arch_dir/Packages")
    gzip -9kf "$arch_dir/Packages"

    (cd "$repo" && apt-ftparchive \
        -o APT::FTPArchive::Release::Origin=Nastro \
        -o APT::FTPArchive::Release::Label=Nastro \
        -o APT::FTPArchive::Release::Suite=stable \
        -o APT::FTPArchive::Release::Codename=stable \
        -o APT::FTPArchive::Release::Architectures=amd64 \
        -o APT::FTPArchive::Release::Components=main \
        release dists/stable > "$dist/Release")

    gpg "${gpg_args[@]}" --armor --detach-sign --output "$dist/Release.gpg" "$dist/Release"
    gpg "${gpg_args[@]}" --clearsign --output "$dist/InRelease" "$dist/Release"
}

build_rpm() {
    local repo=$site/rpm
    rm -rf "$repo"
    mkdir -p "$repo"
    cp "$packages"/*.rpm "$repo/"

    # rpmsign runs gpg itself: tell it which key, and feed the passphrase through a wrapper set as
    # %__gpg. The wrapper drops a leading "gpg" because older rpm passes it as argv[0] text and
    # rpm 6 does not.
    local macros wrapper
    macros=$(mktemp)
    wrapper=$(mktemp)
    {
        echo '#!/bin/sh'
        echo '[ "$1" = gpg ] && shift'
        if [ -n "${GPG_PASSPHRASE_FILE:-}" ]; then
            echo "exec gpg --pinentry-mode loopback --passphrase-file '$GPG_PASSPHRASE_FILE' \"\$@\""
        else
            echo 'exec gpg "$@"'
        fi
    } > "$wrapper"
    chmod +x "$wrapper"
    {
        echo "%_gpg_name $GPG_KEY_ID"
        echo "%__gpg $wrapper"
    } > "$macros"
    for rpm_file in "$repo"/*.rpm; do
        rpmsign --macros="/usr/lib/rpm/macros:$macros" --addsign "$rpm_file"
    done
    rm -f "$macros" "$wrapper"

    createrepo_c --quiet "$repo"
    gpg "${gpg_args[@]}" --armor --detach-sign --output "$repo/repodata/repomd.xml.asc" \
        "$repo/repodata/repomd.xml"

    cat > "$repo/nastro.repo" <<REPO
[nastro]
name=Nastro
baseurl=$base_url/rpm
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=$base_url/nastro.asc
REPO
}

case $what in
    apt) build_apt ;;
    rpm) build_rpm ;;
    key) gpg --batch --armor --export "$GPG_KEY_ID" > "$site/nastro.asc" ;;
    *) usage ;;
esac
