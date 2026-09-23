# API guide

The engine has the same five entry points in every language. Requests and results have the
same shape everywhere: results serialize to the JSON of the Astroceleste API, with
snake_case keys.

| Rust | Python | JavaScript |
|---|---|---|
| `calculate_chart` | `Engine.chart` | `engine.chart` |
| `calculate_horary_chart` | `Engine.horary` | `engine.horary` |
| `calculate_transit_chart` | `Engine.transit` | `engine.transit` |
| `calculate_synastry` | `synastry` | `synastry` |
| `calculate_derived_chart` | `derived_chart` | `derivedChart` |

Every calculation needs JPL kernels (see [Ephemerides](ephemerides.md)); synastry and
derived charts work on already computed charts and do not.

## A chart request

| Field | Meaning | Default |
|---|---|---|
| moment | UTC instant: `UtcInstant` in Rust, `datetime` or ISO string in Python, ISO string `utc` in JS | required |
| latitude | geographic latitude in degrees, north positive | required |
| longitude | geographic longitude in degrees, east positive | required |
| house system | `P` Placidus, `K` Koch, `R` Regiomontanus, `C` Campanus, `T` Topocentric (Polich-Page), `B` Alcabitius, `M` Morinus, `O` Porphyry, `E` Equal, `V` Vehlow, `W` Whole Sign (the Swiss Ephemeris letters) | `P` |
| zodiac type | `tropical` or `sidereal` | `tropical` |
| ayanamsa | sidereal reference, see below (ignored for tropical charts) | `galcent_0sag` |
| orb settings | overrides merged over the default orbs, see below | none |

Times are always UTC. Convert civil time (with its time zone and daylight saving time) to
UTC before calling the engine. Only the first letter of the house system is used, and
unknown codes fall back to Placidus. Koch falls back to Porphyry inside the polar circles,
where it is undefined. Morinus cusps 1 and 10 are not the Ascendant and Midheaven. Zodiac types other than `sidereal` are tropical.

```rust
use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
use astroceleste_engine::{calculate_chart, ChartRequest, UtcInstant};

let mut kernels = KernelSet::new();
kernels.push(Kernel::new("de440s.bsp", Spk::open("kernels/de440s.bsp")?)?);

let mut request = ChartRequest::new(UtcInstant::parse("1987-05-17T14:30:00Z")?, 41.9, 12.5);
request.house_system = "W";
request.zodiac_type = "sidereal";
request.ayanamsa = "lahiri";
let chart = calculate_chart(&kernels, &request)?;
```

```python
import astroceleste_engine as ace

engine = ace.Engine(["kernels/de440s.bsp"])
chart = engine.chart("1987-05-17T14:30:00Z", 41.9, 12.5,
                     house_system="W", zodiac_type="sidereal", ayanamsa="lahiri")
```

```js
const chart = engine.chart({
  utc: "1987-05-17T14:30:00Z", latitude: 41.9, longitude: 12.5,
  house_system: "W", zodiac_type: "sidereal", ayanamsa: "lahiri",
});
```

### Ayanamsas

| Code | Name |
|---|---|
| `lahiri` | Lahiri (Chitra Paksha) |
| `fagan_bradley` | Fagan-Bradley |
| `krishnamurti` | Krishnamurti (KP) |
| `raman` | B.V. Raman |
| `true_citra` | True Chitra |
| `yukteshwar` | Sri Yukteswar |
| `deluce` | De Luce |
| `ushashashi` | Usha-Shashi |
| `jn_bhasin` | J.N. Bhasin |
| `aldebaran_15tau` | Aldebaran 15° Taurus |
| `hipparchos` | Hipparchos |
| `sassanian` | Sassanian |
| `galcent_0sag` | Galactic Center 0° Sagittarius (default) |
| `j2000` | J2000.0 Epoch |

Codes are case-insensitive. Unknown codes use the default.

### Orb settings

Pass a JSON object (a `serde_json::Value` in Rust, a dict in Python, an object in JS).
Only these keys are read, and the orb tables are merged key by key over the defaults:

```json
{
  "method": "moiety_hybrid",
  "fixed_star_orb": 1.5,
  "aspect_orbs": { "Conjunction": 8.0, "Sextile": 6.0, "Square": 7.0, "Trine": 8.0,
                   "Opposition": 8.0, "Semi-Sextile": 2.5, "Quincunx": 3.0 },
  "planet_orbs": { "Sun": 15.0, "Moon": 12.0, "Mercury": 7.0, "Venus": 7.0, "Mars": 8.0,
                   "Jupiter": 9.0, "Saturn": 9.0, "Uranus": 5.0, "Neptune": 5.0,
                   "Pluto": 5.0, "North Node": 5.0, "South Node": 5.0, "Chiron": 5.0,
                   "Lilith": 5.0, "Ascendant": 5.0, "Midheaven": 5.0 }
}
```

`method` selects how the maximum orb of an aspect between two bodies is computed:

- `moiety_traditional`: the mean of the two planets' orbs (moieties);
- `moiety_aspect_weighted`: the moiety scaled by the aspect's orb relative to the
  conjunction;
- `moiety_hybrid` (default): the moiety, capped by the aspect's orb;
- anything else: the aspect's orb alone.

The settings in effect are echoed back in the chart's `orb_settings`.

## The chart

A chart is an object with these keys, in this order:

| Key | Content |
|---|---|
| `house_system`, `zodiac_type` | echoed from the request |
| `ayanamsa`, `ayanamsa_name`, `ayanamsa_value`, `ayanamsa_formatted` | sidereal reference at the chart moment (`null` for tropical charts) |
| `precession_rate_arcsec_yr` | precession rate used for the fixed stars |
| `orb_settings` | the orb settings in effect |
| `planets` | Sun to Pluto, North and South Node, Chiron, Lilith, Ascendant, Midheaven |
| `unavailable_bodies` | expected bodies that could not be computed (Chiron outside 1849–2151) |
| `houses` | the twelve cusps |
| `aspects` | aspects between chart points, then fixed-star conjunctions |
| `fixed_stars` | fixed stars conjunct a chart point |
| `arabic_parts` | the lots (Fortune, Spirit, …) with the formula used |
| `temperament` | traditional temperament: scores, qualities and their breakdown |
| `lunar_status` | phase, illumination, age, speed, dignity and lunar mansion |

A placement in `planets` (houses, fixed stars and lots use a similar shape):

```json
{ "name": "Sun", "symbol": "☉", "sign": "Capricorn", "sign_symbol": "♑",
  "degree": 10, "minute": 22, "ecliptic_longitude": 280.36891840215475,
  "house": 10, "speed": 1.0194357508121357, "is_retrograde": false,
  "symbolic_degree": 11 }
```

An aspect:

```json
{ "body1": "Sun", "body2": "Moon", "aspect_type": "Sextile", "symbol": "⚹",
  "angle": 60.0, "orb": 2.95, "max_orb": 6.0, "is_major": true, "is_applying": true }
```

`is_applying` is always `true` in chart aspects, as in the reference implementation, which
does not evaluate it there. Horary charts compute the Moon's applying and separating
aspects from its speed (`horary_data`).

Longitudes are apparent geocentric ecliptic longitudes of date, in degrees (sidereal charts
subtract the ayanamsa). In Rust, the fields are typed: see [`Chart`] and [`Placement`] on
docs.rs.

[`Chart`]: https://docs.rs/astroceleste-engine/latest/astroceleste_engine/struct.Chart.html
[`Placement`]: https://docs.rs/astroceleste-engine/latest/astroceleste_engine/struct.Placement.html

## Other entry points

- **Horary**: the chart for the moment of the question, plus `horary_data` with the
  planetary day and hour (computed from the actual sunrise and sunset at the location),
  significators, the Moon's applying and separating aspects, void of course and the
  considerations before judgment.
- **Transit**: the sky at a moment and place, with the aspects it forms to a natal chart's
  `planets` (`cross_aspects`).
- **Synastry**: cross-aspects from chart B's `planets` to chart A's `planets`.
- **Derived chart**: a stored chart turned so that radix house *n* (1–12) becomes the first
  house, with the meaning of each derived house. Fields that are not turned are kept.

## Errors

Each failure has a stable code, shared with the Astroceleste HTTP API:

| Code | Cause | Rust | Python | JavaScript |
|---|---|---|---|---|
| `ephemeris_out_of_range` | no loaded kernel covers the date | `EngineError::OutOfRange` | `EphemerisRangeError` | `Error` with `code` |
| `ephemeris_error` | a kernel could not be read or evaluated | `EngineError::Ephemeris` | `EngineError` | `Error` with `code` |
| `invalid_input` | malformed request (bad date, non-numeric orb, …) | `EngineError::InvalidInput` | `ValueError` | `Error` with `code` |
| `invalid_kernel` | a kernel file is not a valid SPK file (JS `addKernel`) | `SpkError` when loading | `EngineError` | `Error` with `code` |

In Rust, `EngineError::code()` returns the code. Dates outside the loaded kernels are
always reported as errors and never approximated. Check them upfront with `supports(jd)` /
`coverage` in Python and JavaScript, or `KernelSet::for_jd` / `KernelSet::coverage` in
Rust.
