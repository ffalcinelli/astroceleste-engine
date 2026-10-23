# Vedic astrology

The engine reads a chart the Jyotish way: sidereal placements with their nakshatras,
the divisional charts (vargas) and the Vimshottari dasha. It also computes the panchanga
of a moment and place. Results carry stable codes only:
- lowercase ASCII transliterations for nakshatras, tithis, yogas, karanas, weekdays,
  months, seasons and years (`ashwini`, `pratipada`, `vishkambha`, `chaitra`, `krodhi`);
- the engine's names for bodies and signs: Rahu is `North Node`, Ketu `South Node`.

The wording is left to the application. Parts are ported from Priyank Gahtori's
[Khagole](https://github.com/PriyankGahtori/Khagole) fork.

| Step | Needs a kernel | Entry point |
|---|---|---|
| The panchanga: sunrise, vara, tithi, nakshatra, yoga, karana, month, year | yes | `vedic_calendar`, or `vedic_calendar: true` on a chart request (the chart's `vedic_calendar` key) |
| The reading: placements, nakshatras, vargas, dasha | no | `jyotish` (Python `jyotish`, JavaScript `jyotish`) |

An app can store the panchanga with a chart and re-cast the reading offline whenever the
ayanamsa, the vargas or the dasha options change.

## The reading

```python
chart = engine.chart("1987-05-17T14:30:00Z", 41.9, 12.5)
reading = ace.jyotish(chart, "1987-05-17T14:30:00Z", {"date": "2026-10-23T00:00:00Z"})
reading["grahas"][2]
# {"name": "Moon", "longitude": 268.315…, "sign": "Sagittarius", "degree": 28.315…,
#  "house": 4, "is_retrograde": False,
#  "nakshatra": {"number": 21, "code": "uttara_ashadha", "lord": "Sun", "pada": 1,
#                "elapsed": 0.1236…},
#  "vargottama": True}
```

The chart may be tropical or sidereal under any ayanamsa: its longitudes are taken back to
tropical and then shifted by the ayanamsa of the options.

| Option | Meaning | Default |
|---|---|---|
| `ayanamsa` | a code from the catalog (unknown codes are an error) | `lahiri` |
| `outer_planets` | also list Uranus, Neptune and Pluto | `false` |
| `vargas` | the divisions to cast | all twenty |
| `hora` | the D2: `parashara` (the Sun's Leo and the Moon's Cancer) or `cyclic` | `parashara` |
| `dasha_year` | `julian` (365.25 days), `gregorian` (365.2425), `sidereal` (365.256363) or `savana` (360) | `julian` |
| `dasha_path` | lords from a mahadasha down: list the subperiods under them | the mahadashas |
| `dasha_depth` | levels below `dasha_path`, 1 to 3 (5 levels in all at most) | 2 |
| `date` | also list the periods running then (ISO 8601 UTC) | none |

- **`grahas`**: the Ascendant, then the Sun, the Moon, Mars, Mercury, Jupiter, Venus,
  Saturn, Rahu and Ketu, each with its sidereal sign and degree, its whole-sign house from
  the Ascendant, its nakshatra (with its Vimshottari lord, pada and the fraction crossed)
  and whether it is vargottama (the same sign in the navamsha).
- **`vargas`**: D1, D2, D3, D4, D5, D6, D7, D8, D9, D10, D11, D12, D16, D20, D24, D27,
  D30, D40, D45 and D60, each with the varga sign, degree and house of every placement.
  The signs follow Brihat Parashara Hora Shastra; D6, D8 and D11 count cyclically from
  Aries. The degree is the position within the part, stretched to a whole sign.
- **`vimshottari`**: the lord of the Moon's nakshatra (`birth_lord`) and the years of its
  mahadasha left at birth (`balance_years`); the periods under `path`, `depth` levels deep,
  each `{lord, start, end, periods}`; and with a `date`, the five periods running then
  (`current`, mahadasha to prana dasha). The first mahadasha starts before the birth, by
  the part of it the Moon had already crossed. A true sidereal solar year, which needs
  the ephemeris, is not offered yet.

## The panchanga

```python
engine.vedic_calendar("2024-04-09T06:00:00Z", 28.6139, 77.2090)
# {"ayanamsa": "lahiri", "sunrise": "2024-04-09T00:32:00+00:00", …,
#  "vara": {"number": 3, "code": "mangalavara", "lord": "Mars"},
#  "tithi": {"number": 1, "code": "pratipada", "start": "2024-04-08T18:20:52+00:00",
#            "end": "2024-04-09T15:01:26+00:00"},
#  "paksha": "shukla",
#  "nakshatra": {"number": 1, "code": "ashwini", …}, "pada": 1,
#  "yoga": {"number": 27, "code": "vaidhriti", …}, "karana": {"number": 2, "code": "bava", …},
#  "masa": {"amanta": "chaitra", "purnimanta": "chaitra", "adhika": False, …},
#  "ritu": "vasanta", "ayana": "uttarayana",
#  "samvatsara_number": 38, "samvatsara": "krodhi", "shaka_year": 1946, "vikram_year": 2081}
```

- **The day** begins at sunrise: the vara is the weekday of the sunrise at or before the
  moment, in local mean time. Where the Sun does not rise or set, half-day defaults stand
  in for the sunrise and sunset.
- **The angas** running at the moment, each with the instants it began and ends (to the
  second): the tithi (each 12° of the Moon's elongation), the nakshatra (the Moon's
  sidereal longitude), the yoga (the sum of the Sun's and the Moon's sidereal longitudes,
  in 27 parts) and the karana (half a tithi).
- **The month** is amanta, from new moon to new moon, named after the sidereal sign the
  Sun enters during it. A month without an ingress is adhika and takes the next month's
  name. In the dark fortnight the purnimanta month is the next one.
- **The year** begins with Chaitra: the samvatsara of the sixty-year cycle as reckoned
  with the Shaka era (southern India), and the Shaka and Vikram years.

The sidereal angles use the Lahiri ayanamsa, the panchanga standard. Sunrise and the anga
searches use the same apparent positions as the charts, so the times agree with published
panchangas to about a minute. The tests check Ugadi 2024 (Krodhi, Shaka 1946), the
adhika Shravana of 2023 and the Vedic day before sunrise.

It adds a few milliseconds to a chart. The panchanga needs about a month of ephemeris
around the moment; near the edge of the loaded kernels it is left out (`None`), and the
chart is still computed.
