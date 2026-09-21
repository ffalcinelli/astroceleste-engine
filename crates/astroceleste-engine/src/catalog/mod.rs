//! Reference tables: fixed stars, lunar mansions, derived-house meanings.

use serde::Serialize;

#[allow(clippy::all)]
#[rustfmt::skip]
mod derived_meanings;
#[allow(clippy::all)]
#[rustfmt::skip]
mod fixed_stars;
#[allow(clippy::all)]
#[rustfmt::skip]
mod mansions;

pub use derived_meanings::{DERIVED_HOUSE_MEANINGS, ROOT_HOUSE_THEMES};
pub use fixed_stars::FIXED_STARS;
pub use mansions::LUNAR_MANSIONS;

/// A star with its tropical ecliptic position at J2000.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedStar {
    pub name: &'static str,
    pub symbol: &'static str,
    pub j2000_lon: f64,
    pub j2000_lat: f64,
    pub magnitude: f64,
}

/// One of the 28 lunar mansions (manazil al-qamar).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LunarMansion {
    pub number: u8,
    pub arabic_name: &'static str,
    pub name: &'static str,
    pub sign: &'static str,
    pub degree_start: f64,
    pub degree_end: f64,
}

/// What a derived house signifies, in English and Italian.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Meaning {
    pub en: &'static str,
    pub it: &'static str,
}
