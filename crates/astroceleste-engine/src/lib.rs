//! astroceleste-engine: astrological chart calculation on JPL ephemerides.
//!
//! One implementation shared by the Astroceleste server (Python bindings), desktop and
//! mobile apps (native) and the web app (WASM), so every platform computes identical charts.

pub mod ephemeris;
