# astroceleste-engine

[![crates.io](https://img.shields.io/crates/v/astroceleste-engine.svg)](https://crates.io/crates/astroceleste-engine)
[![docs.rs](https://img.shields.io/docsrs/astroceleste-engine)](https://docs.rs/astroceleste-engine)
[![PyPI](https://img.shields.io/pypi/v/astroceleste-engine.svg)](https://pypi.org/project/astroceleste-engine/)
[![npm](https://img.shields.io/npm/v/astroceleste-engine.svg)](https://www.npmjs.com/package/astroceleste-engine)
[![CI](https://github.com/ffalcinelli/astroceleste-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/ffalcinelli/astroceleste-engine/actions/workflows/ci.yml)

Astrological chart calculation on JPL ephemerides, in pure Rust.

It is the calculation core of [Astroceleste](https://astroceleste.it). The same code runs
on the server (Python bindings), in the desktop and mobile apps (native) and in the browser
(WASM), so every platform computes the same chart down to the arcsecond.

> **Experimental (0.0.x):** the API may change in any release. Pin an exact version.

> Status: the complete chart pipeline is ported and matches the reference implementation
> on every golden chart: planets, lunar nodes, Chiron, Lilith, houses (Placidus, Whole Sign,
> Equal, Porphyry), aspects and orbs, fixed stars, Arabic parts, temperament, lunar status,
> horary charts (planetary hours from computed sunrise and sunset), transits, synastry and
> derived charts, with Python and WebAssembly bindings.

## Usage

```rust
use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
use astroceleste_engine::{calculate_chart, ChartRequest, UtcInstant};

let mut kernels = KernelSet::new();
kernels.push(Kernel::new("de440s.bsp", Spk::open("kernels/de440s.bsp")?)?);

let request = ChartRequest::new(UtcInstant::parse("1987-05-17T14:30:00Z")?, 41.9, 12.5);
let chart = calculate_chart(&kernels, &request)?;
println!("{}", serde_json::to_string_pretty(&chart)?);
```

Charts serialize to the same JSON as the Astroceleste API. Other entry points:
`calculate_horary_chart`, `calculate_transit_chart`, `calculate_synastry` and
`calculate_derived_chart`. In the browser or on mobile, load the kernel from memory with
`Spk::from_bytes`.

## Bindings

- **Python** (`pip install astroceleste-engine`, CPython ≥ 3.12): `Engine([kernel paths])`
  with `.chart()`, `.horary()`, `.transit()`, plus `synastry()` and `derived_chart()`,
  returning plain dicts. See [crates/astroceleste-engine-py](crates/astroceleste-engine-py).
- **JavaScript / WebAssembly** (`npm install astroceleste-engine`): the same API over
  kernels loaded from memory, for browsers and Node. See
  [crates/astroceleste-engine-wasm](crates/astroceleste-engine-wasm).

Both are tested against the same golden fixtures as the Rust crate.

## Layout

| Path | Content |
|---|---|
| `crates/astroceleste-engine` | the core library |
| `crates/astroceleste-engine-py` | Python bindings (PyO3, built with maturin) |
| `crates/astroceleste-engine-wasm` | WebAssembly bindings (wasm-bindgen, built with wasm-pack) |
| `tests/fixtures/*.json` | golden charts produced by the reference implementation, the acceptance spec |
| `tests/data/de440s_2000.bsp` | one-year excerpt of DE440s used by the tests |
| `scripts/fetch-kernels.sh` | downloads full JPL kernels into `kernels/` (gitignored) |
| `scripts/make_spk_fixtures.py` | rebuilds the excerpt and the SPK reference states with `jplephem` |
| `scripts/make_reduction_fixtures.py` | stage-by-stage reference values from Skyfield |
| `scripts/gen_tables.py` | regenerates the embedded ΔT, nutation and Chiron tables |

## Development

```bash
cargo test --workspace --exclude astroceleste-engine-py   # uses the committed excerpt
scripts/fetch-kernels.sh        # optional: full de440s (1849-2150), enables the full-range tests
cargo clippy --workspace --all-targets -- -D warnings

# Python bindings (in a virtualenv)
pip install maturin pytest && maturin develop -m crates/astroceleste-engine-py/Cargo.toml
pytest crates/astroceleste-engine-py/tests

# WebAssembly bindings (needs wasm-pack and the wasm32-unknown-unknown target)
wasm-pack build --target nodejs --out-dir pkg-node crates/astroceleste-engine-wasm
node crates/astroceleste-engine-wasm/tests/golden.mjs
```

## Releasing

Releases are trunk-based: there are no release branches or release PRs. Pushing a
`vX.Y.Z` tag on `main` publishes that version.

1. Commits on `main` follow [Conventional Commits](https://www.conventionalcommits.org)
   (`feat:`, `fix:`, `perf:`, `refactor:`, … and `!` for breaking changes). While the
   version is 0.0.x, `feat`/`fix` bump the patch version and a breaking change bumps the
   minor version.
2. Prepare the release on `main`: run `release-plz update` (or edit by hand) to bump the
   workspace version, the bindings' `astroceleste-engine` dependency version and
   `CHANGELOG.md`, then commit as `chore: release vX.Y.Z` and push.
3. Once CI is green, tag that commit and push the tag:
   `git tag vX.Y.Z && git push origin vX.Y.Z`.
4. The `Release` workflow checks that the tag is on `main`, matches the version in
   `Cargo.toml` and has a `CHANGELOG.md` entry. It then publishes the crate to crates.io,
   creates the GitHub Release from the changelog entry, and publishes the Python wheels
   (Linux x86_64/aarch64, macOS universal2, Windows x64, sdist) to PyPI and the
   WebAssembly package to npm.

Protect `v*` tags with a tag ruleset so only maintainers can trigger a release.

All three registries use Trusted Publishing (OIDC), so no long-lived token is stored.
crates.io and npm can only trust a workflow for a package that already exists, so the
first release uses short-lived tokens instead:

1. Create the `release`, `pypi` and `npm` environments in the repository settings
   (optionally with required reviewers).
2. **crates.io**: create an API token scoped to `publish-new` and `publish-update` for the
   crate `astroceleste-engine`, with a short expiry, and store it as the
   `CARGO_REGISTRY_TOKEN` secret of the `release` environment.
3. **npm**: create a granular access token with publish rights and a short expiry, and store
   it as the `NPM_TOKEN` secret of the `npm` environment.
4. **PyPI**: add a *pending* trusted publisher for project `astroceleste-engine` (workflow
   `release.yml`, environment `pypi`). No token is needed.
5. Push the `vX.Y.Z` tag for the version in `Cargo.toml`.
6. Once the packages exist, add the trusted publisher on crates.io (crate settings →
   *Trusted Publishing*: this repository, workflow `release.yml`, environment `release`)
   and on npm (package settings: workflow `release.yml`, environment `npm`). Then delete
   both secrets and revoke the tokens. The workflow prefers OIDC whenever it is available.

## Ephemerides

The engine reads NASA JPL SPK kernels (DE440s by default, DE441 for dates outside 1849–2150).
Dates outside the loaded kernels are reported as errors, never approximated.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in this work, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.
