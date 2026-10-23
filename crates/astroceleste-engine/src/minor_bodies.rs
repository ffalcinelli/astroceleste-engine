//! Chiron, the main asteroids and the dwarf planets from precomputed geocentric longitude
//! tables (1849-2151), with Lagrange interpolation (cubic for Chiron, quintic for the
//! others, whose tables are sampled more sparsely). Outside its table a body is
//! reported unavailable rather than extrapolated: these orbits are perturbed by the giant
//! planets and have no honest closed form.
//!
//! Each table holds apparent geocentric longitudes in the ecliptic of date, unwrapped, in
//! radians, after a header of `jd_start` (f64), `step` in days (f64) and the sample count
//! (u32), little-endian. Chiron's samples are f64, indexed by UT1 Julian date (it predates
//! the others and its fixtures are frozen). The others, generated from JPL Horizons by
//! `scripts/fetch_minor_bodies.py`, are indexed by TT and store the first sample (f64) then
//! the differences between consecutive samples as i32 in units of [`DELTA_UNIT`] radians.

use std::sync::OnceLock;

use crate::pyfloat;

struct Table {
    jd_start: f64,
    step: f64,
    lons_unwrapped: Vec<f64>,
}

/// Radians per unit of the stored differences (0.00002 arcseconds).
const DELTA_UNIT: f64 = 1e-10;

impl Table {
    /// A table of f64 samples (`chiron.bin`).
    fn parse(bytes: &[u8]) -> Table {
        let f = |i: usize| f64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
        let n = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        Table {
            jd_start: f(0),
            step: f(8),
            lons_unwrapped: (0..n).map(|i| f(20 + 8 * i)).collect(),
        }
    }

    /// A table of a first sample and i32 differences (`data/minor/*.bin`).
    fn parse_deltas(bytes: &[u8]) -> Table {
        let f = |i: usize| f64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
        let n = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
        let first = f(20);
        let mut units: i64 = 0;
        let mut lons_unwrapped = Vec::with_capacity(n);
        lons_unwrapped.push(first);
        for chunk in bytes[28..28 + 4 * (n - 1)].chunks_exact(4) {
            units += i64::from(i32::from_le_bytes(chunk.try_into().unwrap()));
            lons_unwrapped.push(first + units as f64 * DELTA_UNIT);
        }
        Table {
            jd_start: f(0),
            step: f(8),
            lons_unwrapped,
        }
    }

    /// Position at `jd` (the table's time scale) by cubic Lagrange interpolation (Chiron),
    /// or `None` outside the table.
    fn at(&self, jd: f64, shift: f64) -> Option<TablePosition> {
        let y = &self.lons_unwrapped;
        let step = self.step;
        let idx = ((jd - self.jd_start) / step) as i64;
        if !(1 <= idx && (idx as usize) < y.len() - 2) {
            return None;
        }
        let i = idx as usize;
        let t = (jd - (self.jd_start + idx as f64 * step)) / step;
        let (t2, t3) = (t.powf(2.0), t.powf(3.0));
        let c0 = -0.5 * t + t2 - 0.5 * t3;
        let c1 = 1.0 - 2.5 * t2 + 1.5 * t3;
        let c2 = 0.5 * t + 2.0 * t2 - 1.5 * t3;
        let c3 = -0.5 * t2 + 0.5 * t3;
        let dc0 = (-0.5 + 2.0 * t - 1.5 * t2) / step;
        let dc1 = (-5.0 * t + 4.5 * t2) / step;
        let dc2 = (0.5 + 4.0 * t - 4.5 * t2) / step;
        let dc3 = (-t + 1.5 * t2) / step;
        let (y0, y1, y2, y3) = (y[i - 1], y[i], y[i + 1], y[i + 2]);
        let lon_rad = c0 * y0 + c1 * y1 + c2 * y2 + c3 * y3;
        let speed_rad_day = dc0 * y0 + dc1 * y1 + dc2 * y2 + dc3 * y3;
        Some(TablePosition::new(lon_rad, speed_rad_day, shift))
    }

    /// Position at `jd` (the table's time scale) by 6-point Lagrange interpolation through
    /// the samples two before to three after it, or `None` outside the table.
    fn at_quintic(&self, jd: f64, shift: f64) -> Option<TablePosition> {
        const NODES: [f64; 6] = [-2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
        let y = &self.lons_unwrapped;
        let idx = ((jd - self.jd_start) / self.step).floor();
        if !(2.0 <= idx && idx + 3.0 < y.len() as f64) {
            return None;
        }
        let i = idx as usize;
        let t = (jd - (self.jd_start + idx * self.step)) / self.step;
        let (mut lon_rad, mut rate) = (0.0, 0.0);
        for (j, &xj) in NODES.iter().enumerate() {
            let others = || NODES.iter().enumerate().filter(move |&(k, _)| k != j);
            let denominator: f64 = others().map(|(_, &xk)| xj - xk).product();
            let weight: f64 = others().map(|(_, &xk)| t - xk).product();
            // d/dt of the product: the sum of the products leaving out one factor each.
            let slope: f64 = others()
                .map(|(m, _)| {
                    others()
                        .filter(|&(k, _)| k != m)
                        .map(|(_, &xk)| t - xk)
                        .product::<f64>()
                })
                .sum();
            let sample = y[i - 2 + j];
            lon_rad += weight / denominator * sample;
            rate += slope / denominator * sample;
        }
        Some(TablePosition::new(lon_rad, rate / self.step, shift))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TablePosition {
    pub longitude: f64,
    /// Degrees per day, rounded to 4 decimals.
    pub speed: f64,
    pub is_retrograde: bool,
}

impl TablePosition {
    fn new(lon_rad: f64, speed_rad_day: f64, shift: f64) -> Self {
        let speed = speed_rad_day.to_degrees();
        TablePosition {
            longitude: pyfloat::rem(lon_rad.to_degrees() - shift, 360.0),
            speed: pyfloat::round(speed, 4),
            is_retrograde: speed < 0.0,
        }
    }
}

/// Chiron's longitude (shifted by `shift` degrees for sidereal charts) at a UT1 Julian date.
pub fn chiron(jd: f64, shift: f64) -> Option<TablePosition> {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE
        .get_or_init(|| Table::parse(include_bytes!("data/chiron.bin")))
        .at(jd, shift)
}

macro_rules! tables {
    ($($name:literal => $file:literal),* $(,)?) => {
        /// The bodies with a Horizons table, in chart order.
        pub const MINOR_BODIES: [&str; [$($name),*].len()] = [$($name),*];

        fn table(name: &str) -> Option<&'static Table> {
            match name {
                $($name => {
                    static TABLE: OnceLock<Table> = OnceLock::new();
                    Some(TABLE.get_or_init(|| {
                        Table::parse_deltas(include_bytes!(concat!("data/minor/", $file)))
                    }))
                })*
                _ => None,
            }
        }
    };
}

tables! {
    "Ceres" => "ceres.bin",
    "Pallas" => "pallas.bin",
    "Juno" => "juno.bin",
    "Vesta" => "vesta.bin",
    "Astraea" => "astraea.bin",
    "Eris" => "eris.bin",
    "Sedna" => "sedna.bin",
    "Makemake" => "makemake.bin",
    "Orcus" => "orcus.bin",
    "Ixion" => "ixion.bin",
}

/// Whether `name` is one of [`MINOR_BODIES`].
pub fn is_minor_body(name: &str) -> bool {
    MINOR_BODIES.contains(&name)
}

/// The glyph of a minor body ("" for those without a Unicode symbol).
pub fn minor_body_symbol(name: &str) -> &'static str {
    match name {
        "Ceres" => "⚳",
        "Pallas" => "⚴",
        "Juno" => "⚵",
        "Vesta" => "⚶",
        _ => "",
    }
}

/// A minor body's longitude (shifted by `shift` degrees for sidereal charts) at a TT
/// Julian date, or `None` outside its table or for an unknown name.
pub fn minor_body(name: &str, jd_tt: f64, shift: f64) -> Option<TablePosition> {
    table(name)?.at_quintic(jd_tt, shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Horizons longitudes between the samples (`tests/fixtures/minor_bodies.json`).
    #[test]
    fn tables_match_horizons() {
        let text = include_str!("../../../tests/fixtures/minor_bodies.json");
        let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
        for name in MINOR_BODIES {
            let points = fixture[name].as_array().unwrap();
            assert!(points.len() > 250, "{name}: {} points", points.len());
            let mut worst: f64 = 0.0;
            for p in points {
                let (jd, expected) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
                let got = minor_body(name, jd, 0.0).unwrap().longitude;
                let d = (pyfloat::rem(got - expected + 180.0, 360.0) - 180.0).abs() * 3600.0;
                worst = worst.max(d);
            }
            assert!(worst < 3.0, "{name}: {worst}\" from Horizons");
        }
    }

    #[test]
    fn tables_cover_1849_to_2151_only() {
        let (jan_1850, dec_2150) = (2_396_758.5, 2_506_696.5);
        for name in MINOR_BODIES {
            assert!(minor_body(name, jan_1850, 0.0).is_some(), "{name} in 1850");
            assert!(minor_body(name, dec_2150, 0.0).is_some(), "{name} in 2150");
            assert!(
                minor_body(name, 2_396_000.0, 0.0).is_none(),
                "{name} in 1847"
            );
            assert!(
                minor_body(name, 2_507_500.0, 0.0).is_none(),
                "{name} in 2153"
            );
        }
        // Chiron has a table of its own, read through [`chiron`].
        assert!(minor_body("Chiron", jan_1850, 0.0).is_none());
    }

    #[test]
    fn sidereal_shift_and_speed() {
        let jd = 2_451_545.0;
        let tropical = minor_body("Ceres", jd, 0.0).unwrap();
        let sidereal = minor_body("Ceres", jd, 24.0).unwrap();
        assert!((pyfloat::rem(tropical.longitude - sidereal.longitude, 360.0) - 24.0).abs() < 1e-9);
        // Ceres near 184.45° on 2000-01-01, moving direct at about 0.22°/day (Horizons).
        assert!(
            (tropical.longitude - 184.4528993).abs() < 1e-4,
            "{tropical:?}"
        );
        assert!(tropical.speed > 0.2 && tropical.speed < 0.23 && !tropical.is_retrograde);
    }
}
