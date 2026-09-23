#!/usr/bin/env bash
# Build the landing site with its live demo into target/site (deployed by pages.yml):
#   site/*                          static pages
#   pkg/                            the WebAssembly package (wasm-pack --target web)
#   ephemeris/de440s-1950-2050.bsp  a DE440s excerpt for the demo (~11 MB)
# Needs wasm-pack and the wasm32-unknown-unknown target. Preview locally with
#   python3 -m http.server -d target/site
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="$PWD/target/site"
# 1950-01-01 to 2050-01-01, TDB Julian dates. Keep in sync with site/demo.js.
START_JD=2433282.5
END_JD=2469807.5

rm -rf "$OUT"
mkdir -p "$OUT/ephemeris"
cp -R site/. "$OUT/"

scripts/fetch-kernels.sh >/dev/null
cargo run --release --quiet -p astroceleste-engine --example excerpt -- \
  kernels/de440s.bsp "$OUT/ephemeris/de440s-1950-2050.bsp" "$START_JD" "$END_JD"

wasm-pack build --release --target web --out-dir "$OUT/pkg" crates/astroceleste-engine-wasm
# Only the module and its bindings are served.
rm -f "$OUT/pkg/.gitignore" "$OUT/pkg/package.json" "$OUT/pkg/README.md" \
  "$OUT/pkg"/LICENSE-* "$OUT/pkg/NOTICE"

du -h "$OUT"/ephemeris/*.bsp "$OUT"/pkg/*.wasm
du -sh "$OUT"
