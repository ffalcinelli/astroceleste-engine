//! Electional astrology: how well a moment suits beginning something, and a search over a
//! span of time for the best moments at a place.
//!
//! A moment is assessed with the traditional electional rules (Bonatti, Lilly and the
//! Arabic authors after Sahl): the Moon's condition and the next aspect she perfects, the
//! Ascendant and its ruler, the qualities of the Moon's and the Ascendant's degrees (Lilly), benefics and malefics on the angles, the retrogradation of the
//! planets the matter needs, the ruler of the house of the matter, the planetary hour and,
//! when a natal chart is given, the election's contacts with it. Each rule that applies is
//! an [`ElectionFactor`] with a stable code and a signed weight; the score is 50 plus the
//! weights, between 0 and 100. Explanations are left to the caller, keyed by code.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::almanac::SunEvents;
use crate::chart::{calculate_chart, placements, sky, Chart, ChartRequest, Placement};
use crate::degree_qualities::degree_qualities;
use crate::ephemeris::KernelSet;
use crate::error::EngineError;
use crate::horary::{
    hours_in, jd_utc, moon_status, planetary_hours, solar_day, traditional_ruler, MoonStatus,
    PlanetaryHours, SolarDay, SIGNS,
};
use crate::houses::{houses_at_sidereal_time, HouseSystem};
use crate::instant::UtcInstant;
use crate::planets::{planets_and_sidereal_time, require_kernel, BodyPosition};
use crate::pyfloat;
use crate::zodiac::{ayanamsa, ayanamsa_info};

/// Longest span a search covers, in days.
pub const MAX_SEARCH_DAYS: f64 = 92.0;
/// Scores from here up are favourable.
const FAVOURABLE: f64 = 65.0;
/// Scores from here up (and below [`FAVOURABLE`]) are mixed; below, unfavourable.
const MIXED: f64 = 45.0;
/// A planet this close to the Sun (degrees) is combust, unless cazimi.
const COMBUST_ORB: f64 = 8.5;
/// Within 17 arc minutes of the Sun a planet is cazimi, in the heart of the Sun.
const CAZIMI_ORB: f64 = 17.0 / 60.0;
/// Orb (degrees) of the election's contacts with natal points.
const NATAL_ORB: f64 = 3.0;
/// Below this latitude, where the Sun rises and sets every day, a search that finds
/// sunrises day by day reuses each day for all its moments.
const SOLAR_DAY_CACHE_MAX_LAT: f64 = 60.0;

const BENEFICS: [&str; 2] = ["Venus", "Jupiter"];
const MALEFICS: [&str; 2] = ["Mars", "Saturn"];

/// What the election is for. Each purpose has a house of the matter, a natural
/// significator and the planetary hours that favour it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// No particular matter: the general rules only.
    #[default]
    General,
    /// Signing a contract or an agreement (7th house, Mercury).
    Contract,
    /// Marriage, engagement or partnership (7th house, Venus).
    Partnership,
    /// Setting out on a journey (9th house, Mercury).
    Travel,
    /// Beginning a treatment or a surgery (1st house, the Sun).
    Health,
    /// Buying, investing or borrowing (2nd house, Jupiter).
    Finance,
    /// Opening a business or launching a venture (10th house, Jupiter).
    Launch,
    /// Starting a job or asking for a promotion (10th house, the Sun).
    Career,
}

impl Purpose {
    /// House of the matter.
    fn house(self) -> Option<usize> {
        match self {
            Purpose::General => None,
            Purpose::Contract | Purpose::Partnership => Some(7),
            Purpose::Travel => Some(9),
            Purpose::Health => Some(1),
            Purpose::Finance => Some(2),
            Purpose::Launch | Purpose::Career => Some(10),
        }
    }

    /// Natural significator of the matter.
    fn significator(self) -> Option<&'static str> {
        match self {
            Purpose::General => None,
            Purpose::Contract | Purpose::Travel => Some("Mercury"),
            Purpose::Partnership => Some("Venus"),
            Purpose::Health | Purpose::Career => Some("Sun"),
            Purpose::Finance | Purpose::Launch => Some("Jupiter"),
        }
    }

    /// Hour rulers that favour the matter.
    fn hour_rulers(self) -> &'static [&'static str] {
        match self {
            Purpose::General => &["Jupiter", "Venus"],
            Purpose::Contract => &["Mercury", "Jupiter"],
            Purpose::Partnership => &["Venus", "Moon"],
            Purpose::Travel => &["Mercury", "Moon"],
            Purpose::Health => &["Sun", "Jupiter"],
            Purpose::Finance => &["Jupiter", "Venus"],
            Purpose::Launch => &["Jupiter", "Sun"],
            Purpose::Career => &["Sun", "Jupiter"],
        }
    }

    /// Whether Mercury retrograde spoils the matter.
    fn fears_mercury_retrograde(self) -> bool {
        matches!(self, Purpose::Contract | Purpose::Travel | Purpose::Launch)
    }

    /// Whether Venus retrograde spoils the matter.
    fn fears_venus_retrograde(self) -> bool {
        matches!(self, Purpose::Partnership)
    }
}

/// Local clock hours, `from` inclusive to `to` exclusive; `from > to` spans midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HourRange {
    /// First hour allowed (0-23).
    pub from: u8,
    /// Hour at which the range ends (1-24).
    pub to: u8,
}

impl HourRange {
    fn contains(&self, minute_of_day: i64) -> bool {
        let from = i64::from(self.from) * 60;
        let to = i64::from(self.to) * 60;
        if from <= to {
            (from..to).contains(&minute_of_day)
        } else {
            minute_of_day >= from || minute_of_day < to
        }
    }
}

/// The UTC offset of local time from an instant on (until the next one), so that
/// [`ElectionCriteria::local_hours`] follows daylight saving time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UtcOffset {
    /// ISO 8601 instant from which the offset applies.
    pub from: String,
    /// Minutes to add to UTC for local time (east positive, e.g. 120 for CEST).
    pub minutes: i32,
}

/// A natal point, as in a stored chart's `planets` list (other fields are ignored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NatalPoint {
    /// Body or angle name, e.g. "Sun", "Ascendant".
    pub name: String,
    /// Ecliptic longitude in degrees.
    pub ecliptic_longitude: f64,
}

/// What to look for. Every field has a default, so `{}` is a valid criteria object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ElectionCriteria {
    /// What the election is for.
    pub purpose: Purpose,
    /// Leave out moments when the Moon is void of course (default true).
    pub avoid_void_moon: bool,
    /// Leave out moments when Mercury is retrograde (default: when the purpose fears it).
    pub avoid_mercury_retrograde: Option<bool>,
    /// Leave out moments when Venus is retrograde (default: when the purpose fears it).
    pub avoid_venus_retrograde: Option<bool>,
    /// Leave out moments between sunset and sunrise.
    pub daytime_only: bool,
    /// Only moments within these local clock hours.
    pub local_hours: Option<HourRange>,
    /// UTC offsets of local time for `local_hours` (none: local time is UTC).
    pub utc_offsets: Vec<UtcOffset>,
    /// Minutes between the moments a search assesses (5-60, default 10).
    pub step_minutes: u32,
    /// Lowest score a search keeps (default 60).
    pub min_score: f64,
    /// Most windows a search returns (1-50, default 20).
    pub max_results: usize,
    /// The natal chart to elect for, if any.
    pub natal: Option<Vec<NatalPoint>>,
}

impl Default for ElectionCriteria {
    fn default() -> Self {
        ElectionCriteria {
            purpose: Purpose::General,
            avoid_void_moon: true,
            avoid_mercury_retrograde: None,
            avoid_venus_retrograde: None,
            daytime_only: false,
            local_hours: None,
            utc_offsets: Vec::new(),
            step_minutes: 10,
            min_score: 60.0,
            max_results: 20,
            natal: None,
        }
    }
}

impl ElectionCriteria {
    /// Criteria from a JSON object (`null` gives the defaults).
    pub fn from_value(value: &Value) -> Result<Self, EngineError> {
        if value.is_null() {
            return Ok(ElectionCriteria::default());
        }
        serde_json::from_value(value.clone())
            .map_err(|e| EngineError::InvalidInput(format!("election criteria: {e}")))
    }

    /// The criteria with every default resolved and out-of-range values clamped.
    fn resolve(&self) -> Result<Resolved, EngineError> {
        if let Some(range) = self.local_hours {
            if range.from > 23 || range.to == 0 || range.to > 24 || range.from == range.to {
                return Err(EngineError::InvalidInput(
                    "local_hours needs 0 <= from <= 23, 1 <= to <= 24 and from != to".into(),
                ));
            }
        }
        let mut offsets = self
            .utc_offsets
            .iter()
            .map(|o| {
                if o.minutes.abs() > 18 * 60 {
                    return Err(EngineError::InvalidInput(format!(
                        "UTC offset {} is out of range",
                        o.minutes
                    )));
                }
                UtcInstant::parse(&o.from)
                    .map(|at| (at, o.minutes))
                    .map_err(|e| EngineError::InvalidInput(format!("utc_offsets: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        offsets.sort_by_key(|(at, _)| *at);
        if !self.min_score.is_finite() {
            return Err(EngineError::InvalidInput(
                "min_score must be a number".into(),
            ));
        }
        Ok(Resolved {
            summary: CriteriaSummary {
                purpose: self.purpose,
                avoid_void_moon: self.avoid_void_moon,
                avoid_mercury_retrograde: self
                    .avoid_mercury_retrograde
                    .unwrap_or(self.purpose.fears_mercury_retrograde()),
                avoid_venus_retrograde: self
                    .avoid_venus_retrograde
                    .unwrap_or(self.purpose.fears_venus_retrograde()),
                daytime_only: self.daytime_only,
                local_hours: self.local_hours,
                step_minutes: self.step_minutes.clamp(5, 60),
                min_score: self.min_score.clamp(0.0, 100.0),
                max_results: self.max_results.clamp(1, 50),
                natal: self.natal.is_some(),
            },
            offsets,
            natal: self.natal.clone().unwrap_or_default(),
        })
    }
}

/// The criteria a search ran with, defaults resolved (the natal chart only as a flag).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CriteriaSummary {
    /// What the election is for.
    pub purpose: Purpose,
    /// Whether void-of-course Moon moments were left out.
    pub avoid_void_moon: bool,
    /// Whether moments with Mercury retrograde were left out.
    pub avoid_mercury_retrograde: bool,
    /// Whether moments with Venus retrograde were left out.
    pub avoid_venus_retrograde: bool,
    /// Whether night-time moments were left out.
    pub daytime_only: bool,
    /// Local clock hours searched, if restricted.
    pub local_hours: Option<HourRange>,
    /// Minutes between assessed moments.
    pub step_minutes: u32,
    /// Lowest score kept.
    pub min_score: f64,
    /// Most windows returned.
    pub max_results: usize,
    /// Whether a natal chart was given.
    pub natal: bool,
}

struct Resolved {
    summary: CriteriaSummary,
    offsets: Vec<(UtcInstant, i32)>,
    natal: Vec<NatalPoint>,
}

impl Resolved {
    /// Minutes since local midnight at `instant`.
    fn local_minute_of_day(&self, instant: UtcInstant) -> i64 {
        let offset = self
            .offsets
            .iter()
            .rev()
            .find(|(at, _)| *at <= instant)
            .or(self.offsets.first())
            .map_or(0, |(_, minutes)| *minutes);
        let (_, _, _, hh, mm, ..) = instant.add_micros(i64::from(offset) * 60_000_000).civil();
        i64::from(hh) * 60 + i64::from(mm)
    }
}

/// One electional rule that applies to a moment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElectionFactor {
    /// Stable code, e.g. "MOON_VOC", "BENEFIC_ANGULAR".
    pub code: &'static str,
    /// Points added to (or, when negative, taken from) the score.
    pub weight: f64,
    /// The election planet the rule is about, if any.
    pub planet: Option<&'static str>,
    /// The other party: the planet aspected, or the natal point contacted.
    pub target: Option<String>,
    /// Aspect name, e.g. "Trine", for the rules about aspects.
    pub aspect: Option<&'static str>,
    /// House (1-12) of `planet`, for the rules about houses.
    pub house: Option<u8>,
    /// Sign of `planet`, for the rules about dignity.
    pub sign: Option<&'static str>,
}

impl ElectionFactor {
    fn new(code: &'static str, weight: f64) -> Self {
        ElectionFactor {
            code,
            weight,
            planet: None,
            target: None,
            aspect: None,
            house: None,
            sign: None,
        }
    }
    fn planet(mut self, planet: &'static str) -> Self {
        self.planet = Some(planet);
        self
    }
    fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
    fn aspect(mut self, aspect: &'static str) -> Self {
        self.aspect = Some(aspect);
        self
    }
    fn house(mut self, house: u8) -> Self {
        self.house = Some(house);
        self
    }
    fn sign(mut self, sign: &'static str) -> Self {
        self.sign = Some(sign);
        self
    }
}

/// The electional assessment of a moment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElectionData {
    /// 0-100: 50 plus the factors' weights.
    pub score: f64,
    /// "favourable" (65 and up), "mixed" (45 and up) or "unfavourable".
    pub verdict: &'static str,
    /// What the election is for.
    pub purpose: Purpose,
    /// The rules that apply, Moon first, then the Ascendant, angles, retrogrades, the
    /// matter, the hour and the natal chart.
    pub factors: Vec<ElectionFactor>,
    /// The criteria filters this moment fails ("void_moon", "mercury_retrograde",
    /// "venus_retrograde", "night", "outside_hours"): a search would skip it.
    pub excluded_by: Vec<&'static str>,
    /// Planetary day and hour.
    pub planetary_hours: PlanetaryHours,
    /// The Moon's aspects and void-of-course status.
    pub moon_status: MoonStatus,
}

/// A chart for a candidate moment with its electional assessment (serialized flattened).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElectionChart {
    /// The chart, serialized inline.
    #[serde(flatten)]
    pub chart: Chart,
    /// The electional assessment.
    pub election_data: ElectionData,
}

/// A run of consecutive assessed moments scoring at least the minimum.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElectionWindow {
    /// First moment of the run, ISO 8601 UTC.
    pub start: String,
    /// Last moment of the run, ISO 8601 UTC.
    pub end: String,
    /// The best moment of the run (the earliest, on a tie), ISO 8601 UTC.
    pub best: String,
    /// Score of the best moment.
    pub score: f64,
    /// Verdict of the best moment.
    pub verdict: &'static str,
    /// The factors of the best moment.
    pub factors: Vec<ElectionFactor>,
}

/// How many moments each criteria filter left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Exclusions {
    /// Moon void of course.
    pub void_moon: u32,
    /// Mercury retrograde.
    pub mercury_retrograde: u32,
    /// Venus retrograde.
    pub venus_retrograde: u32,
    /// Between sunset and sunrise.
    pub night: u32,
    /// Outside the local hours.
    pub outside_hours: u32,
}

impl Exclusions {
    fn count(&mut self, reason: &str) {
        match reason {
            "void_moon" => self.void_moon += 1,
            "mercury_retrograde" => self.mercury_retrograde += 1,
            "venus_retrograde" => self.venus_retrograde += 1,
            "night" => self.night += 1,
            _ => self.outside_hours += 1,
        }
    }
}

/// The result of a search.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElectionSearch {
    /// Start of the span searched, ISO 8601 UTC.
    pub start: String,
    /// End of the span searched, ISO 8601 UTC.
    pub end: String,
    /// The best windows, best first.
    pub windows: Vec<ElectionWindow>,
    /// Moments assessed.
    pub evaluated: u32,
    /// Moments left out by each filter (a moment counts once, for the first it fails).
    pub excluded: Exclusions,
    /// The criteria, defaults resolved.
    pub criteria: CriteriaSummary,
}

// ---------------------------------------------------------------------------------------
// Assessment
// ---------------------------------------------------------------------------------------

/// The sign a longitude falls in (by its degree, not the rounded display).
fn sign_of(longitude: f64) -> &'static str {
    SIGNS[(pyfloat::floordiv(pyfloat::rem(longitude, 360.0), 30.0) as usize) % 12]
}

fn sign_index(sign: &str) -> usize {
    SIGNS.iter().position(|s| *s == sign).unwrap_or(0)
}

/// Angular distance between two longitudes, 0-180 degrees.
fn separation(a: f64, b: f64) -> f64 {
    let d = pyfloat::rem((a - b).abs(), 360.0);
    if d > 180.0 {
        360.0 - d
    } else {
        d
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dignity {
    Dignified,
    Debilitated,
    Peregrine,
}

/// Essential dignity by domicile and exaltation, debility by detriment and fall.
fn dignity(planet: &str, sign: &str) -> Dignity {
    let (dignified, debilitated): (&[&str], &[&str]) = match planet {
        "Sun" => (&["Leo", "Aries"], &["Aquarius", "Libra"]),
        "Moon" => (&["Cancer", "Taurus"], &["Capricorn", "Scorpio"]),
        "Mercury" => (&["Gemini", "Virgo"], &["Sagittarius", "Pisces"]),
        "Venus" => (
            &["Taurus", "Libra", "Pisces"],
            &["Scorpio", "Aries", "Virgo"],
        ),
        "Mars" => (
            &["Aries", "Scorpio", "Capricorn"],
            &["Libra", "Taurus", "Cancer"],
        ),
        "Jupiter" => (
            &["Sagittarius", "Pisces", "Cancer"],
            &["Gemini", "Virgo", "Capricorn"],
        ),
        "Saturn" => (
            &["Capricorn", "Aquarius", "Libra"],
            &["Cancer", "Leo", "Aries"],
        ),
        _ => (&[], &[]),
    };
    if dignified.contains(&sign) {
        Dignity::Dignified
    } else if debilitated.contains(&sign) {
        Dignity::Debilitated
    } else {
        Dignity::Peregrine
    }
}

fn is_angular(house: u8) -> bool {
    matches!(house, 1 | 4 | 7 | 10)
}

/// The 6th, 8th and 12th: houses of illness, death and undoing.
fn is_dark_house(house: u8) -> bool {
    matches!(house, 6 | 8 | 12)
}

fn is_soft(aspect: &str) -> bool {
    matches!(aspect, "Conjunction" | "Sextile" | "Trine")
}

/// How a contact with a benefic or a malefic counts: a benefic helps by conjunction,
/// sextile or trine; a malefic hurts by conjunction, square or opposition. `None` for the
/// contacts that count for nothing (a benefic's square, a malefic's trine).
fn contact(benefic: bool, aspect: &str) -> Option<bool> {
    match (benefic, aspect) {
        (true, "Conjunction" | "Sextile" | "Trine") => Some(true),
        (false, "Conjunction" | "Square" | "Opposition") => Some(false),
        _ => None,
    }
}

/// A Ptolemaic aspect between two longitudes within `orb`, if any.
fn ptolemaic_aspect(a: f64, b: f64, orb: f64) -> Option<&'static str> {
    let d = separation(a, b);
    [
        (0.0, "Conjunction"),
        (60.0, "Sextile"),
        (90.0, "Square"),
        (120.0, "Trine"),
        (180.0, "Opposition"),
    ]
    .into_iter()
    .find(|(angle, _)| (d - angle).abs() <= orb)
    .map(|(_, name)| name)
}

/// Whether `planet` is combust (within 8.5° of the Sun, but not cazimi).
fn is_combust(planet: &Placement, sun: Option<&Placement>) -> bool {
    planet.name != "Sun"
        && sun.is_some_and(|sun| {
            let d = separation(planet.ecliptic_longitude, sun.ecliptic_longitude);
            d > CAZIMI_ORB && d < COMBUST_ORB
        })
}

/// The condition of a significator (the Ascendant's ruler, the ruler of the house of the
/// matter, the natural significator), as factors named `{prefix}_{condition}`.
struct Condition {
    dignified: &'static str,
    debilitated: &'static str,
    angular: &'static str,
    dark_house: &'static str,
    retrograde: &'static str,
    combust: &'static str,
}

const ASC_RULER: Condition = Condition {
    dignified: "ASC_RULER_DIGNIFIED",
    debilitated: "ASC_RULER_DEBILITATED",
    angular: "ASC_RULER_ANGULAR",
    dark_house: "ASC_RULER_IN_DARK_HOUSE",
    retrograde: "ASC_RULER_RETROGRADE",
    combust: "ASC_RULER_COMBUST",
};
const HOUSE_RULER: Condition = Condition {
    dignified: "HOUSE_RULER_DIGNIFIED",
    debilitated: "HOUSE_RULER_DEBILITATED",
    angular: "HOUSE_RULER_ANGULAR",
    dark_house: "HOUSE_RULER_IN_DARK_HOUSE",
    retrograde: "HOUSE_RULER_RETROGRADE",
    combust: "HOUSE_RULER_COMBUST",
};
const SIGNIFICATOR: Condition = Condition {
    dignified: "SIGNIFICATOR_DIGNIFIED",
    debilitated: "SIGNIFICATOR_DEBILITATED",
    angular: "SIGNIFICATOR_ANGULAR",
    dark_house: "SIGNIFICATOR_IN_DARK_HOUSE",
    retrograde: "SIGNIFICATOR_RETROGRADE",
    combust: "SIGNIFICATOR_COMBUST",
};

/// Weights of a significator's conditions.
struct ConditionWeights {
    dignity: f64,
    angular: f64,
    dark_house: f64,
    affliction: f64,
}

fn condition(
    factors: &mut Vec<ElectionFactor>,
    names: &Condition,
    weights: &ConditionWeights,
    planet: &Placement,
    sun: Option<&Placement>,
    skip_retrograde: bool,
) {
    let sign = sign_of(planet.ecliptic_longitude);
    let tag = |code, weight| {
        ElectionFactor::new(code, weight)
            .planet(planet.name)
            .house(planet.house)
            .sign(sign)
    };
    match dignity(planet.name, sign) {
        Dignity::Dignified => factors.push(tag(names.dignified, weights.dignity)),
        Dignity::Debilitated => factors.push(tag(names.debilitated, -weights.dignity)),
        Dignity::Peregrine => {}
    }
    if is_angular(planet.house) {
        factors.push(tag(names.angular, weights.angular));
    } else if is_dark_house(planet.house) {
        factors.push(tag(names.dark_house, -weights.dark_house));
    }
    if planet.is_retrograde && !skip_retrograde {
        factors.push(tag(names.retrograde, -weights.affliction));
    }
    if is_combust(planet, sun) {
        factors.push(tag(names.combust, -weights.affliction));
    }
}

/// A moment's positions, ready to be assessed.
struct Moment<'a> {
    instant: UtcInstant,
    planets: &'a [Placement],
    cusps: &'a [f64],
    hours: &'a PlanetaryHours,
}

impl Moment<'_> {
    fn get(&self, name: &str) -> Option<&Placement> {
        self.planets.iter().find(|p| p.name == name)
    }
}

/// The filters a moment fails, in the order a search checks them.
fn exclusions(moment: &Moment, moon: &MoonStatus, criteria: &Resolved) -> Vec<&'static str> {
    let c = &criteria.summary;
    let mut out = Vec::new();
    if let Some(range) = c.local_hours {
        if !range.contains(criteria.local_minute_of_day(moment.instant)) {
            out.push("outside_hours");
        }
    }
    if c.daytime_only && !moment.hours.is_day {
        out.push("night");
    }
    if c.avoid_void_moon && moon.void_of_course {
        out.push("void_moon");
    }
    let retrograde = |name| moment.get(name).is_some_and(|p| p.is_retrograde);
    if c.avoid_mercury_retrograde && retrograde("Mercury") {
        out.push("mercury_retrograde");
    }
    if c.avoid_venus_retrograde && retrograde("Venus") {
        out.push("venus_retrograde");
    }
    out
}

/// Score, factors and failed filters of a moment.
fn assess(moment: &Moment, criteria: &Resolved) -> Result<ElectionData, EngineError> {
    let purpose = criteria.summary.purpose;
    let moon = moment
        .get("Moon")
        .ok_or_else(|| EngineError::InvalidInput("chart has no Moon".into()))?;
    let sun = moment.get("Sun");
    let status = moon_status(moon, moment.planets);
    let mut factors = Vec::new();

    // The Moon: her condition, and the next aspect she perfects.
    let moon_sign = sign_of(moon.ecliptic_longitude);
    if status.void_of_course {
        factors.push(ElectionFactor::new("MOON_VOC", -20.0).planet("Moon"));
    }
    if is_combust(moon, sun) {
        factors.push(ElectionFactor::new("MOON_COMBUST", -12.0).planet("Moon"));
    } else if let Some(sun) = sun {
        if pyfloat::rem(moon.ecliptic_longitude - sun.ecliptic_longitude, 360.0) < 180.0 {
            factors.push(ElectionFactor::new("MOON_WAXING", 5.0).planet("Moon"));
        }
    }
    let lon = pyfloat::rem(moon.ecliptic_longitude, 360.0);
    if (195.0..225.0).contains(&lon) {
        factors.push(ElectionFactor::new("MOON_VIA_COMBUSTA", -8.0).planet("Moon"));
    }
    degree_factors(&mut factors, moon.ecliptic_longitude, true);
    match dignity("Moon", moon_sign) {
        Dignity::Dignified => factors.push(
            ElectionFactor::new("MOON_DIGNIFIED", 6.0)
                .planet("Moon")
                .sign(moon_sign),
        ),
        Dignity::Debilitated => factors.push(
            ElectionFactor::new("MOON_DEBILITATED", -6.0)
                .planet("Moon")
                .sign(moon_sign),
        ),
        Dignity::Peregrine => {}
    }
    if moon.speed > 13.5 {
        factors.push(ElectionFactor::new("MOON_SWIFT", 2.0).planet("Moon"));
    } else if moon.speed < 12.5 {
        factors.push(ElectionFactor::new("MOON_SLOW", -2.0).planet("Moon"));
    }
    if is_angular(moon.house) {
        factors.push(
            ElectionFactor::new("MOON_ANGULAR", 3.0)
                .planet("Moon")
                .house(moon.house),
        );
    } else if is_dark_house(moon.house) {
        factors.push(
            ElectionFactor::new("MOON_IN_DARK_HOUSE", -5.0)
                .planet("Moon")
                .house(moon.house),
        );
    }
    if let Some(next) = &status.next_applying_aspect {
        let soft = is_soft(next.aspect);
        let applying = |code, weight| {
            ElectionFactor::new(code, weight)
                .planet("Moon")
                .target(next.planet)
                .aspect(next.aspect)
        };
        if BENEFICS.contains(&next.planet) {
            factors.push(applying(
                "MOON_APPLYING_BENEFIC",
                if soft { 10.0 } else { 4.0 },
            ));
        } else if MALEFICS.contains(&next.planet) {
            // A conjunction with a malefic is no help.
            let hard = !soft || next.aspect == "Conjunction";
            factors.push(applying(
                "MOON_APPLYING_MALEFIC",
                if hard { -10.0 } else { -3.0 },
            ));
        }
        if purpose.significator() == Some(next.planet) {
            factors.push(applying(
                "MOON_APPLYING_SIGNIFICATOR",
                if soft { 5.0 } else { -3.0 },
            ));
        }
    }

    // The Ascendant and its ruler.
    let asc_lon = moment
        .get("Ascendant")
        .map_or(moment.cusps[0], |a| a.ecliptic_longitude);
    let asc_sign = sign_of(asc_lon);
    let asc_degree = pyfloat::rem(asc_lon, 30.0);
    if asc_degree < 3.0 {
        factors.push(ElectionFactor::new("ASC_EARLY", -5.0).sign(asc_sign));
    } else if asc_degree >= 27.0 {
        factors.push(ElectionFactor::new("ASC_LATE", -5.0).sign(asc_sign));
    }
    degree_factors(&mut factors, asc_lon, false);
    let asc_ruler_name = traditional_ruler(asc_sign);
    if let Some(ruler) = asc_ruler_name.and_then(|r| moment.get(r)) {
        condition(
            &mut factors,
            &ASC_RULER,
            &ConditionWeights {
                dignity: 6.0,
                angular: 4.0,
                dark_house: 5.0,
                affliction: 6.0,
            },
            ruler,
            sun,
            false,
        );
    }

    // Benefics and malefics on the angles.
    for p in moment.planets {
        let benefic = BENEFICS.contains(&p.name);
        let malefic = MALEFICS.contains(&p.name);
        if !(benefic || malefic) {
            continue;
        }
        let (code, weight) = match (benefic, p.house) {
            (true, 1) => ("BENEFIC_IN_1ST", 8.0),
            (true, h) if is_angular(h) => ("BENEFIC_ANGULAR", 4.0),
            (false, 1) => ("MALEFIC_IN_1ST", -10.0),
            (false, h) if is_angular(h) => ("MALEFIC_ANGULAR", -5.0),
            _ => continue,
        };
        factors.push(
            ElectionFactor::new(code, weight)
                .planet(p.name)
                .house(p.house),
        );
    }

    // Retrograde Mercury and Venus: heavy when the matter needs them.
    let feared = [
        (
            "Mercury",
            "MERCURY_RETROGRADE",
            criteria.summary.avoid_mercury_retrograde,
        ),
        (
            "Venus",
            "VENUS_RETROGRADE",
            criteria.summary.avoid_venus_retrograde,
        ),
    ];
    for (name, code, avoided) in feared {
        if moment.get(name).is_some_and(|p| p.is_retrograde) {
            let weight = if avoided || purpose.significator() == Some(name) {
                -10.0
            } else {
                -2.0
            };
            factors.push(ElectionFactor::new(code, weight).planet(name));
        }
    }

    // The matter: the ruler of its house, and its natural significator.
    let house_ruler_name = purpose
        .house()
        .filter(|h| *h != 1)
        .and_then(|h| traditional_ruler(sign_of(moment.cusps[h - 1])));
    if let Some(ruler) = house_ruler_name.and_then(|r| moment.get(r)) {
        if Some(ruler.name) != asc_ruler_name {
            condition(
                &mut factors,
                &HOUSE_RULER,
                &ConditionWeights {
                    dignity: 5.0,
                    angular: 3.0,
                    dark_house: 4.0,
                    affliction: 5.0,
                },
                ruler,
                sun,
                false,
            );
        }
    }
    if let Some(sig) = purpose.significator().and_then(|s| moment.get(s)) {
        if Some(sig.name) != asc_ruler_name && Some(sig.name) != house_ruler_name {
            condition(
                &mut factors,
                &SIGNIFICATOR,
                &ConditionWeights {
                    dignity: 4.0,
                    angular: 2.0,
                    dark_house: 3.0,
                    affliction: 4.0,
                },
                sig,
                sun,
                // Mercury's and Venus's retrogradation is weighed above.
                matches!(sig.name, "Mercury" | "Venus"),
            );
        }
    }

    // The planetary hour.
    let hour_ruler = moment.hours.hour_ruler;
    if purpose.hour_rulers().contains(&hour_ruler) {
        factors.push(ElectionFactor::new("HOUR_RULER_FAVOURS_PURPOSE", 5.0).planet(hour_ruler));
    } else if MALEFICS.contains(&hour_ruler) {
        factors.push(ElectionFactor::new("HOUR_RULER_MALEFIC", -3.0).planet(hour_ruler));
    }

    // The natal chart.
    let natal = &criteria.natal;
    if !natal.is_empty() {
        natal_factors(&mut factors, moment, asc_sign, natal);
    }

    let score = pyfloat::round(
        (50.0 + factors.iter().map(|f| f.weight).sum::<f64>()).clamp(0.0, 100.0),
        1,
    );
    Ok(ElectionData {
        score,
        verdict: verdict(score),
        purpose,
        factors,
        excluded_by: exclusions(moment, &status, criteria),
        planetary_hours: moment.hours.clone(),
        moon_status: status,
    })
}

/// Lilly's qualities of the Moon's or the Ascendant's degree: pitted and lame degrees
/// hinder, degrees increasing fortune help and, for the Ascendant, light degrees help while
/// dark and void ones hinder (smoky degrees are between the two).
fn degree_factors(factors: &mut Vec<ElectionFactor>, longitude: f64, moon: bool) {
    let q = degree_qualities(longitude);
    let tag = |code: &'static str, weight: f64| {
        let factor = ElectionFactor::new(code, weight).sign(q.sign);
        if moon {
            factor.planet("Moon")
        } else {
            factor
        }
    };
    let pick = |moon_code, asc_code| if moon { moon_code } else { asc_code };
    if q.pitted {
        factors.push(tag(pick("MOON_PITTED_DEGREE", "ASC_PITTED_DEGREE"), -4.0));
    }
    if q.azimene {
        factors.push(tag(pick("MOON_AZIMENE_DEGREE", "ASC_AZIMENE_DEGREE"), -3.0));
    }
    if q.fortune {
        let weight = if moon { 3.0 } else { 4.0 };
        factors.push(tag(
            pick("MOON_FORTUNE_DEGREE", "ASC_FORTUNE_DEGREE"),
            weight,
        ));
    }
    if !moon {
        match q.light {
            "light" => factors.push(tag("ASC_LIGHT_DEGREE", 2.0)),
            "dark" | "void" => factors.push(tag("ASC_DARK_DEGREE", -2.0)),
            _ => {}
        }
    }
}

fn verdict(score: f64) -> &'static str {
    if score >= FAVOURABLE {
        "favourable"
    } else if score >= MIXED {
        "mixed"
    } else {
        "unfavourable"
    }
}

/// The election's contacts with a natal chart: the election's Ascendant counted from the
/// natal one, benefics and malefics on the natal lights, angles and Ascendant ruler, and
/// the election Moon's aspects to natal benefics and malefics.
fn natal_factors(
    factors: &mut Vec<ElectionFactor>,
    moment: &Moment,
    asc_sign: &'static str,
    natal: &[NatalPoint],
) {
    let natal_point = |name: &str| natal.iter().find(|p| p.name == name);

    if let Some(natal_asc) = natal_point("Ascendant") {
        let natal_asc_sign = sign_of(natal_asc.ecliptic_longitude);
        let place = (sign_index(asc_sign) + 12 - sign_index(natal_asc_sign)) % 12 + 1;
        let place = place as u8;
        if matches!(place, 1 | 5 | 9 | 10 | 11) {
            factors.push(ElectionFactor::new("NATAL_ASC_WELL_PLACED", 5.0).house(place));
        } else if is_dark_house(place) {
            factors.push(ElectionFactor::new("NATAL_ASC_BADLY_PLACED", -6.0).house(place));
        }
    }

    // Sensitive natal points: the lights, the angles and the Ascendant's ruler.
    let mut sensitive: Vec<&NatalPoint> = ["Sun", "Moon", "Ascendant", "Midheaven"]
        .iter()
        .filter_map(|n| natal_point(n))
        .collect();
    if let Some(ruler) = natal_point("Ascendant")
        .and_then(|a| traditional_ruler(sign_of(a.ecliptic_longitude)))
        .and_then(natal_point)
    {
        if !sensitive.iter().any(|p| p.name == ruler.name) {
            sensitive.push(ruler);
        }
    }
    for p in moment.planets {
        let benefic = BENEFICS.contains(&p.name);
        if !(benefic || MALEFICS.contains(&p.name)) {
            continue;
        }
        for point in &sensitive {
            let Some(aspect) =
                ptolemaic_aspect(p.ecliptic_longitude, point.ecliptic_longitude, NATAL_ORB)
            else {
                continue;
            };
            let (code, weight) = match contact(benefic, aspect) {
                Some(true) => ("NATAL_BENEFIC_CONTACT", 4.0),
                Some(false) => ("NATAL_MALEFIC_CONTACT", -6.0),
                None => continue,
            };
            factors.push(
                ElectionFactor::new(code, weight)
                    .planet(p.name)
                    .target(point.name.clone())
                    .aspect(aspect),
            );
        }
    }

    if let Some(moon) = moment.get("Moon") {
        for point in natal {
            let benefic = BENEFICS.contains(&point.name.as_str());
            if !(benefic || MALEFICS.contains(&point.name.as_str())) {
                continue;
            }
            let Some(aspect) =
                ptolemaic_aspect(moon.ecliptic_longitude, point.ecliptic_longitude, NATAL_ORB)
            else {
                continue;
            };
            let (code, weight) = match contact(benefic, aspect) {
                Some(true) => ("NATAL_MOON_TO_BENEFIC", 3.0),
                Some(false) => ("NATAL_MOON_TO_MALEFIC", -4.0),
                None => continue,
            };
            factors.push(
                ElectionFactor::new(code, weight)
                    .planet("Moon")
                    .target(point.name.clone())
                    .aspect(aspect),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------------------

/// A chart for a candidate moment with its electional assessment.
pub fn calculate_election_chart(
    kernels: &KernelSet,
    req: &ChartRequest,
    criteria: &ElectionCriteria,
) -> Result<ElectionChart, EngineError> {
    let resolved = criteria.resolve()?;
    let chart = calculate_chart(kernels, req)?;
    let hours = planetary_hours(kernels, req.instant, req.latitude, req.longitude);
    let cusps: Vec<f64> = chart.houses.iter().map(|h| h.ecliptic_longitude).collect();
    let election_data = assess(
        &Moment {
            instant: req.instant,
            planets: &chart.planets,
            cusps: &cusps,
            hours: &hours,
        },
        &resolved,
    )?;
    Ok(ElectionChart {
        chart,
        election_data,
    })
}

/// A search's source of positions: exact on an hourly grid, linearly interpolated in
/// between. Each planets computation gives the positions at its instant and an hour later
/// (from which the speeds come), so one computation serves two hours of moments. Between
/// grid points the Moon's longitude is off by well under an arc second.
struct Sampler<'a> {
    kernels: &'a KernelSet,
    req: &'a ChartRequest<'a>,
    system: HouseSystem,
    sidereal: bool,
    ayanamsa: &'static str,
    /// Anchors are every two hours from here.
    origin: UtcInstant,
    anchors: Vec<(i64, Anchor)>,
}

/// Exact positions (and the Greenwich sidereal time) at a grid instant.
struct Anchor {
    bodies: Vec<BodyPosition>,
    gast_hours: f64,
}

/// Longitude difference `b - a`, within ±180°.
fn wrapped(a: f64, b: f64) -> f64 {
    pyfloat::rem(b - a + 180.0, 360.0) - 180.0
}

/// Bodies whose apparent motion can turn retrograde, as the planets computation flags them.
fn can_retrograde(name: &str) -> bool {
    !matches!(name, "Sun" | "Moon" | "Lilith")
}

impl<'a> Sampler<'a> {
    fn new(kernels: &'a KernelSet, req: &'a ChartRequest<'a>) -> Self {
        Sampler {
            kernels,
            req,
            system: HouseSystem::from_code(req.house_system),
            sidereal: req.zodiac_type.trim().eq_ignore_ascii_case("sidereal"),
            ayanamsa: ayanamsa(req.ayanamsa).code,
            origin: req.instant,
            anchors: Vec::new(),
        }
    }

    /// The anchor `index` × 2 hours after the origin (the last three are kept).
    fn anchor(&mut self, index: i64) -> Result<&Anchor, EngineError> {
        if let Some(pos) = self.anchors.iter().position(|(i, _)| *i == index) {
            return Ok(&self.anchors[pos].1);
        }
        let jd = self
            .origin
            .add_micros(index * 2 * 3_600_000_000)
            .julian_day();
        let shift = if self.sidereal {
            ayanamsa_info(jd, self.ayanamsa).value
        } else {
            0.0
        };
        let (planets, gast_hours) = planets_and_sidereal_time(self.kernels, jd, shift)?;
        if self.anchors.len() == 3 {
            self.anchors.remove(0);
        }
        self.anchors.push((
            index,
            Anchor {
                bodies: planets.bodies,
                gast_hours,
            },
        ));
        Ok(&self.anchors.last().unwrap().1)
    }

    /// Placements and house cusps at `instant` (not before the origin).
    fn at(&mut self, instant: UtcInstant) -> Result<(Vec<Placement>, Vec<f64>), EngineError> {
        let hours = instant.seconds_since(&self.origin) / 3600.0;
        let index = (hours / 2.0).floor() as i64;
        let into = hours - index as f64 * 2.0; // 0 ≤ into < 2
        let (first, gast0) = {
            let anchor = self.anchor(index)?;
            let bodies: Vec<(&'static str, f64, f64, bool)> = anchor
                .bodies
                .iter()
                .map(|b| (b.name, b.longitude, b.speed, b.is_retrograde))
                .collect();
            (bodies, anchor.gast_hours)
        };
        let (next_bodies, gast1) = {
            let next = self.anchor(index + 1)?;
            (
                next.bodies
                    .iter()
                    .map(|b| (b.name, b.longitude, b.speed))
                    .collect::<Vec<_>>(),
                next.gast_hours,
            )
        };

        // Each body's track over the three hours from the anchor: exact every hour (an
        // anchor's position an hour later comes from its speed, degrees per day over that
        // hour), linear in between.
        let hour_later = |lon: f64, speed: f64| pyfloat::rem(lon + speed / 24.0, 360.0);
        let mut raw = Vec::with_capacity(first.len());
        for &(name, lon, speed, is_retrograde) in &first {
            if into == 0.0 {
                raw.push((name, lon, speed, is_retrograde));
                continue;
            }
            let Some(&(_, next, next_speed)) = next_bodies.iter().find(|(n, ..)| *n == name) else {
                continue;
            };
            let points = [
                lon,
                hour_later(lon, speed),
                next,
                hour_later(next, next_speed),
            ];
            let track = |hours: f64| {
                let i = (hours.floor() as usize).min(2);
                let (a, b) = (points[i], points[i + 1]);
                pyfloat::rem(a + wrapped(a, b) * (hours - i as f64), 360.0)
            };
            // The speed as the planets computation measures it: over the next hour.
            let (longitude, later) = (track(into), track(into + 1.0));
            let speed = wrapped(longitude, later) * 24.0;
            raw.push((name, longitude, speed, can_retrograde(name) && speed < 0.0));
        }

        let gast1 = if gast1 < gast0 { gast1 + 24.0 } else { gast1 };
        let gast = if into == 0.0 {
            gast0
        } else {
            pyfloat::rem(gast0 + (gast1 - gast0) * into / 2.0, 24.0)
        };
        let jd = instant.julian_day();
        let shift = if self.sidereal {
            ayanamsa_info(jd, self.ayanamsa).value
        } else {
            0.0
        };
        let houses = houses_at_sidereal_time(
            jd,
            gast,
            self.req.latitude,
            self.req.longitude,
            self.system,
            shift,
        );
        let cusps = houses.cusps.to_vec();
        Ok((placements(raw, &houses), cusps))
    }
}

/// The solar days of a search's moments: from the sunrises and sunsets of the whole span
/// when a single kernel covers it, otherwise found day by day.
struct SolarDays<'a> {
    kernels: &'a KernelSet,
    latitude: f64,
    longitude: f64,
    events: Option<SunEvents>,
    last: Option<SolarDay>,
}

impl<'a> SolarDays<'a> {
    fn new(kernels: &'a KernelSet, req: &ChartRequest, end: UtcInstant) -> Self {
        let events = SunEvents::find(
            kernels,
            jd_utc(req.instant) - 1.6,
            jd_utc(end) + 1.6,
            req.latitude,
            req.longitude,
        )
        .ok()
        .flatten();
        SolarDays {
            kernels,
            latitude: req.latitude,
            longitude: req.longitude,
            events,
            last: None,
        }
    }

    fn hours(&mut self, instant: UtcInstant) -> PlanetaryHours {
        if let Some(times) = self
            .events
            .as_ref()
            .and_then(|e| e.rise_set(jd_utc(instant)))
        {
            return hours_in(&SolarDay::from_julian_days(times), instant);
        }
        let reuse = self.latitude.abs() < SOLAR_DAY_CACHE_MAX_LAT;
        let day = match self.last {
            Some(d) if reuse && d.covers(instant) => d,
            _ => solar_day(self.kernels, instant, self.latitude, self.longitude),
        };
        self.last = Some(day);
        hours_in(&day, instant)
    }
}

/// The best windows from `req.instant` to `end` (at most [`MAX_SEARCH_DAYS`]) at the
/// request's place, with its house system and zodiac.
///
/// Moments are assessed every `step_minutes`; those failing a criteria filter are left
/// out, and runs of consecutive moments scoring at least `min_score` form the windows,
/// ranked by their best score. Positions between hourly grid points are interpolated
/// for speed, then each window's best moment is assessed again on exact positions, so
/// its score and factors are what [`calculate_election_chart`] gives for it.
pub fn search_elections(
    kernels: &KernelSet,
    req: &ChartRequest,
    end: UtcInstant,
    criteria: &ElectionCriteria,
) -> Result<ElectionSearch, EngineError> {
    let resolved = criteria.resolve()?;
    let start = req.instant;
    let span_days = end.seconds_since(&start) / 86_400.0;
    if span_days <= 0.0 {
        return Err(EngineError::InvalidInput(
            "the search must end after it starts".into(),
        ));
    }
    if span_days > MAX_SEARCH_DAYS {
        return Err(EngineError::InvalidInput(format!(
            "a search spans at most {MAX_SEARCH_DAYS} days"
        )));
    }
    // Fail fast rather than after assessing most of the span. The grid runs up to two
    // hours past the end, and planets are computed an hour after each grid point.
    require_kernel(kernels, start.julian_day())?;
    require_kernel(kernels, end.julian_day() + 3.0 / 24.0)?;

    let step = i64::from(resolved.summary.step_minutes) * 60_000_000;
    let mut sampler = Sampler::new(kernels, req);
    let mut days = SolarDays::new(kernels, req, end);
    let mut evaluated = 0;
    let mut excluded = Exclusions::default();
    let mut windows: Vec<(UtcInstant, ElectionWindow)> = Vec::new();
    let mut open: Option<(UtcInstant, ElectionWindow)> = None;

    let mut instant = start;
    while instant <= end {
        let kept = 'moment: {
            // Clock hours first: they need no ephemeris.
            if let Some(range) = resolved.summary.local_hours {
                if !range.contains(resolved.local_minute_of_day(instant)) {
                    excluded.count("outside_hours");
                    break 'moment None;
                }
            }
            let hours = days.hours(instant);
            if resolved.summary.daytime_only && !hours.is_day {
                excluded.count("night");
                break 'moment None;
            }
            let (planets, cusps) = sampler.at(instant)?;
            let data = assess(
                &Moment {
                    instant,
                    planets: &planets,
                    cusps: &cusps,
                    hours: &hours,
                },
                &resolved,
            )?;
            evaluated += 1;
            if let Some(reason) = data.excluded_by.first() {
                excluded.count(reason);
                break 'moment None;
            }
            (data.score >= resolved.summary.min_score).then_some(data)
        };

        match kept {
            Some(data) => {
                let at = instant.isoformat();
                match open.as_mut() {
                    Some((best, w)) => {
                        w.end = at.clone();
                        if data.score > w.score {
                            *best = instant;
                            w.best = at;
                            w.score = data.score;
                        }
                    }
                    None => {
                        open = Some((
                            instant,
                            ElectionWindow {
                                start: at.clone(),
                                end: at.clone(),
                                best: at,
                                score: data.score,
                                verdict: data.verdict,
                                factors: Vec::new(),
                            },
                        ))
                    }
                }
            }
            None => windows.extend(open.take()),
        }
        instant = instant.add_micros(step);
    }
    windows.extend(open.take());

    // Best first; earlier first on a tie (windows are already in time order).
    windows.sort_by(|a, b| b.1.score.total_cmp(&a.1.score));
    windows.truncate(resolved.summary.max_results);

    // The best moments again, on exact positions.
    for (best, window) in &mut windows {
        let exact = sky(
            kernels,
            &ChartRequest {
                instant: *best,
                ..req.clone()
            },
        )?;
        let data = assess(
            &Moment {
                instant: *best,
                planets: &exact.planets,
                cusps: &exact.cusps,
                hours: &days.hours(*best),
            },
            &resolved,
        )?;
        window.score = data.score;
        window.verdict = data.verdict;
        window.factors = data.factors;
    }
    windows.sort_by(|a, b| b.1.score.total_cmp(&a.1.score));

    Ok(ElectionSearch {
        start: start.isoformat(),
        end: end.isoformat(),
        windows: windows.into_iter().map(|(_, w)| w).collect(),
        evaluated,
        excluded,
        criteria: resolved.summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(name: &'static str, longitude: f64, house: u8, speed: f64) -> Placement {
        Placement {
            name,
            symbol: "",
            sign: sign_of(longitude),
            sign_symbol: "",
            degree: pyfloat::rem(longitude, 30.0) as i64,
            minute: 0,
            ecliptic_longitude: longitude,
            house,
            speed,
            is_retrograde: speed < 0.0 && name != "Moon" && name != "Sun",
            symbolic_degree: 1,
        }
    }

    fn hours(ruler: &'static str, is_day: bool) -> PlanetaryHours {
        PlanetaryHours {
            is_day,
            day_ruler: "Sun",
            hour_ruler: ruler,
            hour_number: 1,
            hour_type: if is_day { "Day" } else { "Night" },
            sunrise: String::new(),
            sunset: String::new(),
        }
    }

    /// Cusps every 30° from 0° Aries: house n starts at (n-1)·30°.
    fn cusps() -> Vec<f64> {
        (0..12).map(|i| f64::from(i) * 30.0).collect()
    }

    fn assess_with(
        planets: &[Placement],
        hour: &PlanetaryHours,
        c: &ElectionCriteria,
    ) -> ElectionData {
        let cusps = cusps();
        assess(
            &Moment {
                instant: UtcInstant::parse("2026-10-01T12:00:00Z").unwrap(),
                planets,
                cusps: &cusps,
                hours: hour,
            },
            &c.resolve().unwrap(),
        )
        .unwrap()
    }

    fn codes(data: &ElectionData) -> Vec<&'static str> {
        data.factors.iter().map(|f| f.code).collect()
    }

    /// A pleasant sky: Moon in Taurus applying by trine to Jupiter, Venus in the 1st.
    fn good_sky() -> Vec<Placement> {
        vec![
            placement("Sun", 100.0, 4, 1.0),
            placement("Moon", 40.0, 2, 14.0),
            placement("Mercury", 110.0, 4, 1.2),
            placement("Venus", 15.0, 1, 1.1),
            placement("Mars", 160.0, 6, 0.6),
            placement("Jupiter", 165.0, 6, 0.1),
            placement("Saturn", 320.0, 11, 0.05),
            placement("Ascendant", 10.0, 1, 0.0),
            placement("Midheaven", 280.0, 10, 0.0),
        ]
    }

    #[test]
    fn a_good_moment_scores_favourable() {
        let data = assess_with(
            &good_sky(),
            &hours("Jupiter", true),
            &ElectionCriteria::default(),
        );
        let codes = codes(&data);
        assert!(codes.contains(&"MOON_APPLYING_BENEFIC"), "{codes:?}");
        assert!(codes.contains(&"MOON_DIGNIFIED"));
        assert!(codes.contains(&"BENEFIC_IN_1ST"));
        assert!(codes.contains(&"HOUR_RULER_FAVOURS_PURPOSE"));
        assert!(!codes.contains(&"MOON_VOC"));
        assert_eq!(data.verdict, "favourable");
        assert!(data.excluded_by.is_empty());
    }

    #[test]
    fn a_void_moon_is_penalized_and_excluded() {
        // Moon at 29° Taurus: no aspect left before Gemini.
        let mut sky = good_sky();
        sky[1] = placement("Moon", 59.5, 2, 14.0);
        let data = assess_with(&sky, &hours("Jupiter", true), &ElectionCriteria::default());
        assert!(codes(&data).contains(&"MOON_VOC"));
        assert_eq!(data.excluded_by, vec!["void_moon"]);
        let keep = ElectionCriteria {
            avoid_void_moon: false,
            ..Default::default()
        };
        assert!(assess_with(&sky, &hours("Jupiter", true), &keep)
            .excluded_by
            .is_empty());
    }

    #[test]
    fn mercury_retrograde_weighs_on_contracts() {
        let mut sky = good_sky();
        sky[2] = placement("Mercury", 110.0, 4, -0.5);
        let general = assess_with(&sky, &hours("Sun", true), &ElectionCriteria::default());
        let contract = assess_with(
            &sky,
            &hours("Sun", true),
            &ElectionCriteria {
                purpose: Purpose::Contract,
                ..Default::default()
            },
        );
        let weight = |d: &ElectionData| {
            d.factors
                .iter()
                .find(|f| f.code == "MERCURY_RETROGRADE")
                .unwrap()
                .weight
        };
        assert_eq!(weight(&general), -2.0);
        assert_eq!(weight(&contract), -10.0);
        assert!(general.excluded_by.is_empty());
        assert_eq!(contract.excluded_by, vec!["mercury_retrograde"]);
    }

    #[test]
    fn degree_qualities_of_the_moon_and_the_ascendant() {
        // Ascendant in the 11th degree of Aries (pitted, dark); Moon in the 8th of Taurus
        // (lame).
        let mut sky = good_sky();
        sky[7] = placement("Ascendant", 10.5, 1, 0.0);
        sky[1] = placement("Moon", 37.5, 2, 14.0);
        let data = assess_with(&sky, &hours("Jupiter", true), &ElectionCriteria::default());
        let found = codes(&data);
        for code in [
            "ASC_PITTED_DEGREE",
            "ASC_DARK_DEGREE",
            "MOON_AZIMENE_DEGREE",
        ] {
            assert!(found.contains(&code), "{code} missing from {found:?}");
        }
        let lame = data
            .factors
            .iter()
            .find(|f| f.code == "MOON_AZIMENE_DEGREE")
            .unwrap();
        assert_eq!(
            (lame.planet, lame.sign, lame.weight),
            (Some("Moon"), Some("Taurus"), -3.0)
        );

        // Ascendant in the 19th of Aries (light, increasing fortune); Moon in the 3rd of
        // Taurus (increasing fortune).
        sky[7] = placement("Ascendant", 18.5, 1, 0.0);
        sky[1] = placement("Moon", 32.5, 2, 14.0);
        let data = assess_with(&sky, &hours("Jupiter", true), &ElectionCriteria::default());
        let found = codes(&data);
        for code in [
            "ASC_FORTUNE_DEGREE",
            "ASC_LIGHT_DEGREE",
            "MOON_FORTUNE_DEGREE",
        ] {
            assert!(found.contains(&code), "{code} missing from {found:?}");
        }
        assert!(!found.iter().any(|c| c.ends_with("PITTED_DEGREE")));
    }

    #[test]
    fn malefics_on_the_ascendant_and_bad_hours_hurt() {
        let mut sky = good_sky();
        sky[6] = placement("Saturn", 20.0, 1, 0.05);
        let data = assess_with(&sky, &hours("Saturn", false), &ElectionCriteria::default());
        let codes = codes(&data);
        assert!(codes.contains(&"MALEFIC_IN_1ST"));
        assert!(codes.contains(&"HOUR_RULER_MALEFIC"));
    }

    #[test]
    fn natal_factors_only_with_a_natal_chart() {
        let natal = vec![
            NatalPoint {
                name: "Ascendant".into(),
                ecliptic_longitude: 250.0, // Sagittarius: the election's Aries is its 5th
            },
            NatalPoint {
                name: "Sun".into(),
                ecliptic_longitude: 135.5, // trine the election's Venus at 15°
            },
            NatalPoint {
                name: "Moon".into(),
                ecliptic_longitude: 250.0,
            },
            NatalPoint {
                name: "Jupiter".into(),
                ecliptic_longitude: 280.0, // trine the election's Moon at 40°
            },
        ];
        let sky = good_sky();
        let without = assess_with(&sky, &hours("Sun", true), &ElectionCriteria::default());
        assert!(!codes(&without).iter().any(|c| c.starts_with("NATAL_")));
        let with = assess_with(
            &sky,
            &hours("Sun", true),
            &ElectionCriteria {
                natal: Some(natal),
                ..Default::default()
            },
        );
        let codes = codes(&with);
        assert!(codes.contains(&"NATAL_ASC_WELL_PLACED"), "{codes:?}");
        assert!(codes.contains(&"NATAL_BENEFIC_CONTACT"));
        assert!(codes.contains(&"NATAL_MOON_TO_BENEFIC"));
    }

    #[test]
    fn local_hours_follow_the_utc_offset_and_wrap_midnight() {
        let criteria = ElectionCriteria {
            local_hours: Some(HourRange { from: 22, to: 2 }),
            utc_offsets: vec![UtcOffset {
                from: "2026-01-01T00:00:00Z".into(),
                minutes: 120,
            }],
            ..Default::default()
        }
        .resolve()
        .unwrap();
        let at = |s| criteria.local_minute_of_day(UtcInstant::parse(s).unwrap());
        assert_eq!(at("2026-10-01T21:30:00Z"), 23 * 60 + 30);
        let range = criteria.summary.local_hours.unwrap();
        assert!(range.contains(at("2026-10-01T21:30:00Z")));
        assert!(range.contains(at("2026-10-01T23:59:00Z")));
        assert!(!range.contains(at("2026-10-02T00:00:00Z")));
    }

    #[test]
    fn criteria_parse_with_defaults_and_reject_typos() {
        let c = ElectionCriteria::from_value(&serde_json::json!({"purpose": "travel"})).unwrap();
        assert_eq!(c.purpose, Purpose::Travel);
        assert!(c.avoid_void_moon);
        let r = c.resolve().unwrap();
        assert!(r.summary.avoid_mercury_retrograde);
        assert!(!r.summary.avoid_venus_retrograde);
        assert!(ElectionCriteria::from_value(&serde_json::json!({"purpose": "war"})).is_err());
        assert!(ElectionCriteria::from_value(&serde_json::json!({"avoid_voc": true})).is_err());
        let hours = serde_json::json!({"local_hours": {"from": 9, "to": 9}});
        assert!(ElectionCriteria::from_value(&hours)
            .unwrap()
            .resolve()
            .is_err());
    }

    #[test]
    fn dignities() {
        assert_eq!(dignity("Mercury", "Virgo"), Dignity::Dignified);
        assert_eq!(dignity("Mercury", "Pisces"), Dignity::Debilitated);
        assert_eq!(dignity("Saturn", "Libra"), Dignity::Dignified);
        assert_eq!(dignity("Mars", "Gemini"), Dignity::Peregrine);
        assert_eq!(sign_of(359.99), "Pisces");
        assert_eq!(sign_of(-0.5), "Pisces");
    }

    /// The full de440s kernel, when it has been fetched (scripts/fetch-kernels.sh).
    fn kernels() -> Option<KernelSet> {
        use crate::ephemeris::{Kernel, Spk};
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../kernels/de440s.bsp");
        let spk = Spk::open(path).ok()?;
        let mut set = KernelSet::new();
        set.push(Kernel::new("de440s.bsp", spk).ok()?);
        Some(set)
    }

    #[test]
    fn interpolated_moments_match_exact_ones() {
        let Some(kernels) = kernels() else {
            eprintln!("skipping: kernels/de440s.bsp not found");
            return;
        };
        for (zodiac, system) in [("tropical", "P"), ("sidereal", "W")] {
            let mut origin = ChartRequest::new(
                UtcInstant::parse("2026-10-01T00:00:00Z").unwrap(),
                41.9,
                12.5,
            );
            origin.zodiac_type = zodiac;
            origin.house_system = system;
            origin.ayanamsa = "lahiri";
            let mut sampler = Sampler::new(&kernels, &origin);
            let criteria = ElectionCriteria::default().resolve().unwrap();
            let hours = hours("Sun", true);
            let (mut same, mut total) = (0, 0);
            // Every 7 minutes for four days: off the grid most of the time.
            for k in 0..(4 * 24 * 60 / 7) {
                let instant = origin.instant.add_micros(k * 7 * 60_000_000);
                let (planets, cusps) = sampler.at(instant).unwrap();
                let exact = sky(
                    &kernels,
                    &ChartRequest {
                        instant,
                        ..origin.clone()
                    },
                )
                .unwrap();
                for (p, e) in planets.iter().zip(&exact.planets) {
                    assert_eq!(p.name, e.name);
                    let off = separation(p.ecliptic_longitude, e.ecliptic_longitude) * 3600.0;
                    assert!(off < 1.0, "{} off by {off}\" at {instant:?}", p.name);
                    if k % (120 / 7 + 1) == 0 && instant.micros() % 7_200_000_000 == 0 {
                        assert_eq!(p, e, "grid points are exact");
                    }
                }
                for (c, e) in cusps.iter().zip(&exact.cusps) {
                    assert!(separation(*c, *e) * 3600.0 < 0.1, "cusp off at {instant:?}");
                }
                let moment = |planets, cusps| Moment {
                    instant,
                    planets,
                    cusps,
                    hours: &hours,
                };
                let a = assess(&moment(&planets, &cusps), &criteria).unwrap();
                let b = assess(&moment(&exact.planets, &exact.cusps), &criteria).unwrap();
                if codes(&a) != codes(&b) {
                    eprintln!("{instant:?}: {:?} vs {:?}", codes(&a), codes(&b));
                }
                same += usize::from(codes(&a) == codes(&b));
                total += 1;
            }
            assert!(
                same * 1000 >= total * 998,
                "{zodiac}: {same}/{total} moments agree"
            );
        }
    }
}
