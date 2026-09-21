"""Build the SPK reader's test data from a full JPL kernel, using jplephem as the oracle.

    uv run --with jplephem --with numpy scripts/make_spk_fixtures.py kernels/de440s.bsp

Writes:
  tests/data/de440s_2000.bsp         a one-year excerpt small enough to commit
  tests/fixtures/spk_reference.json  positions (km) and velocities (km/day) from jplephem,
                                     both inside the excerpt and across the full kernel
"""

import json
import subprocess
import sys
from pathlib import Path

from jplephem.spk import SPK

ROOT = Path(__file__).resolve().parents[1]
EXCERPT = ROOT / "tests" / "data" / "de440s_2000.bsp"
REFERENCE = ROOT / "tests" / "fixtures" / "spk_reference.json"

# JD TDB samples: inside the committed excerpt (year 2000) and spread over 1849-2150.
EXCERPT_JDS = [2451545.0, 2451545.123456789, 2451600.5, 2451700.25, 2451800.75, 2451899.9]
FULL_JDS = [2396770.5, 2415020.0, 2433282.5, 2440587.5, 2460000.5, 2488069.5, 2506330.5]


def sample(kernel: SPK, jds: list[float]) -> list[dict]:
    rows = []
    for segment in kernel.segments:
        for jd in jds:
            position, velocity = segment.compute_and_differentiate(jd)
            rows.append(
                {
                    "center": segment.center,
                    "target": segment.target,
                    "jd_tdb": jd,
                    "position_km": [float(v) for v in position],
                    "velocity_km_per_day": [float(v) for v in velocity],
                }
            )
    return rows


def main(full_kernel: Path) -> None:
    EXCERPT.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [sys.executable, "-m", "jplephem", "excerpt", "2000/1/1", "2001/1/1", str(full_kernel), str(EXCERPT)],
        check=True,
    )
    with SPK.open(str(full_kernel)) as full, SPK.open(str(EXCERPT)) as excerpt:
        payload = {
            "generator": "scripts/make_spk_fixtures.py (jplephem)",
            "kernel": full_kernel.name,
            "excerpt": sample(excerpt, EXCERPT_JDS),
            "full": sample(full, FULL_JDS),
        }
    REFERENCE.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    print(f"wrote {EXCERPT} ({EXCERPT.stat().st_size // 1024} KiB) and {REFERENCE}")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
