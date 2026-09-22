//! Ephemeris sources. Everything above this layer (houses, aspects, lots, stars, …) is
//! independent of where planetary positions come from.

mod kernels;
#[doc(hidden)] // exposed for the reduction tests; not part of the API
pub mod observe;
mod spk;

pub use kernels::{Kernel, KernelSet};
pub use spk::{jd_tdb_to_et, Segment, Spk, SpkError, State};
