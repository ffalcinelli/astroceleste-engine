//! Vedic astrology (Jyotish): the sidereal placements, nakshatras, divisional charts
//! (vargas) and Vimshottari dasha of a chart ([`jyotish`], no kernel needed), and the
//! panchanga of a moment ([`calendar`], from the kernels).
//!
//! Not part of the reference implementation. Results carry stable codes only (lowercase
//! ASCII transliterations for nakshatras, tithis, yogas, karanas, months and years, the
//! engine's names for bodies and signs: "North Node" is Rahu, "South Node" Ketu); wording
//! belongs to the application. Parts are ported from Priyank Gahtori's Khagole fork.

pub(crate) mod calendar;
pub(crate) mod dasha;
pub(crate) mod varga;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::EngineError;
use crate::instant::UtcInstant;
use crate::zodiac::{ayanamsa_info, AYANAMSA_CATALOG, ZODIAC_SIGNS};
use dasha::{vimshottari, Vimshottari};
use varga::{varga_position, Hora, VARGAS};

/// The 27 nakshatras from Ashwini, 13°20′ each from 0° sidereal Aries.
pub const NAKSHATRAS: [&str; 27] = [
    "ashwini",
    "bharani",
    "krittika",
    "rohini",
    "mrigashira",
    "ardra",
    "punarvasu",
    "pushya",
    "ashlesha",
    "magha",
    "purva_phalguni",
    "uttara_phalguni",
    "hasta",
    "chitra",
    "swati",
    "vishakha",
    "anuradha",
    "jyeshtha",
    "mula",
    "purva_ashadha",
    "uttara_ashadha",
    "shravana",
    "dhanishta",
    "shatabhisha",
    "purva_bhadrapada",
    "uttara_bhadrapada",
    "revati",
];

/// Span of a nakshatra, degrees.
pub(crate) const NAKSHATRA_SPAN: f64 = 360.0 / 27.0;

/// The Vimshottari lords in order with their years: Ashwini's lord (Ketu) first, then
/// repeating every nine nakshatras.
pub(crate) const DASHA_LORDS: [(&str, f64); 9] = [
    ("South Node", 7.0),
    ("Venus", 20.0),
    ("Sun", 6.0),
    ("Moon", 10.0),
    ("Mars", 7.0),
    ("North Node", 18.0),
    ("Jupiter", 16.0),
    ("Saturn", 19.0),
    ("Mercury", 17.0),
];

/// The nine grahas, in the order results list them.
const GRAHAS: [&str; 9] = [
    "Sun",
    "Moon",
    "Mars",
    "Mercury",
    "Jupiter",
    "Venus",
    "Saturn",
    "North Node",
    "South Node",
];
const OUTER_PLANETS: [&str; 3] = ["Uranus", "Neptune", "Pluto"];

/// The nakshatra of a sidereal longitude.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Nakshatra {
    /// 1 (Ashwini) to 27 (Revati).
    pub number: u8,
    /// Stable code, e.g. "ashwini".
    pub code: &'static str,
    /// Its Vimshottari lord.
    pub lord: &'static str,
    /// Quarter, 1 to 4.
    pub pada: u8,
    /// Fraction of the nakshatra already crossed, 0 to 1.
    pub elapsed: f64,
}

/// The nakshatra of sidereal longitude `longitude` (degrees).
pub fn nakshatra(longitude: f64) -> Nakshatra {
    let longitude = longitude.rem_euclid(360.0);
    let index = ((longitude / NAKSHATRA_SPAN) as usize).min(26);
    let elapsed = ((longitude - index as f64 * NAKSHATRA_SPAN) / NAKSHATRA_SPAN).clamp(0.0, 1.0);
    Nakshatra {
        number: index as u8 + 1,
        code: NAKSHATRAS[index],
        lord: DASHA_LORDS[index % 9].0,
        pada: ((elapsed * 4.0) as u8).min(3) + 1,
        elapsed,
    }
}

/// What [`jyotish`] computes; every field is optional in JSON.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct JyotishOptions {
    /// Ayanamsa code from the catalog (default "lahiri"); unknown codes are an error.
    pub ayanamsa: String,
    /// Also list Uranus, Neptune and Pluto (default `false`).
    pub outer_planets: bool,
    /// The vargas, by division (default all of [`VARGAS`]).
    pub vargas: Option<Vec<u8>>,
    /// The hora (D2): "parashara" (default) or "cyclic".
    pub hora: String,
    /// The dasha year: "julian" (365.25 days, default), "gregorian", "sidereal" or "savana".
    pub dasha_year: String,
    /// The lords from a mahadasha down whose subperiods to list (default: the mahadashas).
    pub dasha_path: Vec<String>,
    /// Levels of the dasha tree below `dasha_path`, 1 to 3 (default 2).
    pub dasha_depth: usize,
    /// List the dasha periods running at this moment (ISO 8601 UTC).
    pub date: Option<String>,
}

impl Default for JyotishOptions {
    fn default() -> Self {
        JyotishOptions {
            ayanamsa: "lahiri".into(),
            outer_planets: false,
            vargas: None,
            hora: "parashara".into(),
            dasha_year: "julian".into(),
            dasha_path: Vec::new(),
            dasha_depth: 2,
            date: None,
        }
    }
}

/// A body or the Ascendant in the sidereal zodiac.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Graha {
    /// Body name, or "Ascendant".
    pub name: &'static str,
    /// Sidereal longitude, degrees.
    pub longitude: f64,
    /// Sign name, e.g. "Taurus".
    pub sign: &'static str,
    /// Degrees within the sign.
    pub degree: f64,
    /// Whole-sign house from the Ascendant (1 for the Ascendant itself).
    pub house: u8,
    /// Whether the body is retrograde.
    pub is_retrograde: bool,
    /// Its nakshatra.
    pub nakshatra: Nakshatra,
    /// In the same sign in the navamsha (D9) as in the rashi (D1).
    pub vargottama: bool,
}

/// A placement in a divisional chart.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VargaPlacement {
    /// Body name, or "Ascendant".
    pub name: &'static str,
    /// Sign name, e.g. "Taurus".
    pub sign: &'static str,
    /// Degrees within the varga sign.
    pub degree: f64,
    /// Whole-sign house from the varga's Ascendant.
    pub house: u8,
}

/// A divisional chart: the Ascendant first, then the grahas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Varga {
    /// "D1" to "D60".
    pub code: String,
    /// Parts per sign.
    pub division: u8,
    /// The placements.
    pub placements: Vec<VargaPlacement>,
}

/// The Vedic reading of a chart: [`jyotish`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Jyotish {
    /// Ayanamsa code.
    pub ayanamsa: &'static str,
    /// Degrees at the birth.
    pub ayanamsa_value: f64,
    /// The Ascendant first, then the grahas (and the outer planets when asked).
    pub grahas: Vec<Graha>,
    /// The divisional charts asked for.
    pub vargas: Vec<Varga>,
    /// The Vimshottari dasha.
    pub vimshottari: Vimshottari,
}

fn invalid(what: &str) -> EngineError {
    EngineError::InvalidInput(format!("jyotish: {what}"))
}

fn whole_sign_house(sign: usize, ascendant_sign: usize) -> u8 {
    ((sign + 12 - ascendant_sign) % 12) as u8 + 1
}

/// The sidereal placements, nakshatras, vargas and Vimshottari dasha of `chart` (the
/// chart JSON, tropical or sidereal under any ayanamsa) born at `birth`, under the
/// ayanamsa of `options`.
pub fn jyotish(
    chart: &Value,
    birth: UtcInstant,
    options: &JyotishOptions,
) -> Result<Jyotish, EngineError> {
    let code = options.ayanamsa.trim().to_lowercase();
    if !AYANAMSA_CATALOG.iter().any(|a| a.code == code) {
        return Err(invalid(&format!("unknown ayanamsa {}", options.ayanamsa)));
    }
    let jd = birth.julian_day();
    let info = ayanamsa_info(jd, &code);
    // The chart's own longitudes back to tropical, then to the requested ayanamsa.
    let chart_shift = if chart.get("zodiac_type").and_then(Value::as_str) == Some("sidereal") {
        chart
            .get("ayanamsa_value")
            .and_then(Value::as_f64)
            .ok_or_else(|| invalid("a sidereal chart needs its ayanamsa_value"))?
    } else {
        0.0
    };
    let planets = chart
        .get("planets")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("the chart has no planets"))?;
    let find = |name: &str| -> Option<(f64, bool)> {
        let p = planets
            .iter()
            .find(|p| p.get("name").and_then(Value::as_str) == Some(name))?;
        let longitude = p.get("ecliptic_longitude")?.as_f64()?;
        longitude.is_finite().then(|| {
            (
                (longitude + chart_shift - info.value).rem_euclid(360.0),
                p.get("is_retrograde")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
        })
    };

    let hora = Hora::from_code(&options.hora);
    let names = std::iter::once("Ascendant")
        .chain(GRAHAS)
        .chain(OUTER_PLANETS.into_iter().filter(|_| options.outer_planets));
    let points: Vec<(&'static str, f64, bool)> = names
        .filter_map(|name| find(name).map(|(lon, retro)| (name, lon, retro)))
        .collect();
    let (ascendant, moon) = (
        points.iter().find(|p| p.0 == "Ascendant"),
        points.iter().find(|p| p.0 == "Moon"),
    );
    let (Some(&(_, ascendant, _)), Some(&(_, moon, _))) = (ascendant, moon) else {
        return Err(invalid("the chart needs the Ascendant and the Moon"));
    };

    let sign_of = |lon: f64| ((lon / 30.0) as usize).min(11);
    let ascendant_sign = sign_of(ascendant);
    let grahas = points
        .iter()
        .map(|&(name, longitude, is_retrograde)| {
            let sign = sign_of(longitude);
            Ok(Graha {
                name,
                longitude,
                sign: ZODIAC_SIGNS[sign].name,
                degree: longitude - sign as f64 * 30.0,
                house: whole_sign_house(sign, ascendant_sign),
                is_retrograde,
                nakshatra: nakshatra(longitude),
                vargottama: varga_position(longitude, 9, hora)?.0 == sign,
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;

    let divisions = options.vargas.clone().unwrap_or_else(|| VARGAS.to_vec());
    let vargas = divisions
        .iter()
        .map(|&n| {
            let ascendant_sign = varga_position(ascendant, n, hora)?.0;
            let placements = points
                .iter()
                .map(|&(name, longitude, _)| {
                    let (sign, degree) = varga_position(longitude, n, hora)?;
                    Ok(VargaPlacement {
                        name,
                        sign: ZODIAC_SIGNS[sign].name,
                        degree,
                        house: whole_sign_house(sign, ascendant_sign),
                    })
                })
                .collect::<Result<Vec<_>, EngineError>>()?;
            Ok(Varga {
                code: format!("D{n}"),
                division: n,
                placements,
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;

    let date = options
        .date
        .as_deref()
        .map(UtcInstant::parse)
        .transpose()
        .map_err(|e| invalid(&e.to_string()))?;
    let vimshottari = vimshottari(
        birth,
        moon,
        &options.dasha_year,
        &options.dasha_path,
        options.dasha_depth,
        date,
    )?;

    Ok(Jyotish {
        ayanamsa: info.code,
        ayanamsa_value: info.value,
        grahas,
        vargas,
        vimshottari,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nakshatra_boundaries() {
        let n = nakshatra(0.0);
        assert_eq!(
            (n.number, n.code, n.lord, n.pada),
            (1, "ashwini", "South Node", 1)
        );
        assert_eq!(nakshatra(3.34).pada, 2);
        let n = nakshatra(26.7);
        assert_eq!((n.code, n.lord), ("krittika", "Sun"));
        let n = nakshatra(359.9);
        assert_eq!(
            (n.number, n.code, n.lord, n.pada),
            (27, "revati", "Mercury", 4)
        );
        assert_eq!(nakshatra(-1e-17).number, 27); // just before 0°: the end of Revati
    }

    fn chart(zodiac: &str, shift: f64) -> Value {
        let at = |lon: f64| (lon - shift).rem_euclid(360.0);
        json!({
            "zodiac_type": zodiac,
            "ayanamsa_value": shift,
            "planets": [
                {"name": "Sun", "ecliptic_longitude": at(100.0), "is_retrograde": false},
                {"name": "Moon", "ecliptic_longitude": at(200.0), "is_retrograde": false},
                {"name": "Mars", "ecliptic_longitude": at(10.0), "is_retrograde": true},
                {"name": "Ascendant", "ecliptic_longitude": at(50.0)},
            ]
        })
    }

    #[test]
    fn tropical_and_sidereal_charts_agree() {
        let birth = UtcInstant::parse("2000-01-01T12:00:00Z").unwrap();
        let options = JyotishOptions::default();
        let tropical = jyotish(&chart("tropical", 0.0), birth, &options).unwrap();
        let sidereal = jyotish(&chart("sidereal", 24.1), birth, &options).unwrap();
        assert_eq!(tropical.ayanamsa, "lahiri");
        for (a, b) in tropical.grahas.iter().zip(&sidereal.grahas) {
            assert_eq!(a.name, b.name);
            assert!((a.longitude - b.longitude).abs() < 1e-9);
        }
        let names: Vec<&str> = tropical.grahas.iter().map(|g| g.name).collect();
        assert_eq!(names, ["Ascendant", "Sun", "Moon", "Mars"]);
        assert_eq!(tropical.grahas[0].house, 1);
        assert!(tropical.grahas[3].is_retrograde);
        assert_eq!(tropical.vargas.len(), VARGAS.len());
        assert_eq!(tropical.vargas[8].code, "D9");
    }

    #[test]
    fn options_are_checked() {
        let birth = UtcInstant::parse("2000-01-01T12:00:00Z").unwrap();
        let bad_ayanamsa = JyotishOptions {
            ayanamsa: "nope".into(),
            ..JyotishOptions::default()
        };
        assert!(jyotish(&chart("tropical", 0.0), birth, &bad_ayanamsa).is_err());
        let no_moon = json!({"planets": [{"name": "Ascendant", "ecliptic_longitude": 1.0}]});
        assert!(jyotish(&no_moon, birth, &JyotishOptions::default()).is_err());
    }
}
