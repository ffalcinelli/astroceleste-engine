//! Planetary positions (`SkyfieldEngine.calculate_planets`): apparent geocentric
//! longitudes of date from the JPL kernels, the mean lunar nodes, Chiron and Black Moon
//! Lilith.

use crate::chiron::chiron;
use crate::ephemeris::observe::{apparent, ecliptic_latlon, observe};
use crate::ephemeris::{Kernel, KernelSet};
use crate::error::EngineError;
use crate::frames::Orientation;
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

pub struct Planets {
    pub bodies: Vec<BodyPosition>,
    /// Bodies that could not be computed, e.g. Chiron outside its table.
    pub unavailable: Vec<&'static str>,
}

/// All chart bodies at UT1 Julian date `jd`, shifted by `shift` degrees (the ayanamsa for
/// sidereal charts, 0 for tropical).
pub fn calculate_planets(kernels: &KernelSet, jd: f64, shift: f64) -> Result<Planets, EngineError> {
    let t = Time::from_ut1(jd);
    // One hour later, for the apparent speed and the retrograde flag.
    let t_plus = Time::from_ut1(t.ut1() + 1.0 / 24.0);

    // Both ends of the speed sample must be covered by the same kernel.
    let mut kernel = require_kernel(kernels, jd)?;
    let plus_kernel = kernels.for_jd(t_plus.ut1());
    if !plus_kernel.is_some_and(|k| std::ptr::eq(k, kernel)) {
        kernel = require_kernel(kernels, t_plus.ut1())?;
    }

    let orientation = Orientation::at(&t);
    let orientation_plus = Orientation::at(&t_plus);
    let mut bodies = Vec::with_capacity(14);
    let mut unavailable = Vec::new();

    for (name, code, symbol, can_retrograde) in TARGETS {
        let observed = (|| {
            let a = observe(kernel, code, &t)?;
            let app = apparent(kernel, &a, &t)?;
            let a_plus = observe(kernel, code, &t_plus)?;
            let app_plus = apparent(kernel, &a_plus, &t_plus)?;
            Ok::<_, EngineError>((
                ecliptic_latlon(&orientation, &app),
                ecliptic_latlon(&orientation_plus, &app_plus).1,
            ))
        })();
        let Ok(((lat, lon, dist), lon_plus)) = observed else {
            unavailable.push(name);
            continue;
        };
        let d_lon = pyfloat::rem(lon_plus - lon + 180.0, 360.0) - 180.0;
        let speed = d_lon * 24.0;
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

    // Mean lunar ascending node (IAU polynomial) and its opposite.
    let t_cent = (jd - 2_451_545.0) / 36525.0;
    let omega = pyfloat::rem(
        125.0445479 - 1934.1362891 * t_cent
            + 0.0020754 * t_cent.powf(2.0)
            + t_cent.powf(3.0) / 467440.0
            - t_cent.powf(4.0) / 60616000.0,
        360.0,
    );
    let mean_node = pyfloat::rem(omega - shift, 360.0);
    for (name, symbol, longitude) in [
        ("North Node", "☊", mean_node),
        ("South Node", "☋", pyfloat::rem(mean_node + 180.0, 360.0)),
    ] {
        bodies.push(BodyPosition {
            name,
            symbol,
            longitude,
            latitude: 0.0,
            distance: 0.00257,
            speed: -0.05295,
            is_retrograde: true,
        });
    }

    match chiron(jd, shift) {
        Some(c) => bodies.push(BodyPosition {
            name: "Chiron",
            symbol: "⚷",
            longitude: c.longitude,
            latitude: 0.0,
            distance: 0.0,
            speed: c.speed,
            is_retrograde: c.is_retrograde,
        }),
        None => unavailable.push("Chiron"),
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

    Ok(Planets {
        bodies,
        unavailable,
    })
}
