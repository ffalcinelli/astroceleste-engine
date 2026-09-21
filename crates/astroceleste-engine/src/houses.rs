//! House cusps, Ascendant and Midheaven (`SkyfieldEngine.calculate_houses`):
//! Placidus (default), Whole Sign, Equal and Porphyry.

use std::f64::consts::PI;

use crate::frames::Orientation;
use crate::pyfloat;
use crate::time::Time;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HouseSystem {
    Placidus,
    WholeSign,
    Equal,
    Porphyry,
}

impl HouseSystem {
    /// From the first letter of the code, like the reference: anything unknown is Placidus.
    pub fn from_code(code: &str) -> Self {
        match code.trim().chars().next().map(|c| c.to_ascii_uppercase()) {
            Some('W') => HouseSystem::WholeSign,
            Some('E') => HouseSystem::Equal,
            Some('O') => HouseSystem::Porphyry,
            _ => HouseSystem::Placidus,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Houses {
    /// Cusps 1-12, degrees.
    pub cusps: [f64; 12],
    pub ascendant: f64,
    pub midheaven: f64,
}

/// Houses at UT1 Julian date `jd` for geographic `lat`/`lon` (degrees, east positive).
pub fn calculate_houses(jd: f64, lat: f64, lon: f64, system: HouseSystem, shift: f64) -> Houses {
    let t = Time::from_ut1(jd);
    let gast_hours = Orientation::at(&t).gast_hours;
    let ramc = pyfloat::rem(pyfloat::rem(gast_hours * 15.0 + lon, 360.0) + 360.0, 360.0);
    let ramc_r = ramc.to_radians();
    let lat_r = lat.to_radians();

    // Obliquity polynomial of the reference implementation.
    let t_cent = (jd - 2_451_545.0) / 36525.0;
    let eps_deg = 23.4392911 - 0.0130042 * t_cent - 0.00000016 * t_cent.powf(2.0)
        + 0.000000504 * t_cent.powf(3.0);
    let eps_r = eps_deg.to_radians();

    let mc_deg = pyfloat::rem(
        ramc_r.sin().atan2(ramc_r.cos() * eps_r.cos()).to_degrees(),
        360.0,
    );
    let y = ramc_r.cos();
    let x = -ramc_r.sin() * eps_r.cos() - lat_r.tan() * eps_r.sin();
    let asc_deg = pyfloat::rem(y.atan2(x).to_degrees(), 360.0);

    let mut c = [0.0_f64; 13]; // 1-indexed
    match system {
        HouseSystem::WholeSign => {
            let asc_sign = pyfloat::floordiv(asc_deg, 30.0);
            for (i, cusp) in c.iter_mut().enumerate().skip(1) {
                *cusp = pyfloat::rem((asc_sign + i as f64 - 1.0) * 30.0, 360.0);
            }
        }
        HouseSystem::Equal => {
            for (i, cusp) in c.iter_mut().enumerate().skip(1) {
                *cusp = pyfloat::rem(asc_deg + (i as f64 - 1.0) * 30.0, 360.0);
            }
        }
        HouseSystem::Porphyry => {
            set_angles(&mut c, asc_deg, mc_deg);
            let q1 = pyfloat::rem(asc_deg - mc_deg, 360.0);
            c[11] = pyfloat::rem(mc_deg + q1 / 3.0, 360.0);
            c[12] = pyfloat::rem(mc_deg + (2.0 * q1) / 3.0, 360.0);
            let q2 = pyfloat::rem(c[4] - asc_deg, 360.0);
            c[2] = pyfloat::rem(asc_deg + q2 / 3.0, 360.0);
            c[3] = pyfloat::rem(asc_deg + (2.0 * q2) / 3.0, 360.0);
            // c[i - 6] with i = 5, 6 reads c[-1], c[0] in Python: c[12] and c[0] (= 0.0).
            c[5] = pyfloat::rem(c[12] + 180.0, 360.0);
            c[6] = pyfloat::rem(c[0] + 180.0, 360.0);
            c[8] = pyfloat::rem(c[2] + 180.0, 360.0);
            c[9] = pyfloat::rem(c[3] + 180.0, 360.0);
        }
        HouseSystem::Placidus => {
            set_angles(&mut c, asc_deg, mc_deg);
            let solve = |offset: f64, semi_factor: f64| {
                let target_ra = pyfloat::rem(ramc + offset, 360.0).to_radians();
                let mut lon = pyfloat::rem(ramc + offset, 360.0);
                for _ in 0..25 {
                    let l_r = lon.to_radians();
                    let sin_d = eps_r.sin() * l_r.sin();
                    let decl = sin_d.clamp(-1.0, 1.0).asin();
                    let ra = pyfloat::rem((eps_r.cos() * l_r.sin()).atan2(l_r.cos()), 2.0 * PI);
                    let tan_d = decl.tan() * lat_r.tan();
                    if tan_d.abs() >= 1.0 {
                        break;
                    }
                    let ad = tan_d.asin();
                    let diff = (ra - target_ra) - semi_factor * ad;
                    let diff = pyfloat::rem(diff + PI, 2.0 * PI) - PI;
                    if diff.abs() < 1e-8 {
                        break;
                    }
                    lon = pyfloat::rem(lon - diff.to_degrees(), 360.0);
                }
                lon
            };
            c[11] = solve(30.0, 1.0 / 3.0);
            c[12] = solve(60.0, 2.0 / 3.0);
            c[9] = solve(-30.0, -1.0 / 3.0);
            c[8] = solve(-60.0, -2.0 / 3.0);
            c[5] = pyfloat::rem(c[11] + 180.0, 360.0);
            c[6] = pyfloat::rem(c[12] + 180.0, 360.0);
            c[2] = pyfloat::rem(c[8] + 180.0, 360.0);
            c[3] = pyfloat::rem(c[9] + 180.0, 360.0);
        }
    }

    let mut cusps = [0.0; 12];
    for i in 0..12 {
        cusps[i] = pyfloat::rem(c[i + 1] - shift, 360.0);
    }
    Houses {
        cusps,
        ascendant: pyfloat::rem(asc_deg - shift, 360.0),
        midheaven: pyfloat::rem(mc_deg - shift, 360.0),
    }
}

fn set_angles(c: &mut [f64; 13], asc: f64, mc: f64) {
    c[1] = asc;
    c[10] = mc;
    c[7] = pyfloat::rem(asc + 180.0, 360.0);
    c[4] = pyfloat::rem(mc + 180.0, 360.0);
}
