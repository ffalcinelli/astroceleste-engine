#!/usr/bin/env bash
# Download the JPL kernels the engine is validated against into ./kernels (gitignored).
#   de440s.bsp  1849-2150, ~32 MB (default)
#   de441_part-1.bsp  -13200..1969, ~1.4 GB (pass --de441)
set -euo pipefail
cd "$(dirname "$0")/.."
BASE=https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp
mkdir -p kernels
fetch() {
  if [ -f "kernels/$1" ]; then return; fi
  curl -fL --retry 3 -o "kernels/$1.part" "$BASE/$1"
  mv "kernels/$1.part" "kernels/$1"
}
fetch de440s.bsp
if [ "${1:-}" = "--de441" ]; then fetch de441_part-1.bsp; fi
ls -la kernels
