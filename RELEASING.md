# Releasing

Releases are trunk-based: there are no release branches or release PRs. Pushing a
`vX.Y.Z` tag on `main` publishes that version. One workspace version is shared by the
crate, the PyPI wheels and the npm package.

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
   (Linux x86_64/aarch64 glibc and musl, macOS universal2, Windows x64, sdist) to PyPI
   and the WebAssembly package to npm.

Protect `v*` tags with a tag ruleset so only maintainers can trigger a release.

## Trusted publishing

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

## Website

The landing site and live demo (`site/`) are deployed to GitHub Pages by
`.github/workflows/pages.yml` on every push to `main` that touches the site or the crates,
independently of releases. The repository's Pages source must be set to *GitHub Actions*.
