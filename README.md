# astroceleste-engine

Astrological chart calculation on JPL ephemerides, in pure Rust.

It is the calculation core of [Astroceleste](https://astroceleste.it). The same code runs
on the server (Python bindings), in the desktop and mobile apps (native) and in the browser
(WASM), so every platform computes the same chart down to the arcsecond.

> Status: early. The JPL SPK reader is done and verified against `jplephem`. The chart
> pipeline (apparent positions, houses, aspects, lots, fixed stars, lunar data, transits,
> synastry, derived charts) is being ported against the golden fixtures in `tests/fixtures/`.

## Layout

| Path | Content |
|---|---|
| `crates/astroceleste-engine` | the core library |
| `tests/fixtures/*.json` | golden charts produced by the reference implementation, the acceptance spec |
| `tests/data/de440s_2000.bsp` | one-year excerpt of DE440s used by the tests |
| `scripts/fetch-kernels.sh` | downloads full JPL kernels into `kernels/` (gitignored) |
| `scripts/make_spk_fixtures.py` | rebuilds the excerpt and the SPK reference states with `jplephem` |

## Development

```bash
cargo test                      # uses the committed excerpt
scripts/fetch-kernels.sh        # optional: full de440s (1849-2150), enables the full-range tests
cargo clippy --all-targets -- -D warnings
```

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
