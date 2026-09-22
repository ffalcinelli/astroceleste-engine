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
    /// Mansion number (1-28).
    pub number: u8,
    /// Arabic name.
    pub arabic_name: &'static str,
    /// English name.
    pub name: &'static str,
    /// Sign or signs the mansion spans, e.g. "Aries - Taurus".
    pub sign: &'static str,
    /// Start, ecliptic longitude in degrees.
    pub degree_start: f64,
    /// End, ecliptic longitude in degrees.
    pub degree_end: f64,
}

/// What a derived house signifies, in English and Italian.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Meaning {
    pub en: &'static str,
    pub it: &'static str,
}
