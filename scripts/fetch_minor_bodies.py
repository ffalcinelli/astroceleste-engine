"""Generate the embedded minor-body longitude tables from JPL Horizons.

    python3 scripts/fetch_minor_bodies.py [--cache DIR]

For each body in BODIES this queries the JPL Horizons API for the geocentric apparent
longitude in the ecliptic of date (quantity 31: light-time, gravitational deflection and
aberration applied), one sample a day from 1849 to 2152 (TT). It then keeps every
`step`-th sample, choosing the largest step whose 6-point (quintic) Lagrange
interpolation, as `minor_bodies.rs` evaluates it, stays within MAX_ERROR_ARCSEC of every
daily value, and writes:

  src/data/minor/<body>.bin                  jd_start (TT, f64), step (days, f64), n (u32),
                                             the first unwrapped longitude in radians (f64),
                                             then the n-1 differences between consecutive
                                             samples in units of DELTA_UNIT radians (i32);
                                             little-endian
  tests/fixtures/minor_bodies.json           Horizons longitudes at instants between the
                                             samples, for tests/minor_bodies.rs

Horizons data is computed by NASA JPL (see NOTICE). Responses are cached in --cache, so a
re-run only reshapes them.
"""

import argparse
import json
import math
import struct
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "crates" / "astroceleste-engine" / "src" / "data" / "minor"
FIXTURE = ROOT / "tests" / "fixtures" / "minor_bodies.json"
API = "https://ssd.jpl.nasa.gov/api/horizons.api"

# (name, Horizons small-body number)
BODIES = [
    ("Ceres", 1),
    ("Pallas", 2),
    ("Juno", 3),
    ("Vesta", 4),
    ("Astraea", 5),
    ("Eris", 136199),
    ("Sedna", 90377),
    ("Makemake", 136472),
    ("Orcus", 90482),
    ("Ixion", 28978),
]

STOP = "2152-01-01"
# Horizons caps a response at about 90,000 lines: fetch the daily series in chunks.
CHUNKS = [("1849-01-01", "1950-01-01"), ("1950-01-02", "2051-01-01"), ("2051-01-02", STOP)]
# Horizons' own daily series has jumps of 1-2" here and there (its integration segments),
# so a tighter bound only measures those. 3" is far below the arc minute charts display.
MAX_ERROR_ARCSEC = 3.0
STEPS = [1, 2, 4, 8, 16, 32, 64, 128]
# 1e-10 rad = 0.00002": exact for our purposes, and half the size of f64 samples.
DELTA_UNIT = 1e-10


def horizons(number: int, start: str, stop: str, step: str, cache: Path) -> list[tuple[float, float]]:
    """(JD TT, longitude in degrees) rows of a Horizons observer table."""
    key = f"{number}_{start}_{stop}_{step}".replace(" ", "_").replace(":", "")
    path = cache / f"{key}.txt"
    if not path.exists():
        query = {
            "format": "text",
            "COMMAND": f"'{number};'",
            "OBJ_DATA": "'NO'",
            "MAKE_EPHEM": "'YES'",
            "EPHEM_TYPE": "'OBSERVER'",
            "CENTER": "'500@399'",
            "START_TIME": f"'{start}'",
            "STOP_TIME": f"'{stop}'",
            "STEP_SIZE": f"'{step}'",
            "QUANTITIES": "'31'",
            "TIME_TYPE": "'TT'",
            "CSV_FORMAT": "'YES'",
            "CAL_FORMAT": "'JD'",
            "ANG_FORMAT": "'DEG'",
            "EXTRA_PREC": "'YES'",
        }
        with urllib.request.urlopen(f"{API}?{urllib.parse.urlencode(query)}", timeout=300) as r:
            text = r.read().decode()
        if "$$SOE" not in text:
            raise SystemExit(f"Horizons error for {number}:\n{text[-2000:]}")
        path.write_text(text)
        time.sleep(1)
    text = path.read_text()
    rows = text.split("$$SOE")[1].split("$$EOE")[0].strip().splitlines()
    return [(float(r.split(",")[0]), float(r.split(",")[3])) for r in rows]


def unwrap(degrees: list[float]) -> list[float]:
    """Continuous longitudes in radians (no jumps at 0°/360°)."""
    out = [math.radians(degrees[0])]
    for d in degrees[1:]:
        r = math.radians(d)
        prev = out[-1]
        r += 2 * math.pi * round((prev - r) / (2 * math.pi))
        out.append(r)
    return out


# Interpolation nodes around sample i (t in [0, 1) between samples i and i+1).
NODES = range(-2, 4)


def interpolate(y: list[float], i: int, t: float) -> float:
    """6-point Lagrange through samples i-2..i+3 at fraction t, as minor_bodies.rs does."""
    total = 0.0
    for j in NODES:
        w = 1.0
        for k in NODES:
            if k != j:
                w *= (t - k) / (j - k)
        total += w * y[i + j]
    return total


def max_error(daily: list[float], step: int) -> float:
    """Worst interpolation error (arcsec) against the daily series with one-day samples."""
    samples = daily[::step]
    worst = 0.0
    for i in range(-NODES[0], len(samples) - NODES[-1]):
        for k in range(1, step):
            exact = daily[i * step + k]
            worst = max(worst, abs(interpolate(samples, i, k / step) - exact))
    return math.degrees(worst) * 3600


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", type=Path, default=Path(".horizons-cache"))
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)
    DATA.mkdir(parents=True, exist_ok=True)

    fixture = {}
    for name, number in BODIES:
        rows = [r for a, b in CHUNKS for r in horizons(number, a, b, "1 d", args.cache)]
        jds = [jd for jd, _ in rows]
        assert all(abs(b - a - 1.0) < 1e-9 for a, b in zip(jds, jds[1:])), f"{name}: gap"
        daily = unwrap([lon for _, lon in rows])

        step = max(s for s in STEPS if max_error(daily, s) <= MAX_ERROR_ARCSEC)
        samples = daily[::step]
        units = [round((y - samples[0]) / DELTA_UNIT) for y in samples]
        deltas = [b - a for a, b in zip(units, units[1:])]
        assert max(map(abs, deltas)) < 2**31, f"{name}: delta overflow"
        out = struct.pack("<ddId", jds[0], float(step), len(samples), samples[0])
        out += struct.pack(f"<{len(deltas)}i", *deltas)
        (DATA / f"{name.lower()}.bin").write_bytes(out)
        print(
            f"{name}: step {step} d, {len(samples)} samples, {len(out) // 1024} KiB, "
            f"max error {max_error(daily, step):.3f}\""
        )

        # Check points between the samples (an odd step, at a fractional hour).
        checks = horizons(number, "1849-06-01 05:17", "2151-06-30", "379 d", args.cache)
        fixture[name] = [[jd, lon] for jd, lon in checks]

    FIXTURE.write_text(json.dumps(fixture, indent=1) + "\n")
    print(f"{FIXTURE.relative_to(ROOT)}: {sum(len(v) for v in fixture.values())} points")


if __name__ == "__main__":
    main()
