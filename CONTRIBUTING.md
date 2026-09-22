# Contributing

## Correctness first

Every number the engine produces is checked against a reference:

- `tests/fixtures/spk_reference.json`: raw kernel states from `jplephem`
  (`scripts/make_spk_fixtures.py`).
- `tests/fixtures/{natal,horary,transits,synastry,derived,errors}.json`: complete charts from
  the reference implementation. A change that moves any of them must explain why the new
  value is more correct, and regenerate the fixtures in the same pull request.

Run the full suite with the complete kernel before opening a pull request:

```bash
scripts/fetch-kernels.sh
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Dependencies

The engine ships inside closed-source applications and in the browser, so it stays
dependency-light and permissively licensed (`deny.toml`). It must keep building for
`wasm32-unknown-unknown`, Android and iOS: no C code, no platform-specific I/O in the core.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org): `release-plz update` derives
the version bump and the changelog from them. By contributing you agree to license your work
under MIT OR Apache-2.0.
