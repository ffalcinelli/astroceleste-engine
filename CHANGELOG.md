# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the
version is 0.0.x the API is experimental and any release may break it.

## [Unreleased]

## [0.0.3]

### Added

- Koch, Regiomontanus, Campanus, Topocentric, Alcabitius, Morinus and Vehlow house
  systems. These codes used to fall back to Placidus silently; Koch now falls back to
  Porphyry inside the polar circles, where it is undefined.
- Landing site with a live WebAssembly demo, API guide and architecture docs, security
  policy and an expanded contributing guide.

### Fixed

- The npm package now ships the licenses and NOTICE (Skyfield's MIT notice).

## [0.0.2]

### Added

- Python wheels for musllinux (Alpine and other musl-based distributions).

## [0.0.1]

### Added

- Initial experimental release: the complete Astroceleste chart pipeline on JPL SPK
  kernels (natal, horary, transit, synastry and derived charts), with Python and
  WebAssembly bindings.
