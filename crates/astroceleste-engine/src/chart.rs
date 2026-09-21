//! A complete chart: positions, houses, aspects, fixed stars, lots, temperament and
//! lunar status, in the JSON shape of the reference's `calculate_chart_data`.

use serde::{Serialize, Serializer};
use serde_json::Value;

use crate::aspects::{natal_aspects, Aspect, OrbSettings, Point};
use crate::ephemeris::KernelSet;
use crate::error::EngineError;
use crate::fixed_stars::{fixed_stars, FixedStarPosition};
use crate::houses::{calculate_houses, HouseSystem};
use crate::instant::UtcInstant;
use crate::lots::{arabic_parts, Lot};
use crate::lunar::{lunar_status, LunarStatus};
use crate::planets::calculate_planets;
use crate::symbolic::symbolic_degree_number;
use crate::temperament::{temperament, Temperament};
use crate::zodiac::{
    ayanamsa, ayanamsa_info, determine_house, longitude_to_zodiac, DEFAULT_AYANAMSA,
};

/// Every body a complete chart carries; missing ones are reported in
/// `unavailable_bodies` rather than silently dropped.
const EXPECTED_BODIES: [&str; 16] = [
    "Sun",
    "Moon",
    "Mercury",
    "Venus",
    "Mars",
    "Jupiter",
    "Saturn",
    "Uranus",
    "Neptune",
    "Pluto",
    "North Node",
    "South Node",
    "Chiron",
    "Lilith",
    "Ascendant",
    "Midheaven",
];

/// What to compute.
#[derive(Debug, Clone)]
pub struct ChartRequest<'a> {
    pub instant: UtcInstant,
    pub latitude: f64,
    pub longitude: f64,
    /// House system code, echoed as given; its first letter selects the system.
    pub house_system: &'a str,
    /// "tropical" or "sidereal" (anything else is tropical).
    pub zodiac_type: &'a str,
    /// Ayanamsa code for sidereal charts (unknown codes use the default).
    pub ayanamsa: &'a str,
    /// Caller orb settings, merged over the defaults.
    pub orb_settings: Option<&'a Value>,
}

impl<'a> ChartRequest<'a> {
    pub fn new(instant: UtcInstant, latitude: f64, longitude: f64) -> Self {
        ChartRequest {
            instant,
            latitude,
            longitude,
            house_system: "P",
            zodiac_type: "tropical",
            ayanamsa: DEFAULT_AYANAMSA,
            orb_settings: None,
        }
    }
}

/// A planet, lunar point or angle placed in the chart.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Placement {
    pub name: &'static str,
    pub symbol: &'static str,
    pub sign: &'static str,
    pub sign_symbol: &'static str,
    pub degree: i64,
    pub minute: i64,
    pub ecliptic_longitude: f64,
    pub house: u8,
    pub speed: f64,
    pub is_retrograde: bool,
    pub symbolic_degree: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HouseCusp {
    pub house_number: u8,
    pub sign: &'static str,
    pub sign_symbol: &'static str,
    pub degree: i64,
    pub minute: i64,
    pub ecliptic_longitude: f64,
    pub symbolic_degree: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chart {
    pub house_system: String,
    pub zodiac_type: &'static str,
    pub ayanamsa: Option<&'static str>,
    pub ayanamsa_name: Option<&'static str>,
    pub ayanamsa_value: Option<f64>,
    pub ayanamsa_formatted: Option<String>,
    pub precession_rate_arcsec_yr: f64,
    pub orb_settings: Value,
    pub planets: Vec<Placement>,
    pub unavailable_bodies: Vec<&'static str>,
    pub houses: Vec<HouseCusp>,
    pub aspects: Vec<Aspect>,
    pub fixed_stars: Vec<FixedStarPosition>,
    pub arabic_parts: Vec<Lot>,
    pub temperament: Temperament,
    #[serde(serialize_with = "empty_object_if_none")]
    pub lunar_status: Option<LunarStatus>,
}

fn empty_object_if_none<S: Serializer>(v: &Option<LunarStatus>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(status) => status.serialize(s),
        None => serde_json::Map::new().serialize(s),
    }
}

impl Chart {
    /// The chart's planets and angles as named ecliptic points.
    pub fn points(&self) -> Vec<Point<'_>> {
        self.planets
            .iter()
            .map(|p| Point {
                name: p.name,
                longitude: p.ecliptic_longitude,
            })
            .collect()
    }
}

fn symbol_of(name: &str) -> &'static str {
    match name {
        "Sun" => "☉",
        "Moon" => "☽",
        "Mercury" => "☿",
        "Venus" => "♀",
        "Mars" => "♂",
        "Jupiter" => "♃",
        "Saturn" => "♄",
        "Uranus" => "♅",
        "Neptune" => "♆",
        "Pluto" => "♇",
        "North Node" => "☊",
        "South Node" => "☋",
        "Chiron" => "⚷",
        "Lilith" => "⚸",
        "Ascendant" => "ASC",
        "Midheaven" => "MC",
        _ => "",
    }
}

/// Compute a chart (`calculate_chart_data`).
pub fn calculate_chart(kernels: &KernelSet, req: &ChartRequest) -> Result<Chart, EngineError> {
    let zodiac_type = match req.zodiac_type.trim().to_lowercase().as_str() {
        "sidereal" => "sidereal",
        _ => "tropical",
    };
    let is_sidereal = zodiac_type == "sidereal";
    let ayanamsa_code = ayanamsa(req.ayanamsa).code;
    let orbs = OrbSettings::merge(req.orb_settings)?;

    let jd = req.instant.julian_day();
    let info = ayanamsa_info(jd, ayanamsa_code);
    let shift = if is_sidereal { info.value } else { 0.0 };

    let engine = calculate_planets(kernels, jd, shift)?;
    let houses = calculate_houses(
        jd,
        req.latitude,
        req.longitude,
        HouseSystem::from_code(req.house_system),
        shift,
    );
    let cusps = houses.cusps.to_vec();

    let house_cusps: Vec<HouseCusp> = houses
        .cusps
        .iter()
        .enumerate()
        .map(|(i, &lon)| {
            let z = longitude_to_zodiac(lon);
            HouseCusp {
                house_number: i as u8 + 1,
                sign: z.sign.name,
                sign_symbol: z.sign.symbol,
                degree: z.degree,
                minute: z.minute,
                ecliptic_longitude: lon,
                symbolic_degree: symbolic_degree_number(z.degree, z.minute),
            }
        })
        .collect();

    let mut raw: Vec<(&'static str, f64, f64, bool)> = engine
        .bodies
        .iter()
        .map(|b| (b.name, b.longitude, b.speed, b.is_retrograde))
        .collect();
    raw.push(("Ascendant", houses.ascendant, 0.0, false));
    raw.push(("Midheaven", houses.midheaven, 0.0, false));

    let planets: Vec<Placement> = raw
        .iter()
        .map(|&(name, lon, speed, is_retrograde)| {
            let z = longitude_to_zodiac(lon);
            Placement {
                name,
                symbol: symbol_of(name),
                sign: z.sign.name,
                sign_symbol: z.sign.symbol,
                degree: z.degree,
                minute: z.minute,
                ecliptic_longitude: lon,
                house: determine_house(lon, &cusps),
                speed,
                is_retrograde,
                symbolic_degree: symbolic_degree_number(z.degree, z.minute),
            }
        })
        .collect();
    let points: Vec<Point> = planets
        .iter()
        .map(|p| Point {
            name: p.name,
            longitude: p.ecliptic_longitude,
        })
        .collect();

    let mut aspects = natal_aspects(&points, &orbs)?;
    let (stars, star_aspects) = fixed_stars(
        jd,
        &cusps,
        &points,
        orbs.fixed_star_orb()?,
        is_sidereal.then_some(info.value),
    );
    aspects.extend(star_aspects);

    let lots = arabic_parts(&points, &cusps);
    let temperament = temperament(&planets, &aspects);
    let lunar = lunar_status(&planets);

    let mut unavailable: Vec<&'static str> = EXPECTED_BODIES
        .iter()
        .copied()
        .filter(|name| !planets.iter().any(|p| p.name == *name))
        .collect();
    unavailable.sort_unstable();

    Ok(Chart {
        house_system: req.house_system.to_string(),
        zodiac_type,
        ayanamsa: is_sidereal.then_some(ayanamsa_code),
        ayanamsa_name: is_sidereal.then_some(info.name),
        ayanamsa_value: is_sidereal.then_some(info.value),
        ayanamsa_formatted: is_sidereal.then(|| info.formatted.clone()),
        precession_rate_arcsec_yr: info.precession_rate_arcsec_yr,
        orb_settings: orbs.to_value(),
        planets,
        unavailable_bodies: unavailable,
        houses: house_cusps,
        aspects,
        fixed_stars: stars,
        arabic_parts: lots,
        temperament,
        lunar_status: lunar,
    })
}
