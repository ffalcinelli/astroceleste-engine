#!/usr/bin/env bash
# Download the JPL kernels the engine is validated against into ./kernels (gitignored).
#   de440s.bsp  1849-2150, ~32 MB (default)
#   de441_part-1.bsp  -13200..1969, ~1.4 GB (pass --de441)
set -euo pipefail
cd "$(dirname "$0")/.."
BASE=https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp
mkdir -p kernels
# SHA-256 of the kernels the engine is validated against: a download or a cached copy
# that differs is an error.
DE440S_SHA256=c1c7feeab882263fc493a9d5a5b2ddd71b54826cdf65d8d17a76126b260a49f2
sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}
fetch() {
  if [ ! -f "kernels/$1" ]; then
    curl -fL --retry 3 -o "kernels/$1.part" "$BASE/$1"
    mv "kernels/$1.part" "kernels/$1"
  fi
  if [ -n "${2:-}" ] && [ "$(sha256 "kernels/$1")" != "$2" ]; then
    echo "kernels/$1: SHA-256 mismatch (expected $2); delete it and run this again" >&2
    exit 1
  fi
}
fetch de440s.bsp "$DE440S_SHA256"
if [ "${1:-}" = "--de441" ]; then fetch de441_part-1.bsp; fi
ls -la kernels
