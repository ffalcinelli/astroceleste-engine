//! Ephemeris sources. Everything above this layer (houses, aspects, lots, stars, …) is
//! independent of where planetary positions come from.

pub mod spk;

pub use spk::{jd_tdb_to_et, Segment, Spk, SpkError, State};
