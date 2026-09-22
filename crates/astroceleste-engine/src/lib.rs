//! astroceleste-engine: astrological chart calculation on JPL ephemerides.
//!
//! One implementation shared by the Astroceleste server (Python bindings), desktop and
//! mobile apps (native) and the web app (WASM), so every platform computes identical charts.
//!
//! **Experimental (0.0.x):** the API may change in any release.
//!
//! ```no_run
//! use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
//! use astroceleste_engine::{calculate_chart, ChartRequest, UtcInstant};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Kernels in preference order: the first one covering a date is used.
//! let mut kernels = KernelSet::new();
//! kernels.push(Kernel::new("de440s.bsp", Spk::open("kernels/de440s.bsp")?)?);
//!
//! let mut request = ChartRequest::new(UtcInstant::parse("1987-05-17T14:30:00Z")?, 41.9, 12.5);
//! request.house_system = "P";
//! request.zodiac_type = "sidereal";
//! request.ayanamsa = "lahiri";
//!
//! let chart = calculate_chart(&kernels, &request)?;
//! println!("{}", serde_json::to_string_pretty(&chart)?);
//! # Ok(())
//! # }
//! ```

mod almanac;
mod aspects;
mod catalog;
mod chart;
mod chiron;
mod constants;
mod derived;
pub mod ephemeris;
mod error;
mod fixed_stars;
#[doc(hidden)] // exposed for the reduction tests; not part of the API
pub mod frames;
mod horary;
mod houses;
mod instant;
mod lots;
mod lunar;
mod planets;
mod pyfloat;
mod symbolic;
mod temperament;
#[doc(hidden)] // exposed for the reduction tests; not part of the API
pub mod time;
mod zodiac;

pub use aspects::{Aspect, CrossAspect};
pub use catalog::LunarMansion;
pub use chart::{calculate_chart, Chart, ChartRequest, HouseCusp, Placement};
pub use derived::{
    calculate_derived_chart, calculate_synastry, calculate_transit_chart, Synastry, TransitChart,
};
pub use error::EngineError;
pub use fixed_stars::FixedStarPosition;
pub use horary::{
    calculate_horary_chart, ApplyingAspect, HoraryChart, HoraryData, MoonStatus, PlanetaryHours,
    SeparatingAspect, Stricture,
};
pub use instant::{ParseError, UtcInstant};
pub use lots::Lot;
pub use lunar::LunarStatus;
pub use temperament::{Factor, Qualities, Scores, Temperament};
