//! Lunar phase, illumination, speed, dignity and mansion (`charts/calc/lunar.py`).

use serde::Serialize;

use crate::catalog::{LunarMansion, LUNAR_MANSIONS};
use crate::chart::Placement;
use crate::pyfloat;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LunarStatus {
    pub phase_key: &'static str,
    pub phase_name: &'static str,
    pub phase_quarter: u8,
    pub glyph: &'static str,
    pub elongation: f64,
    pub illumination_percentage: f64,
    pub moon_age_days: f64,
    pub is_waxing: bool,
    pub moon_sign: &'static str,
    pub moon_degree: i64,
    pub moon_minute: i64,
    pub moon_longitude: f64,
    pub moon_house: u8,
    pub moon_speed: f64,
    pub speed_status: &'static str,
    pub essential_dignity: &'static str,
    pub lunar_mansion: LunarMansion,
}

/// Lunar status from the chart's planets; `None` when there is no Moon.
pub fn lunar_status(planets: &[Placement]) -> Option<LunarStatus> {
    let moon = planets.iter().find(|p| p.name == "Moon")?;
    let sun_lon = planets
        .iter()
        .find(|p| p.name == "Sun")
        .map_or(0.0, |s| s.ecliptic_longitude);
    let moon_lon = moon.ecliptic_longitude;
    let diff = pyfloat::rem(moon_lon - sun_lon, 360.0);

    let (phase_key, phase_name, phase_quarter, glyph) = if !(22.5..337.5).contains(&diff) {
        ("new_moon", "New Moon", 1, "🌑")
    } else if diff < 67.5 {
        ("waxing_crescent", "Waxing Crescent", 1, "🌒")
    } else if diff < 112.5 {
        ("first_quarter", "First Quarter", 2, "🌓")
    } else if diff < 157.5 {
        ("waxing_gibbous", "Waxing Gibbous", 2, "🌔")
    } else if diff < 202.5 {
        ("full_moon", "Full Moon", 3, "🌕")
    } else if diff < 247.5 {
        ("waning_gibbous", "Waning Gibbous", 3, "🌖")
    } else if diff < 292.5 {
        ("third_quarter", "Third Quarter", 4, "🌗")
    } else {
        ("waning_crescent", "Waning Crescent", 4, "🌘")
    };

    let speed = moon.speed;
    let mansion_index = ((moon_lon / (360.0 / 28.0)).floor() as i64).rem_euclid(28) as usize;
    Some(LunarStatus {
        phase_key,
        phase_name,
        phase_quarter,
        glyph,
        elongation: pyfloat::round(diff, 2),
        illumination_percentage: pyfloat::round(((1.0 - diff.to_radians().cos()) / 2.0) * 100.0, 1),
        moon_age_days: pyfloat::round((diff / 360.0) * 29.530588853, 1),
        is_waxing: diff < 180.0,
        moon_sign: moon.sign,
        moon_degree: moon.degree,
        moon_minute: moon.minute,
        moon_longitude: pyfloat::round(moon_lon, 3),
        moon_house: moon.house,
        moon_speed: pyfloat::round(speed, 2),
        speed_status: if speed > 13.5 {
            "swift"
        } else if speed < 12.5 {
            "slow"
        } else {
            "average"
        },
        essential_dignity: match moon.sign {
            "Cancer" => "Domicile",
            "Taurus" => "Exaltation",
            "Capricorn" => "Detriment",
            "Scorpio" => "Fall",
            _ => "Peregrine",
        },
        lunar_mansion: LUNAR_MANSIONS[mansion_index],
    })
}
