//! Fixed stars conjunct chart points (`charts/calc/fixed_stars.py`).

use serde::Serialize;

use crate::aspects::{Aspect, Point};
use crate::catalog::FIXED_STARS;
use crate::pyfloat;
use crate::symbolic::symbolic_degree_number;
use crate::zodiac::{determine_house, longitude_to_zodiac, PRECESSION_RATE_ARCSEC_YEAR};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FixedStarPosition {
    pub name: &'static str,
    pub symbol: &'static str,
    pub sign: &'static str,
    pub sign_symbol: &'static str,
    pub degree: i64,
    pub minute: i64,
    pub ecliptic_longitude: f64,
    pub ecliptic_latitude: f64,
    pub house: u8,
    pub magnitude: f64,
    pub symbolic_degree: i64,
}

/// Stars within `max_orb` of a chart point, and those conjunctions. Positions are the
/// J2000 longitudes precessed at the mean rate; a star is listed only when conjunct.
pub fn fixed_stars(
    jd: f64,
    cusps: &[f64],
    points: &[Point],
    max_orb: f64,
    sidereal_shift: Option<f64>,
) -> (Vec<FixedStarPosition>, Vec<Aspect>) {
    let years = (jd - 2_451_545.0) / 365.25;
    let precession_deg = (years * PRECESSION_RATE_ARCSEC_YEAR) / 3600.0;
    let mut stars = Vec::new();
    let mut aspects = Vec::new();

    for star in &FIXED_STARS {
        let mut lon = pyfloat::rem(star.j2000_lon + precession_deg, 360.0);
        if let Some(ayanamsa) = sidereal_shift {
            lon = pyfloat::rem(pyfloat::rem(lon - ayanamsa, 360.0) + 360.0, 360.0);
        }
        let mut conjunct = false;
        for body in points {
            let mut diff = pyfloat::rem((lon - body.longitude).abs(), 360.0);
            if diff > 180.0 {
                diff = 360.0 - diff;
            }
            let orb = (diff - 0.0).abs();
            if orb <= max_orb {
                conjunct = true;
                aspects.push(Aspect {
                    body1: star.name.to_string(),
                    body2: body.name.to_string(),
                    aspect_type: "Conjunction",
                    symbol: "☌",
                    angle: 0.0,
                    orb: pyfloat::round(orb, 2),
                    max_orb: pyfloat::round(max_orb, 2),
                    is_major: None,
                    is_applying: true,
                });
            }
        }
        if conjunct {
            let z = longitude_to_zodiac(lon);
            stars.push(FixedStarPosition {
                name: star.name,
                symbol: star.symbol,
                sign: z.sign.name,
                sign_symbol: z.sign.symbol,
                degree: z.degree,
                minute: z.minute,
                ecliptic_longitude: lon,
                ecliptic_latitude: star.j2000_lat,
                house: determine_house(lon, cusps),
                magnitude: star.magnitude,
                symbolic_degree: symbolic_degree_number(z.degree, z.minute),
            });
        }
    }
    (stars, aspects)
}
