//! astroceleste-engine: astrological chart calculation on JPL ephemerides.
//!
//! One implementation shared by the Astroceleste server (Python bindings), desktop and
//! mobile apps (native) and the web app (WASM), so every platform computes identical charts.
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

pub mod almanac;
pub mod aspects;
pub mod catalog;
pub mod chart;
pub mod chiron;
pub mod constants;
pub mod derived;
pub mod ephemeris;
pub mod error;
pub mod fixed_stars;
pub mod frames;
pub mod horary;
pub mod houses;
pub mod instant;
pub mod lots;
pub mod lunar;
pub mod planets;
pub mod pyfloat;
pub mod symbolic;
pub mod temperament;
pub mod time;
pub mod zodiac;

pub use chart::{calculate_chart, Chart, ChartRequest};
pub use derived::{calculate_derived_chart, calculate_synastry, calculate_transit_chart};
pub use error::EngineError;
pub use horary::{calculate_horary_chart, HoraryChart};
pub use instant::UtcInstant;
