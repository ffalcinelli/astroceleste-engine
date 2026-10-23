//! Moments when the Sun and the Moon reach given apparent longitudes: the solar terms and
//! new moons of the Chinese calendar, the angas and months of the Vedic one.

use crate::ephemeris::observe::{apparent, ecliptic_latlon, observe};
use crate::ephemeris::KernelSet;
use crate::error::EngineError;
use crate::frames::nutation::iau2000b_radians;
use crate::frames::Orientation;
use crate::instant::UtcInstant;
use crate::planets::require_kernel;
use crate::pyfloat::wrap180;
use crate::time::Time;

pub(crate) const SUN: i32 = 10;
pub(crate) const MOON: i32 = 301;
pub(crate) const TROPICAL_YEAR: f64 = 365.242_19;
pub(crate) const SYNODIC_MONTH: f64 = 29.530_589;
/// Mean motion of the Sun, degrees per day.
pub(crate) const SUN_RATE: f64 = 360.0 / TROPICAL_YEAR;
/// Mean motion of the Moon away from the Sun, degrees per day.
pub(crate) const ELONGATION_RATE: f64 = 360.0 / SYNODIC_MONTH;
/// Search precision, days (under 0.01 s).
pub(crate) const EPSILON_DAYS: f64 = 1e-7;
/// Julian date of the Unix epoch.
const UNIX_EPOCH_JD: f64 = 2_440_587.5;

/// The instant of a UT Julian date, to the second.
pub(crate) fn instant_of(jd: f64) -> UtcInstant {
    let seconds = ((jd - UNIX_EPOCH_JD) * 86_400.0).round() as i64;
    UtcInstant::from_micros(seconds * 1_000_000)
}

/// Apparent geocentric positions for the searches.
pub(crate) struct Sky<'a> {
    pub(crate) kernels: &'a KernelSet,
}

impl Sky<'_> {
    /// Apparent ecliptic longitude of date of `target` at UT Julian date `jd`, degrees.
    pub(crate) fn longitude(&self, target: i32, jd: f64) -> Result<f64, EngineError> {
        let kernel = require_kernel(self.kernels, jd)?;
        let t = Time::from_ut1(jd);
        // The faster IAU 2000B nutation: it moves a term by well under a second.
        let orientation = Orientation::with_nutation(&t, iau2000b_radians(t.tt()));
        let position = apparent(kernel, &observe(kernel, target, &t)?, &t)?;
        Ok(ecliptic_latlon(&orientation, &position).1)
    }

    pub(crate) fn sun(&self, jd: f64) -> Result<f64, EngineError> {
        self.longitude(SUN, jd)
    }

    pub(crate) fn moon(&self, jd: f64) -> Result<f64, EngineError> {
        self.longitude(MOON, jd)
    }

    pub(crate) fn elongation(&self, jd: f64) -> Result<f64, EngineError> {
        Ok(self.moon(jd)? - self.sun(jd)?)
    }

    /// When the Sun reaches `longitude`, starting from `guess` (secant method).
    pub(crate) fn sun_at(&self, longitude: f64, guess: f64) -> Result<f64, EngineError> {
        crossing(|jd| self.sun(jd), longitude, guess, SUN_RATE)
    }

    /// The new moon nearest `guess`.
    pub(crate) fn new_moon_near(&self, guess: f64) -> Result<f64, EngineError> {
        crossing(|jd| self.elongation(jd), 0.0, guess, ELONGATION_RATE)
    }

    /// The last moment at or before `jd` when the Sun was at `longitude`.
    pub(crate) fn sun_before(&self, longitude: f64, jd: f64) -> Result<f64, EngineError> {
        let behind = (self.sun(jd)? - longitude).rem_euclid(360.0);
        let found = self.sun_at(longitude, jd - behind / SUN_RATE)?;
        if found > jd {
            self.sun_at(longitude, found - TROPICAL_YEAR)
        } else {
            Ok(found)
        }
    }

    /// The last new moon at or before `jd`.
    pub(crate) fn new_moon_before(&self, jd: f64) -> Result<f64, EngineError> {
        let behind = self.elongation(jd)?.rem_euclid(360.0);
        let found = self.new_moon_near(jd - behind / ELONGATION_RATE)?;
        if found > jd {
            self.new_moon_near(found - SYNODIC_MONTH)
        } else {
            Ok(found)
        }
    }
}

/// The moment near `guess` when the angle `f` (degrees, growing at about `rate` degrees
/// per day) equals `target`.
pub(crate) fn crossing(
    f: impl Fn(f64) -> Result<f64, EngineError>,
    target: f64,
    guess: f64,
    rate: f64,
) -> Result<f64, EngineError> {
    let mut t0 = guess;
    let mut d0 = wrap180(f(t0)? - target);
    let mut t1 = t0 - d0 / rate;
    for _ in 0..50 {
        if (t1 - t0).abs() < EPSILON_DAYS {
            break;
        }
        let d1 = wrap180(f(t1)? - target);
        let slope = (d1 - d0) / (t1 - t0);
        // The angle always grows near the crossing; fall back to the mean rate when the
        // secant says otherwise (a wrap between the samples).
        let slope = if slope.is_finite() && slope > rate / 4.0 {
            slope
        } else {
            rate
        };
        (t0, d0) = (t1, d1);
        t1 -= d1 / slope;
    }
    Ok(t1)
}
