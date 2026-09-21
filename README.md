# astroceleste-engine

Astrological chart calculation on JPL ephemerides, in pure Rust.

It is the calculation core of [Astroceleste](https://astroceleste.it). The same code runs
on the server (Python bindings), in the desktop and mobile apps (native) and in the browser
(WASM), so every platform computes the same chart down to the arcsecond.

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

Releases are automated with [release-plz](https://release-plz.dev):

1. Commits on `main` follow [Conventional Commits](https://www.conventionalcommits.org)
   (`feat:`, `fix:`, `perf:`, `refactor:`, … and `!` for breaking changes).
2. The `Release` workflow keeps a release PR open. It bumps the version and updates
   `CHANGELOG.md`.
3. Merging that PR tags `vX.Y.Z`, creates the GitHub Release and publishes to crates.io.

One-time setup:

- **First publish** is manual (crates.io trusted publishing can only be configured for an
  existing crate): `cargo publish -p astroceleste-engine` with a personal token.
- On crates.io → crate settings → *Trusted Publishing*, add this repository, workflow
  `release.yml`, environment `release`. From then on, no registry token is stored anywhere.
- Create the `release` environment in the GitHub repository settings (optionally with
  required reviewers).
- Release PRs opened with the default `GITHUB_TOKEN` do not trigger CI. To get CI on
  them, use a GitHub App or fine-grained token in the `release-pr` job.

The same release publishes the Python wheels (Linux x86_64/aarch64, macOS universal2,
Windows x64, sdist) to PyPI and the WebAssembly package to npm, from `release.yml`:

- **PyPI**: add a *pending* trusted publisher for project `astroceleste-engine`
  (workflow `release.yml`, environment `pypi`); it works from the first release.
- **npm**: trusted publishing can only be configured on an existing package, so publish
  the first version by hand (`wasm-pack build --release --target web
  crates/astroceleste-engine-wasm`, then in `pkg/`: `npm pkg set name=astroceleste-engine
  && npm publish --access public`), then add the trusted publisher (workflow
  `release.yml`, environment `npm`).
- Create the `pypi` and `npm` environments in the repository settings.

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
