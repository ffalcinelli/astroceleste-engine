//! Planetary positions (`SkyfieldEngine.calculate_planets`): apparent geocentric
//! longitudes of date from the JPL kernels, the mean or true lunar nodes, Chiron, Black
//! Moon Lilith and the minor bodies (main asteroids and dwarf planets).

use crate::ephemeris::observe::{apparent, ecliptic_latlon, observe, EARTH};
use crate::ephemeris::{Kernel, KernelSet};
use crate::error::EngineError;
use crate::frames::{mxv, Orientation};
use crate::minor_bodies::{chiron, minor_body, minor_body_symbol, MINOR_BODIES};
use crate::pyfloat;
use crate::time::Time;

#[derive(Debug, Clone, PartialEq)]
pub struct BodyPosition {
    pub name: &'static str,
    pub symbol: &'static str,
    /// Degrees, tropical or sidereal as requested.
    pub longitude: f64,
    pub latitude: f64,
    /// au
    pub distance: f64,
    /// Degrees per day.
    pub speed: f64,
    pub is_retrograde: bool,
}

/// (name, NAIF code, symbol, can be retrograde)
const TARGETS: [(&str, i32, &str, bool); 10] = [
    ("Sun", 10, "☉", false),
    ("Moon", 301, "☽", false),
    ("Mercury", 199, "☿", true),
    ("Venus", 299, "♀", true),
    ("Mars", 4, "♂", true),
    ("Jupiter", 5, "♃", true),
    ("Saturn", 6, "♄", true),
    ("Uranus", 7, "♅", true),
    ("Neptune", 8, "♆", true),
    ("Pluto", 9, "♇", true),
];

/// The kernel covering `jd`, raising when none does (`SkyfieldEngine._require_eph`).
pub fn require_kernel(kernels: &KernelSet, jd: f64) -> Result<&Kernel, EngineError> {
    kernels.for_jd(jd).ok_or(EngineError::OutOfRange {
        jd,
        coverage: kernels.coverage(),
    })
}

/// Which lunar node a chart carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum NodeType {
    /// The mean node (IAU polynomial), moving steadily backwards.
    #[default]
    Mean,
    /// The true (osculating) node: where the Moon's instantaneous orbit crosses the
    /// ecliptic of date, from the kernel's state vectors. It wobbles about the mean node
    /// by up to 1.7° and is sometimes direct.
    True,
}

impl NodeType {
    /// "true" or "osculating" (any case) is the true node; anything else is the mean one.
    pub fn from_code(code: &str) -> Self {
        let code = code.trim();
        if code.eq_ignore_ascii_case("true") || code.eq_ignore_ascii_case("osculating") {
            NodeType::True
        } else {
            NodeType::Mean
        }
    }

    /// The code charts echo: "mean" or "true".
    pub fn code(self) -> &'static str {
        match self {
            NodeType::Mean => "mean",
            NodeType::True => "true",
        }
    }
}

pub struct Planets {
    /// Bodies that could not be computed (e.g. Chiron outside its table) are left out.
    pub bodies: Vec<BodyPosition>,
}

/// All chart bodies at UT1 Julian date `jd`, shifted by `shift` degrees (the ayanamsa for
/// sidereal charts, 0 for tropical), with the Greenwich apparent sidereal time (hours)
/// at `jd`. `node` selects the mean or the true lunar nodes.
pub fn planets_and_sidereal_time(
    kernels: &KernelSet,
    jd: f64,
    shift: f64,
    node: NodeType,
) -> Result<(Planets, f64), EngineError> {
    let sample = SpeedSample::at(kernels, jd)?;
    let mut bodies = Vec::with_capacity(14 + MINOR_BODIES.len());

    for (name, code, symbol, can_retrograde) in TARGETS {
        let Ok((lat, lon, dist, speed)) = sample.observe(code) else {
            continue;
        };
        bodies.push(BodyPosition {
            name,
            symbol,
            longitude: pyfloat::rem(lon - shift, 360.0),
            latitude: lat,
            distance: dist,
            speed,
            is_retrograde: can_retrograde && speed < 0.0,
        });
    }

    // Mean lunar ascending node (IAU polynomial), also the base of Lilith below.
    let t_cent = (jd - 2_451_545.0) / 36525.0;
    let omega = pyfloat::rem(
        125.0445479 - 1934.1362891 * t_cent
            + 0.0020754 * t_cent.powf(2.0)
            + t_cent.powf(3.0) / 467440.0
            - t_cent.powf(4.0) / 60616000.0,
        360.0,
    );
    let (node_longitude, node_speed) = match node {
        NodeType::Mean => (omega, -0.05295),
        NodeType::True => sample.true_node()?,
    };
    let north = pyfloat::rem(node_longitude - shift, 360.0);
    for (name, symbol, longitude) in [
        ("North Node", "☊", north),
        ("South Node", "☋", pyfloat::rem(north + 180.0, 360.0)),
    ] {
        bodies.push(BodyPosition {
            name,
            symbol,
            longitude,
            latitude: 0.0,
            distance: 0.00257,
            speed: node_speed,
            is_retrograde: node_speed < 0.0,
        });
    }

    if let Some(c) = chiron(jd, shift) {
        bodies.push(BodyPosition {
            name: "Chiron",
            symbol: "⚷",
            longitude: c.longitude,
            latitude: 0.0,
            distance: 0.0,
            speed: c.speed,
            is_retrograde: c.is_retrograde,
        });
    }

    // Black Moon Lilith: mean lunar apogee projected from the lunar orbit onto the ecliptic.
    let perigee = pyfloat::rem(
        83.3532465 + 4069.0137287 * t_cent
            - 0.01032 * t_cent.powf(2.0)
            - t_cent.powf(3.0) / 80053.0,
        360.0,
    );
    let apogee = pyfloat::rem(perigee + 180.0, 360.0);
    let u = pyfloat::rem(apogee - omega, 360.0).to_radians();
    let inc = 5.1453964_f64.to_radians();
    let proj_u = (u.sin() * inc.cos()).atan2(u.cos());
    bodies.push(BodyPosition {
        name: "Lilith",
        symbol: "⚸",
        longitude: pyfloat::rem(proj_u.to_degrees() + omega - shift, 360.0),
        latitude: 0.0,
        distance: 0.0027,
        speed: 0.1114,
        is_retrograde: false,
    });

    // Minor bodies, from their tables (indexed by TT).
    for name in MINOR_BODIES {
        if let Some(m) = minor_body(name, sample.t.tt(), shift) {
            bodies.push(BodyPosition {
                name,
                symbol: minor_body_symbol(name),
                longitude: m.longitude,
                latitude: 0.0,
                distance: 0.0,
                speed: m.speed,
                is_retrograde: m.is_retrograde,
            });
        }
    }

    Ok((Planets { bodies }, sample.orientation.gast_hours))
}

/// The apparent speed (degrees per day) of the planet `name` at UT1 Julian date `jd`,
/// exactly as [`planets_and_sidereal_time`] gives it, without computing the other bodies.
pub fn planet_speed(kernels: &KernelSet, jd: f64, name: &str) -> Result<f64, EngineError> {
    let (.., code, _, _) = TARGETS
        .iter()
        .find(|(n, ..)| *n == name)
        .ok_or_else(|| EngineError::InvalidInput(format!("not a planet: {name}")))?;
    Ok(SpeedSample::at(kernels, jd)?.observe(*code)?.3)
}

/// An instant and the instant an hour later (for the apparent speed and the retrograde
/// flag), with the kernel and the orientations of both.
struct SpeedSample<'k> {
    kernel: &'k Kernel,
    t: Time,
    t_plus: Time,
    orientation: Orientation,
    orientation_plus: Orientation,
}

impl<'k> SpeedSample<'k> {
    fn at(kernels: &'k KernelSet, jd: f64) -> Result<Self, EngineError> {
        let t = Time::from_ut1(jd);
        let t_plus = Time::from_ut1(t.ut1() + 1.0 / 24.0);

        // Both ends of the speed sample must be covered by the same kernel.
        let mut kernel = require_kernel(kernels, jd)?;
        let plus_kernel = kernels.for_jd(t_plus.ut1());
        if !plus_kernel.is_some_and(|k| std::ptr::eq(k, kernel)) {
            kernel = require_kernel(kernels, t_plus.ut1())?;
        }
        Ok(SpeedSample {
            kernel,
            orientation: Orientation::at(&t),
            orientation_plus: Orientation::at(&t_plus),
            t,
            t_plus,
        })
    }

    /// Ecliptic latitude, longitude (degrees, tropical), distance (au) and apparent speed
    /// in longitude (degrees per day) of the body with NAIF code `code`.
    fn observe(&self, code: i32) -> Result<(f64, f64, f64, f64), EngineError> {
        let (kernel, t, t_plus) = (self.kernel, &self.t, &self.t_plus);
        let a = observe(kernel, code, t)?;
        let app = apparent(kernel, &a, t)?;
        let a_plus = observe(kernel, code, t_plus)?;
        let app_plus = apparent(kernel, &a_plus, t_plus)?;
        let (lat, lon, dist) = ecliptic_latlon(&self.orientation, &app);
        let lon_plus = ecliptic_latlon(&self.orientation_plus, &app_plus).1;
        let d_lon = pyfloat::wrap180(lon_plus - lon);
        Ok((lat, lon, dist, d_lon * 24.0))
    }

    /// Longitude (degrees, tropical) and speed (degrees per day) of the true ascending
    /// node: the direction of ẑ × h, h being the Moon's geocentric orbital angular momentum
    /// in the true ecliptic and equinox of date.
    fn true_node(&self) -> Result<(f64, f64), EngineError> {
        let node_at = |t: &Time, orientation: &Orientation| -> Result<f64, EngineError> {
            let (moon, moon_velocity) = self.kernel.barycentric(301, t)?;
            let (earth, earth_velocity) = self.kernel.barycentric(EARTH, t)?;
            let r = mxv(
                &orientation.ecliptic,
                &[moon[0] - earth[0], moon[1] - earth[1], moon[2] - earth[2]],
            );
            let v = mxv(
                &orientation.ecliptic,
                &[
                    moon_velocity[0] - earth_velocity[0],
                    moon_velocity[1] - earth_velocity[1],
                    moon_velocity[2] - earth_velocity[2],
                ],
            );
            let h_x = r[1] * v[2] - r[2] * v[1];
            let h_y = r[2] * v[0] - r[0] * v[2];
            Ok(h_x.atan2(-h_y).to_degrees().rem_euclid(360.0))
        };
        let lon = node_at(&self.t, &self.orientation)?;
        let lon_plus = node_at(&self.t_plus, &self.orientation_plus)?;
        Ok((lon, pyfloat::wrap180(lon_plus - lon) * 24.0))
    }
}
