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
//!
//! # Entry points
//!
//! | Function | Result |
//! |---|---|
//! | [`calculate_chart`] | a natal or event [`Chart`]: planets, houses, aspects, fixed stars, lots, temperament, lunar status |
//! | [`calculate_horary_chart`] | the chart plus [`HoraryData`]: planetary hours, significators, the Moon's aspects, strictures |
//! | [`calculate_transit_chart`] | the sky at a moment and place, with its [`CrossAspect`]s to natal planets |
//! | [`calculate_synastry`] | cross-aspects between two charts' planets (no kernel needed) |
//! | [`calculate_derived_chart`] | a stored chart turned to a new first house (no kernel needed) |
//! | [`calculate_election_chart`] | the chart plus [`ElectionData`]: an electional score with the rules that apply |
//! | [`search_elections`] | the best [`ElectionWindow`]s over a span of time at a place |
//!
//! Every result implements [`serde::Serialize`] and serializes to the JSON of the
//! Astroceleste API, with the same key order and the same integer vs float types. The
//! Python and WebAssembly bindings return exactly that JSON.
//!
//! # Loading kernels
//!
//! Positions come from NASA JPL SPK kernels (`de440s.bsp` covers 1849–2150; DE441 covers
//! 13200 BC–17191). A [`KernelSet`](ephemeris::KernelSet) holds them in preference order,
//! and each date is computed with the first kernel that covers it. [`Spk::open`] reads a
//! file. Where there is no file system (WebAssembly) or the kernel is bundled with an app,
//! load the bytes and use [`Spk::from_bytes`]. [`Spk::excerpt`] cuts a smaller kernel for a
//! date range, with positions unchanged inside it (1950–2050 of DE440s is about 11 MB).
//!
//! ```no_run
//! use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
//!
//! # fn download(_: &str) -> Vec<u8> { Vec::new() }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let bytes: Vec<u8> = download("https://example.com/de440s-1950-2050.bsp");
//! let mut kernels = KernelSet::new();
//! kernels.push(Kernel::new("de440s-1950-2050.bsp", Spk::from_bytes(bytes)?)?);
//! assert!(kernels.coverage().is_some());
//! # Ok(())
//! # }
//! ```
//!
//! # Errors
//!
//! Calculations return [`EngineError`], whose [`code`](EngineError::code) is the stable
//! error code of the Astroceleste API. A date that no loaded kernel covers is
//! [`EngineError::OutOfRange`]: the engine never extrapolates or approximates.
//!
//! ```no_run
//! # use astroceleste_engine::ephemeris::KernelSet;
//! # use astroceleste_engine::{calculate_chart, ChartRequest, EngineError, UtcInstant};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let kernels = KernelSet::new();
//! let request = ChartRequest::new(UtcInstant::parse("1700-01-01T00:00:00Z")?, 41.9, 12.5);
//! match calculate_chart(&kernels, &request) {
//!     Ok(chart) => println!("{} planets", chart.planets.len()),
//!     Err(EngineError::OutOfRange { jd, coverage }) => {
//!         eprintln!("JD {jd} is outside the loaded kernels ({coverage:?})")
//!     }
//!     Err(err) => eprintln!("{}: {err}", err.code()),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Platforms
//!
//! The crate is pure Rust with no C code, and depends only on `serde` and `serde_json`.
//! It builds for servers and desktops, `wasm32-unknown-unknown`, Android and iOS. Python
//! (`pip install astroceleste-engine`) and JavaScript (`npm install astroceleste-engine`)
//! bindings are published from the same repository.
//!
//! # Further reading
//!
//! - [API guide](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/api.md):
//!   request options (house systems, ayanamsas, orb settings) and the chart JSON
//! - [Ephemerides](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/ephemerides.md):
//!   kernels, coverage and excerpts
//! - [Accuracy](https://github.com/ffalcinelli/astroceleste-engine/blob/main/docs/accuracy.md):
//!   how results are verified against the reference implementation
//! - [Live demo](https://ffalcinelli.github.io/astroceleste-engine/): this crate compiled
//!   to WebAssembly, computing charts in your browser
//!
//! [`Spk::open`]: ephemeris::Spk::open
//! [`Spk::from_bytes`]: ephemeris::Spk::from_bytes
//! [`Spk::excerpt`]: ephemeris::Spk::excerpt

mod almanac;
mod aspects;
mod catalog;
mod chart;
mod chiron;
mod constants;
mod derived;
mod election;
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
pub use election::{
    calculate_election_chart, search_elections, CriteriaSummary, ElectionChart, ElectionCriteria,
    ElectionData, ElectionFactor, ElectionSearch, ElectionWindow, Exclusions, HourRange,
    NatalPoint, Purpose, UtcOffset, MAX_SEARCH_DAYS,
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
