//! Hermetic lots / Arabic parts (`charts/calc/arabic_parts.py`).

use serde::Serialize;

use crate::aspects::Point;
use crate::pyfloat;
use crate::symbolic::symbolic_degree_number;
use crate::zodiac::{determine_house, longitude_to_zodiac};

struct LotDefinition {
    name: &'static str,
    symbol: &'static str,
    /// (base, add, subtract) by day; swapped add/subtract by night.
    day: (&'static str, &'static str, &'static str),
}

const LOTS: [LotDefinition; 11] = [
    LotDefinition {
        name: "Fortune",
        symbol: "⊗",
        day: ("Ascendant", "Moon", "Sun"),
    },
    LotDefinition {
        name: "Spirit",
        symbol: "🜕",
        day: ("Ascendant", "Sun", "Moon"),
    },
    LotDefinition {
        name: "Eros",
        symbol: "🏹",
        day: ("Ascendant", "Venus", "Spirit"),
    },
    LotDefinition {
        name: "Necessity",
        symbol: "⛓",
        day: ("Ascendant", "Fortune", "Mercury"),
    },
    LotDefinition {
        name: "Courage",
        symbol: "🛡",
        day: ("Ascendant", "Fortune", "Mars"),
    },
    LotDefinition {
        name: "Victory",
        symbol: "🏆",
        day: ("Ascendant", "Jupiter", "Fortune"),
    },
    LotDefinition {
        name: "Nemesis",
        symbol: "⚖",
        day: ("Ascendant", "Fortune", "Saturn"),
    },
    LotDefinition {
        name: "Marriage",
        symbol: "💍",
        day: ("Ascendant", "Venus", "Saturn"),
    },
    LotDefinition {
        name: "Commerce",
        symbol: "☤",
        day: ("Ascendant", "Mercury", "Sun"),
    },
    LotDefinition {
        name: "Sickness",
        symbol: "⚕",
        day: ("Ascendant", "Mars", "Saturn"),
    },
    LotDefinition {
        name: "Death",
        symbol: "☠",
        day: ("Ascendant", "House 8", "Moon"),
    },
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lot {
    pub name: &'static str,
    pub symbol: &'static str,
    pub sign: &'static str,
    pub sign_symbol: &'static str,
    pub degree: i64,
    pub minute: i64,
    pub ecliptic_longitude: f64,
    pub house: u8,
    pub formula_used: String,
    pub is_diurnal: bool,
    pub symbolic_degree: i64,
}

/// The eleven classical lots, with day/night formulas by the Sun's hemisphere.
/// `points` are the planets and angles (by name), `cusps` the twelve house cusps.
pub fn arabic_parts(points: &[Point], cusps: &[f64]) -> Vec<Lot> {
    let find = |name: &str| points.iter().find(|p| p.name == name).map(|p| p.longitude);
    let (Some(sun), Some(_)) = (find("Sun"), find("Ascendant")) else {
        return Vec::new();
    };
    let sun_house = if cusps.is_empty() {
        1
    } else {
        determine_house(sun, cusps)
    };
    let is_day = sun_house >= 7;
    let period = if is_day { "Day" } else { "Night" };

    let mut computed: Vec<(&str, f64)> = Vec::new();
    let mut out = Vec::new();
    for lot in &LOTS {
        let (base, a, b) = if is_day {
            lot.day
        } else {
            (lot.day.0, lot.day.2, lot.day.1)
        };
        let lon_of = |name: &str| {
            if let Some((_, lon)) = computed.iter().find(|(n, _)| *n == name) {
                return *lon;
            }
            if let Some(lon) = find(name) {
                return lon;
            }
            if name == "House 8" && cusps.len() >= 8 {
                return cusps[7];
            }
            0.0
        };
        let lon = pyfloat::rem(lon_of(base) + lon_of(a) - lon_of(b), 360.0);
        computed.push((lot.name, lon));
        let z = longitude_to_zodiac(lon);
        let formula_used = if base == "Ascendant" {
            format!("ASC + {a} - {b} ({period})")
        } else {
            format!("{base} + {a} - {b} ({period})")
        };
        out.push(Lot {
            name: lot.name,
            symbol: lot.symbol,
            sign: z.sign.name,
            sign_symbol: z.sign.symbol,
            degree: z.degree,
            minute: z.minute,
            ecliptic_longitude: lon,
            house: determine_house(lon, cusps),
            formula_used,
            is_diurnal: is_day,
            symbolic_degree: symbolic_degree_number(z.degree, z.minute),
        });
    }
    out
}
