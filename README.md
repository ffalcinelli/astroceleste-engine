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

**[Website and live demo](https://ffalcinelli.github.io/astroceleste-engine/)** · [API docs](https://docs.rs/astroceleste-engine) ·
[Guides](https://github.com/ffalcinelli/astroceleste-engine/tree/main/docs)

> **Experimental (0.0.x):** the API may change in any release. Pin an exact version.

> Status: the complete chart pipeline is ported and matches the reference implementation
> on every golden chart: planets, lunar nodes, Chiron, Lilith, houses (Placidus, Whole Sign,
> Equal, Porphyry; Koch, Regiomontanus, Campanus, Topocentric, Alcabitius, Morinus and Vehlow
> are checked against Swiss Ephemeris), aspects and orbs, fixed stars, Arabic parts, temperament, lunar status,
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
`calculate_horary_chart`, `calculate_transit_chart`, `calculate_synastry`,
`calculate_derived_chart`, and for electional astrology `calculate_election_chart` and
`search_elections` (the best moments over a span of time). In the browser or on mobile, load the kernel from memory with
`Spk::from_bytes`.

## Bindings

- **Python** (`pip install astroceleste-engine`, CPython ≥ 3.12): `Engine([kernel paths])`
  with `.chart()`, `.horary()`, `.transit()`, `.election()`, `.elections()`, plus `synastry()` and `derived_chart()`,
  returning plain dicts. See [crates/astroceleste-engine-py](https://github.com/ffalcinelli/astroceleste-engine/tree/main/crates/astroceleste-engine-py).
- **JavaScript / WebAssembly** (`npm install astroceleste-engine`): the same API over
  kernels loaded from memory, for browsers and Node. See
  [crates/astroceleste-engine-wasm](https://github.com/ffalcinelli/astroceleste-engine/tree/main/crates/astroceleste-engine-wasm).

Both are tested against the same golden fixtures as the Rust crate.

## Documentation

- [API reference on docs.rs](https://docs.rs/astroceleste-engine)
- [API guide](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/api.md): request options, the chart JSON, error codes in every
  language
- [Ephemerides](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/ephemerides.md): kernels, date coverage, loading from memory,
  smaller excerpts for the web and mobile
- [Accuracy](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/accuracy.md): how every number is verified
- [Architecture](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/architecture.md): crates and calculation pipeline

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
| `scripts/build-site.sh` | builds the landing site and live demo into `target/site/` |
| `docs/` | guides |
| `site/` | landing site and live demo, deployed to GitHub Pages |

## Contributing

Contributions are welcome: see [CONTRIBUTING.md](https://github.com/ffalcinelli/astroceleste-engine/blob/main/CONTRIBUTING.md) for setup, the test
suite and the rules that keep every platform identical. Report vulnerabilities privately,
as described in [SECURITY.md](https://github.com/ffalcinelli/astroceleste-engine/blob/main/SECURITY.md). Maintainers release by pushing a version
tag ([RELEASING.md](https://github.com/ffalcinelli/astroceleste-engine/blob/main/RELEASING.md)).

```bash
cargo test --workspace --exclude astroceleste-engine-py   # uses the committed excerpt
scripts/fetch-kernels.sh        # full de440s (1849-2150): enables the golden and full-range tests
cargo clippy --workspace --all-targets -- -D warnings
```

## Ephemerides

The engine reads NASA JPL SPK kernels (DE440s by default, DE441 for dates outside 1849–2150).
Dates outside the loaded kernels are reported as errors, never approximated. See
[docs/ephemerides.md](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/ephemerides.md) for coverage, sizes and excerpts.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/ffalcinelli/astroceleste-engine/blob/main/LICENSE-APACHE))
- MIT license ([LICENSE-MIT](https://github.com/ffalcinelli/astroceleste-engine/blob/main/LICENSE-MIT))

at your option.

The astronomical reduction (ΔT, IAU 2000A nutation, IAU 2006 precession, light-time,
deflection and aberration) is ported from [Skyfield](https://github.com/skyfielders/python-skyfield)
by Brandon Rhodes, used under the MIT license: see
[NOTICE](https://github.com/ffalcinelli/astroceleste-engine/blob/main/NOTICE), which is shipped in
every package.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in this work, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.
