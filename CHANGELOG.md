# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the
version is 0.0.x the API is experimental and any release may break it.

## [Unreleased]

### Added

- Degree qualities after William Lilly (*Christian Astrology*, 1659, p. 116):
  `degree_qualities` gives the masculine or feminine, light, dark, smoky or void, pitted,
  lame (azimene) and fortune-increasing qualities of a longitude's degree, and
  `degree_quality_table` returns the whole table. Python: `degree_qualities` /
  `degree_quality_table`. JavaScript: `degreeQualities` / `degreeQualityTable`.
- Elections weigh the degree qualities of the Moon and the Ascendant: `MOON_PITTED_DEGREE`,
  `MOON_AZIMENE_DEGREE`, `MOON_FORTUNE_DEGREE`, `ASC_PITTED_DEGREE`, `ASC_AZIMENE_DEGREE`,
  `ASC_FORTUNE_DEGREE`, `ASC_LIGHT_DEGREE` and `ASC_DARK_DEGREE`. Scores change for moments
  where they apply.

## [0.0.4]

### Added

- Electional astrology. `calculate_election_chart` scores a moment by the traditional
  electional rules and returns stable factor codes with weights. The rules cover the Moon,
  the Ascendant and its ruler, benefics and malefics on the angles, retrograde Mercury and
  Venus, the house of the matter and its significator, the planetary hour and, optionally,
  contacts with a natal chart. `search_elections` finds the best windows over up to 92 days
  at a place, with purpose presets and filters (void-of-course Moon, retrogrades, daytime,
  local hours with daylight saving time). Python: `Engine.election` / `Engine.elections`.
  JavaScript: `engine.election` / `engine.elections`.

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
