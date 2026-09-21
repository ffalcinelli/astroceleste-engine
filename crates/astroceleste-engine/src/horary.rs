//! Horary charts (`charts/horary.py`): planetary day and hour, significators, Moon's
//! applying and separating aspects, void of course, and considerations before judgment.

use serde::Serialize;

use crate::almanac::rise_set;
use crate::chart::{calculate_chart, Chart, ChartRequest, Placement};
use crate::ephemeris::KernelSet;
use crate::error::EngineError;
use crate::instant::UtcInstant;
use crate::pyfloat;

const CHALDEAN_ORDER: [&str; 7] = [
    "Saturn", "Jupiter", "Mars", "Sun", "Venus", "Mercury", "Moon",
];
/// Ruler of each weekday, Monday first (Python `weekday()`).
const DAY_RULERS: [&str; 7] = [
    "Moon", "Mars", "Mercury", "Jupiter", "Venus", "Saturn", "Sun",
];
const SIGNS: [&str; 12] = [
    "Aries",
    "Taurus",
    "Gemini",
    "Cancer",
    "Leo",
    "Virgo",
    "Libra",
    "Scorpio",
    "Sagittarius",
    "Capricorn",
    "Aquarius",
    "Pisces",
];
const PTOLEMAIC: [(f64, &str); 5] = [
    (0.0, "Conjunction"),
    (60.0, "Sextile"),
    (90.0, "Square"),
    (120.0, "Trine"),
    (180.0, "Opposition"),
];

fn traditional_ruler(sign: &str) -> Option<&'static str> {
    Some(match sign {
        "Aries" | "Scorpio" => "Mars",
        "Taurus" | "Libra" => "Venus",
        "Gemini" | "Virgo" => "Mercury",
        "Cancer" => "Moon",
        "Leo" => "Sun",
        "Sagittarius" | "Pisces" => "Jupiter",
        "Capricorn" | "Aquarius" => "Saturn",
        _ => return None,
    })
}

fn modern_ruler(sign: &str) -> Option<&'static str> {
    Some(match sign {
        "Scorpio" => "Pluto",
        "Aquarius" => "Uranus",
        "Pisces" => "Neptune",
        _ => return None,
    })
}

fn element(sign: &str) -> Option<&'static str> {
    Some(match sign {
        "Aries" | "Leo" | "Sagittarius" => "Fire",
        "Taurus" | "Virgo" | "Capricorn" => "Earth",
        "Gemini" | "Libra" | "Aquarius" => "Air",
        "Cancer" | "Scorpio" | "Pisces" => "Water",
        _ => return None,
    })
}

fn triplicity_ruler(element: &str, is_day: bool) -> Option<&'static str> {
    Some(match (element, is_day) {
        ("Fire", true) => "Sun",
        ("Earth", true) => "Venus",
        ("Air", true) => "Saturn",
        ("Water", true) => "Venus",
        ("Fire", false) => "Jupiter",
        ("Earth", false) => "Moon",
        ("Air", false) => "Mercury",
        ("Water", false) => "Mars",
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanetaryHours {
    pub is_day: bool,
    pub day_ruler: &'static str,
    pub hour_ruler: &'static str,
    pub hour_number: i64,
    pub hour_type: &'static str,
    pub sunrise: String,
    pub sunset: String,
}

/// Planetary day and hour at `instant` for a place, from the actual sunrise and sunset.
/// Where they cannot be computed, 06:00 and 18:00 UTC stand in.
pub fn planetary_hours(
    kernels: &KernelSet,
    instant: UtcInstant,
    latitude: f64,
    longitude: f64,
) -> PlanetaryHours {
    let jd_utc = 2_451_545.0 + instant.seconds_since(&UtcInstant::J2000) / 86_400.0;
    let (sunrise, sunset, next_sunrise) = match rise_set(kernels, jd_utc, latitude, longitude) {
        Ok((rise, set, next)) => {
            let at = |jd: f64| UtcInstant::J2000.plus_days(jd - 2_451_545.0);
            (at(rise), at(set), at(next))
        }
        Err(_) => {
            let rise = instant.with_time(6, 0, 0, 0);
            (
                rise,
                instant.with_time(18, 0, 0, 0),
                rise.add_micros(86_400_000_000),
            )
        }
    };

    let is_day = sunrise <= instant && instant <= sunset;
    let day_ruler = DAY_RULERS[sunrise.weekday() as usize];
    let start = CHALDEAN_ORDER.iter().position(|p| *p == day_ruler).unwrap() as i64;

    let (hour_number, offset, hour_type) = if is_day {
        let hour_length = sunset.seconds_since(&sunrise) / 12.0;
        let elapsed = instant.seconds_since(&sunrise);
        let n = (elapsed / hour_length.max(1.0)) as i64 + 1;
        (n.clamp(1, 12), 0, "Day")
    } else {
        let hour_length = next_sunrise.seconds_since(&sunset) / 12.0;
        let elapsed = instant.seconds_since(&sunset);
        let n = if elapsed < 0.0 {
            12
        } else {
            (elapsed / hour_length.max(1.0)) as i64 + 1
        };
        (n.clamp(1, 12), 12, "Night")
    };
    let hour_ruler = CHALDEAN_ORDER[(start + offset + hour_number - 1).rem_euclid(7) as usize];

    PlanetaryHours {
        is_day,
        day_ruler,
        hour_ruler,
        hour_number,
        hour_type,
        sunrise: sunrise.isoformat(),
        sunset: sunset.isoformat(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApplyingAspect {
    pub planet: &'static str,
    pub aspect: &'static str,
    pub degrees_to_exact: f64,
    pub target_sign: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeparatingAspect {
    pub planet: &'static str,
    pub aspect: &'static str,
    pub degrees_ago: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MoonStatus {
    pub void_of_course: bool,
    pub degrees_to_next_sign: f64,
    pub hours_to_next_sign: f64,
    pub next_sign: &'static str,
    pub speed_status: &'static str,
    pub applying_aspects: Vec<ApplyingAspect>,
    pub next_applying_aspect: Option<ApplyingAspect>,
    pub last_aspect: Option<SeparatingAspect>,
    pub separating_aspects: Vec<SeparatingAspect>,
}

/// The Moon's Ptolemaic aspects to the traditional planets before it leaves its sign,
/// and void-of-course status (`analyze_moon_horary`).
pub fn moon_status(moon: &Placement, planets: &[Placement]) -> MoonStatus {
    let moon_lon = moon.ecliptic_longitude;
    let moon_speed = moon.speed;
    let sign_start = (moon_lon / 30.0).floor() * 30.0;
    let to_sign_end = ((moon_lon / 30.0).floor() + 1.0) * 30.0 - moon_lon;
    let from_sign_start = moon_lon - sign_start;
    let speed_abs = if moon_speed.abs() > 1.0 {
        moon_speed.abs()
    } else {
        13.18
    };
    let current = ((moon_lon / 30.0).floor() as i64).rem_euclid(12) as usize;

    let mut applying: Vec<ApplyingAspect> = Vec::new();
    let mut separating: Vec<SeparatingAspect> = Vec::new();
    for p in planets {
        if !matches!(
            p.name,
            "Sun" | "Mercury" | "Venus" | "Mars" | "Jupiter" | "Saturn"
        ) {
            continue;
        }
        for (angle, aspect) in PTOLEMAIC {
            let mut targets = vec![pyfloat::rem(p.ecliptic_longitude + angle, 360.0)];
            if angle != 0.0 && angle != 180.0 {
                targets.push(pyfloat::rem(p.ecliptic_longitude - angle, 360.0));
            }
            for target in targets {
                let ahead = pyfloat::rem(target - moon_lon, 360.0);
                if 0.0 < ahead
                    && ahead <= to_sign_end
                    && moon_speed - p.speed > 0.0
                    && !applying
                        .iter()
                        .any(|a| a.planet == p.name && a.aspect == aspect)
                {
                    applying.push(ApplyingAspect {
                        planet: p.name,
                        aspect,
                        degrees_to_exact: pyfloat::round(ahead, 2),
                        target_sign: moon.sign,
                    });
                }
                let behind = pyfloat::rem(moon_lon - target, 360.0);
                if 0.0 < behind
                    && behind <= from_sign_start
                    && !separating
                        .iter()
                        .any(|s| s.planet == p.name && s.aspect == aspect)
                {
                    separating.push(SeparatingAspect {
                        planet: p.name,
                        aspect,
                        degrees_ago: pyfloat::round(behind, 2),
                    });
                }
            }
        }
    }
    applying.sort_by(|a, b| a.degrees_to_exact.total_cmp(&b.degrees_to_exact));
    separating.sort_by(|a, b| a.degrees_ago.total_cmp(&b.degrees_ago));

    MoonStatus {
        void_of_course: applying.is_empty(),
        degrees_to_next_sign: pyfloat::round(to_sign_end, 2),
        hours_to_next_sign: pyfloat::round(to_sign_end / (speed_abs / 24.0), 1),
        next_sign: SIGNS[(current + 1) % 12],
        speed_status: if moon_speed > 13.5 {
            "swift"
        } else if moon_speed < 12.5 {
            "slow"
        } else {
            "average"
        },
        next_applying_aspect: applying.first().cloned(),
        last_aspect: separating.first().cloned(),
        applying_aspects: applying,
        separating_aspects: separating,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stricture {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoraryData {
    pub planetary_hours: PlanetaryHours,
    pub ascendant_sign: &'static str,
    pub ascendant_degree: String,
    pub traditional_asc_ruler: &'static str,
    pub modern_asc_ruler: &'static str,
    pub is_radical: bool,
    pub strictures: Vec<Stricture>,
    pub moon_status: MoonStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoraryChart {
    #[serde(flatten)]
    pub chart: Chart,
    pub horary_data: HoraryData,
}

/// A chart for the moment of the question, with its horary analysis
/// (`calculate_horary_chart_data`).
pub fn calculate_horary_chart(
    kernels: &KernelSet,
    req: &ChartRequest,
) -> Result<HoraryChart, EngineError> {
    let chart = calculate_chart(kernels, req)?;
    let hours = planetary_hours(kernels, req.instant, req.latitude, req.longitude);

    let by_name = |name: &str| chart.planets.iter().find(|p| p.name == name);
    let (asc_sign, asc_deg, asc_min) = match (by_name("Ascendant"), chart.houses.first()) {
        (Some(asc), _) => (asc.sign, asc.degree, asc.minute),
        (None, Some(h)) => (h.sign, h.degree, h.minute),
        _ => ("Aries", 0, 0),
    };
    let traditional = traditional_ruler(asc_sign).unwrap_or("Mars");
    let modern = modern_ruler(asc_sign).unwrap_or(traditional);

    let mut strictures = Vec::new();
    if asc_deg < 3 {
        strictures.push(Stricture {
            code: "EARLY_ASC",
            severity: "warning",
            message: format!(
                "Ascendant is very early ({asc_deg}° {asc_sign}): The question may be premature, or circumstances are still developing."
            ),
        });
    } else if asc_deg >= 27 {
        strictures.push(Stricture {
            code: "LATE_ASC",
            severity: "warning",
            message: format!(
                "Ascendant is very late ({asc_deg}° {asc_sign}): The situation has already been decided or is out of the querent's control."
            ),
        });
    }
    match by_name("Saturn").map(|s| s.house) {
        Some(1) => strictures.push(Stricture {
            code: "SATURN_IN_1ST",
            severity: "warning",
            message: "Saturn in the 1st House: Querent may be obstructed, anxious, or facing delays."
                .into(),
        }),
        Some(7) => strictures.push(Stricture {
            code: "SATURN_IN_7TH",
            severity: "info",
            message: "Saturn in the 7th House: Astrologer's judgment may be challenged or the matter may be difficult to judge clearly."
                .into(),
        }),
        _ => {}
    }

    let moon =
        by_name("Moon").ok_or_else(|| EngineError::InvalidInput("chart has no Moon".into()))?;
    let moon = moon_status(moon, &chart.planets);
    if moon.void_of_course {
        strictures.push(Stricture {
            code: "MOON_VOC",
            severity: "info",
            message: "Moon is Void of Course: Nothing will come of the matter in question, or no immediate action will yield changes."
                .into(),
        });
    }

    let triplicity = triplicity_ruler(element(asc_sign).unwrap_or("Fire"), hours.is_day);
    let is_radical = hours.hour_ruler == traditional || Some(hours.hour_ruler) == triplicity;

    Ok(HoraryChart {
        horary_data: HoraryData {
            planetary_hours: hours,
            ascendant_sign: asc_sign,
            ascendant_degree: format!("{asc_deg}° {asc_min}' {asc_sign}"),
            traditional_asc_ruler: traditional,
            modern_asc_ruler: modern,
            is_radical,
            strictures,
            moon_status: moon,
        },
        chart,
    })
}
