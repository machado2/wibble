#!/usr/bin/env bash
set -Eeuo pipefail

version=1.17.0
download_url="https://github.com/nickel-lang/nickel/releases/download/${version}/nickel-x86_64-linux"
expected_sha256=afcdfa6e0fff31760cf229e85997456c02c00b8b3b84ff38f897ac7b3f39ae34
target=/home/fabio/.local/bin/nickel

if [[ -x "${target}" ]] && "${target}" --version | grep -Fq "nickel-lang-cli nickel ${version}"; then
  exit 0
fi

install -d -m 0755 -o fabio -g fabio "$(dirname "${target}")"
temporary_dir=$(mktemp -d /var/tmp/wibble-nickel.XXXXXX)
cleanup() {
  case "${temporary_dir}" in
    /var/tmp/wibble-nickel.*) rm -rf -- "${temporary_dir}" ;;
  esac
}
trap cleanup EXIT

curl --fail --location --silent --show-error \
  --output "${temporary_dir}/nickel" \
  "${download_url}"
printf '%s  %s\n' "${expected_sha256}" "${temporary_dir}/nickel" \
  | sha256sum --check --status
install -m 0755 -o fabio -g fabio "${temporary_dir}/nickel" "${target}"
"${target}" --version
