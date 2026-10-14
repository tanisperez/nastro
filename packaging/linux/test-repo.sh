#!/usr/bin/env bash
# Tests repo.sh end to end with podman, no network except the image pulls. A throwaway key signs
# a repo built from the given packages inside an Ubuntu and a Fedora container, a local server
# publishes it, and clean Ubuntu and Fedora containers add it and install nastro.
#
#   packaging/linux/test-repo.sh <packages-dir>
#
# <packages-dir> holds one .deb and one .rpm (for example a release's assets).
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
packages=$(realpath "${1:?usage: test-repo.sh <packages-dir>}")
work=$(mktemp -d)
trap 'kill "${server:-0}" 2>/dev/null || true; rm -rf "$work"' EXIT
port=18099
url=http://127.0.0.1:$port

mkdir -p "$work/gnupg" "$work/site"
chmod 700 "$work/gnupg"
echo "test-passphrase" > "$work/pass"

# A key with a passphrase, to exercise the loopback pinentry path used in CI.
GNUPGHOME=$work/gnupg gpg --batch --pinentry-mode loopback --passphrase-file "$work/pass" \
    --quick-generate-key "Nastro test <test@example.com>" rsa3072 sign never
fpr=$(GNUPGHOME=$work/gnupg gpg --batch --list-keys --with-colons | awk -F: '/^fpr/ {print $10; exit}')

run_repo() { # image, then the setup commands, then the repo.sh subcommand
    local image=$1 setup=$2 what=$3
    podman run --rm -v "$here:/scripts:ro" -v "$packages:/packages:ro" -v "$work:/work" \
        -e GNUPGHOME=/work/gnupg -e GPG_KEY_ID="$fpr" -e GPG_PASSPHRASE_FILE=/work/pass \
        -e NASTRO_REPO_URL="$url" "$image" \
        bash -euc "$setup && /scripts/repo.sh $what /packages /work/site"
}

run_repo docker.io/library/ubuntu:24.04 \
    "apt-get update -qq && apt-get install -y -qq apt-utils gnupg >/dev/null" apt
run_repo docker.io/library/fedora:latest \
    "dnf install -y -q createrepo_c rpm-sign gnupg2 >/dev/null" rpm
run_repo docker.io/library/fedora:latest "true" key

python3 -m http.server "$port" --bind 127.0.0.1 --directory "$work/site" >/dev/null 2>&1 &
server=$!
sleep 1

echo "== Ubuntu: apt install"
podman run --rm --network host docker.io/library/ubuntu:24.04 bash -euc "
    apt-get update -qq && apt-get install -y -qq curl ca-certificates >/dev/null
    mkdir -p /etc/apt/keyrings
    curl -fsSLo /etc/apt/keyrings/nastro.asc $url/nastro.asc
    echo 'deb [signed-by=/etc/apt/keyrings/nastro.asc] $url/apt stable main' \
        > /etc/apt/sources.list.d/nastro.list
    apt-get update
    apt-get install -y nastro
    dpkg -s nastro | grep -E '^(Package|Version|Status)'
    test -x /usr/bin/nastro && echo 'ok: /usr/bin/nastro'"

echo "== Fedora: dnf install"
podman run --rm --network host docker.io/library/fedora:latest bash -euc "
    curl -fsSLo /etc/yum.repos.d/nastro.repo $url/rpm/nastro.repo
    dnf install -y nastro
    rpm -q nastro
    test -x /usr/bin/nastro && echo 'ok: /usr/bin/nastro'"

echo "== Both repos installed and verified signatures"
