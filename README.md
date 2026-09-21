# astroceleste-engine

Astrological chart calculation on JPL ephemerides, in pure Rust.

It is the calculation core of [Astroceleste](https://astroceleste.it). The same code runs
on the server (Python bindings), in the desktop and mobile apps (native) and in the browser
(WASM), so every platform computes the same chart down to the arcsecond.

> Status: early. Done and verified: the JPL SPK reader (against `jplephem`), the
> reduction to apparent positions of date (against Skyfield, to 1e-7″), planets, lunar
> nodes, Chiron, Lilith and house cusps (identical to the reference on all golden charts).
> Being ported: aspects, fixed stars, lots, temperament, lunar data, horary, transits,
> synastry and derived charts.

## Layout

| Path | Content |
|---|---|
| `crates/astroceleste-engine` | the core library |
| `tests/fixtures/*.json` | golden charts produced by the reference implementation, the acceptance spec |
| `tests/data/de440s_2000.bsp` | one-year excerpt of DE440s used by the tests |
| `scripts/fetch-kernels.sh` | downloads full JPL kernels into `kernels/` (gitignored) |
| `scripts/make_spk_fixtures.py` | rebuilds the excerpt and the SPK reference states with `jplephem` |
| `scripts/make_reduction_fixtures.py` | stage-by-stage reference values from Skyfield |
| `scripts/gen_tables.py` | regenerates the embedded ΔT, nutation and Chiron tables |

## Development

```bash
cargo test                      # uses the committed excerpt
scripts/fetch-kernels.sh        # optional: full de440s (1849-2150), enables the full-range tests
cargo clippy --all-targets -- -D warnings
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

The Python (PyPI) and WASM (npm) packages will join the same release train once their
bindings exist, both through trusted publishing.

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
