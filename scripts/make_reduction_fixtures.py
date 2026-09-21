"""Stage-by-stage reference values from Skyfield 1.55, for testing each reduction step.

    uv run --no-project --with skyfield==1.55 --with numpy \
        scripts/make_reduction_fixtures.py kernels/de440s.bsp

For every natal fixture instant (tests/fixtures/natal.json) this records the time scales,
sidereal time, nutation, the ICRS → true-equator-of-date matrix, and for every body the
astrometric and apparent geocentric vectors and the true ecliptic longitude/latitude of
date, exactly as the reference implementation computes them. It also samples ΔT far
outside the daily IERS table to cover the long-term splines.

Writes tests/fixtures/reduction.json.
"""

import json
import math
import sys
from pathlib import Path

import numpy as np
import skyfield
from skyfield.api import Loader
from skyfield.framelib import ecliptic_frame

ROOT = Path(__file__).resolve().parents[1]

BODIES = [
    ("Sun", "sun"),
    ("Moon", "moon"),
    ("Mercury", "mercury"),
    ("Venus", "venus"),
    ("Mars", "mars barycenter"),
    ("Jupiter", "jupiter barycenter"),
    ("Saturn", "saturn barycenter"),
    ("Uranus", "uranus barycenter"),
    ("Neptune", "neptune barycenter"),
    ("Pluto", "pluto barycenter"),
]


def julian_day(utc: str) -> float:
    """Same arithmetic as the reference implementation's datetime_to_julian_day."""
    date, time = utc.split("T")
    year, month, day = (int(p) for p in date.split("-"))
    hh, mm, ss = time.split(":")
    hour = int(hh) + int(mm) / 60.0 + float(ss) / 3600.0
    if month <= 2:
        year -= 1
        month += 12
    a = math.floor(year / 100)
    b = 2 - a + math.floor(a / 4)
    return (
        math.floor(365.25 * (year + 4716))
        + math.floor(30.6001 * (month + 1))
        + day
        + (hour / 24.0)
        + b
        - 1524.5
    )


def flat(matrix) -> list[float]:
    return [float(v) for v in np.asarray(matrix).ravel()]


def main(kernel: Path) -> None:
    assert skyfield.__version__ == "1.55", skyfield.__version__
    load = Loader(str(kernel.parent))
    ts = load.timescale()
    eph = load(kernel.name)
    earth = eph["earth"]

    natal = json.loads((ROOT / "tests" / "fixtures" / "natal.json").read_text())
    instants = []
    for case in natal:
        jd = julian_day(case["input"]["utc"])
        t = ts.ut1_jd(jd)
        t_plus = ts.ut1_jd(t.ut1 + 1.0 / 24.0)
        d_psi, d_eps = t._nutation_angles_radians
        bodies = {}
        for name, key in BODIES:
            astrometric = earth.at(t).observe(eph[key])
            apparent = astrometric.apparent()
            lat, lon, dist = apparent.ecliptic_latlon(epoch="date")
            _, lon_plus, _ = earth.at(t_plus).observe(eph[key]).apparent().ecliptic_latlon(
                epoch="date"
            )
            bodies[name] = {
                "astrometric_au": flat(astrometric.xyz.au),
                "light_time_days": float(astrometric.light_time),
                "apparent_au": flat(apparent.xyz.au),
                "lon_deg": float(lon.degrees),
                "lat_deg": float(lat.degrees),
                "distance_au": float(dist.au),
                "lon_plus_1h_deg": float(lon_plus.degrees),
            }
        instants.append(
            {
                "id": case["input"]["id"],
                "jd_ut1": jd,
                "whole": float(t.whole),
                "tt_fraction": float(t.tt_fraction),
                "tdb_fraction": float(t.tdb_fraction),
                "gmst_hours": float(t.gmst),
                "gast_hours": float(t.gast),
                "d_psi_rad": float(d_psi),
                "d_eps_rad": float(d_eps),
                "mean_obliquity_rad": float(t._mean_obliquity_radians),
                "precession": flat(t.precession_matrix()),
                "nutation": flat(t.nutation_matrix()),
                "m": flat(t.M),
                "ecliptic": flat(ecliptic_frame.rotation_at(t)),
                "bodies": bodies,
            }
        )

    # ΔT over four millennia, well beyond the daily table (1973-now+1y).
    delta_t = []
    for year in range(-1500, 3001, 25):
        tt = 1721045.0 + year * 365.25
        delta_t.append({"tt": tt, "delta_t": float(ts.delta_t_function(tt))})
    for tt in np.linspace(2441684.5, 2441684.5 + 19744, 97):
        delta_t.append({"tt": float(tt), "delta_t": float(ts.delta_t_function(tt))})

    payload = {
        "generator": "scripts/make_reduction_fixtures.py (Skyfield 1.55)",
        "kernel": kernel.name,
        "instants": instants,
        "delta_t": delta_t,
    }
    out = ROOT / "tests" / "fixtures" / "reduction.json"
    out.write_text(json.dumps(payload, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"wrote {out} ({out.stat().st_size // 1024} KiB, {len(instants)} instants)")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
