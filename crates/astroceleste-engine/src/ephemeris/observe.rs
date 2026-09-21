//! Geocentric apparent positions: light-time, gravitational deflection and aberration.
//! Ported from Skyfield 1.55 (`vectorlib`, `positionlib`, `relativity`), MIT, see NOTICE.

use crate::constants::{AU_M, C, C_AUDAY, GS, TAU};
use crate::frames::{mxv, Orientation, Vec3};
use crate::time::Time;

use super::kernels::Kernel;
use super::spk::SpkError;

pub const EARTH: i32 = 399;

fn length(v: &Vec3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn dot(a: &Vec3, b: &Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: &Vec3, b: &Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Reciprocal masses of the deflecting bodies (Skyfield `relativity.rmasses`).
fn reciprocal_mass(code: i32) -> f64 {
    match code {
        10 => 1.0,
        599 => 1047.3486,
        699 => 3497.898,
        _ => unreachable!("no mass for {code}"),
    }
}

/// Deflectors Skyfield's `apparent()` uses by default: Sun, Jupiter, Saturn.
const DEFLECTORS: [i32; 3] = [10, 599, 699];

/// A light-time corrected position of `target` seen from the geocenter.
#[derive(Debug, Clone)]
pub struct Astrometric {
    /// Target relative to the observer, au (ICRS).
    pub position: Vec3,
    pub light_time: f64,
    observer_position: Vec3,
    observer_velocity: Vec3,
}

/// Skyfield `earth.at(t).observe(target)`.
pub fn observe(kernel: &Kernel, target: i32, t: &Time) -> Result<Astrometric, SpkError> {
    let (observer_position, observer_velocity) = kernel.barycentric(EARTH, t)?;
    let (mut tposition, _) = kernel.barycentric(target, t)?;
    let mut distance = length(&sub(&tposition, &observer_position));
    let mut light_time0 = 0.0;
    let mut converged = None;
    for _ in 0..10 {
        let light_time = distance / C_AUDAY;
        if (light_time - light_time0).abs() < 1e-12 {
            converged = Some(light_time);
            break;
        }
        let t2 = Time::from_tdb(t.whole, t.tdb_fraction - light_time);
        tposition = kernel.barycentric(target, &t2)?.0;
        distance = length(&sub(&tposition, &observer_position));
        light_time0 = light_time;
    }
    let light_time =
        converged.ok_or_else(|| SpkError::Format("light-travel time failed to converge".into()))?;
    Ok(Astrometric {
        position: sub(&tposition, &observer_position),
        light_time,
        observer_position,
        observer_velocity,
    })
}

/// Skyfield `Astrometric.apparent()` for a geocentric observer: deflection by the Sun,
/// Jupiter and Saturn, then aberration. Returns the apparent ICRS vector in au.
pub fn apparent(kernel: &Kernel, astrometric: &Astrometric, t: &Time) -> Result<Vec3, SpkError> {
    let mut target = astrometric.position;
    let observer = astrometric.observer_position;
    let tlt = length(&target) / C_AUDAY;

    for code in DEFLECTORS {
        let rmass = reciprocal_mass(code);
        // Kernels such as DE440s carry only planetary barycenters.
        let body = if code % 100 == 99 && !kernel.contains(code) {
            code / 100
        } else {
            code
        };
        let pe = deflector_position(kernel, body, t, &observer, &target, tlt)?;
        let d = deflection(&target, &pe, rmass);
        for k in 0..3 {
            target[k] += d[k];
        }
    }
    add_aberration(
        &mut target,
        &astrometric.observer_velocity,
        astrometric.light_time,
    );
    Ok(target)
}

/// Observer position relative to the deflector when the light passed closest to it.
fn deflector_position(
    kernel: &Kernel,
    body: i32,
    t: &Time,
    observer: &Vec3,
    position: &Vec3,
    tlt: f64,
) -> Result<Vec3, SpkError> {
    let bposition = kernel.barycentric(body, t)?.0;
    let gpv = sub(&bposition, observer);
    let dis = length(position);
    let u1 = position.map(|x| x / (dis + f64::MIN_POSITIVE));
    let dlt = dot(&u1, &gpv) / C_AUDAY;
    let tclose = t.minus_days(dlt.clamp(0.0, tlt));
    let bposition = kernel.barycentric(body, &tclose)?.0;
    Ok(sub(observer, &bposition))
}

fn deflection(position: &Vec3, pe: &Vec3, rmass: f64) -> Vec3 {
    let pq = [
        position[0] + pe[0],
        position[1] + pe[1],
        position[2] + pe[2],
    ];
    let pmag = length(position);
    let qmag = length(&pq);
    let emag = length(pe);
    let unit = |v: &Vec3, m: f64| {
        let m = if m != 0.0 { m } else { 1.0 };
        v.map(|x| x / m)
    };
    let phat = unit(position, pmag);
    let qhat = unit(&pq, qmag);
    let ehat = unit(pe, emag);
    let pdotq = dot(&phat, &qhat);
    let qdote = dot(&qhat, &ehat);
    let edotp = dot(&ehat, &phat);
    if edotp.abs() > 0.99999999999 {
        return [0.0; 3];
    }
    let fac1 = 2.0 * GS / (C * C * emag * AU_M * rmass);
    let fac2 = 1.0 + qdote;
    [0, 1, 2].map(|k| fac1 * (pdotq * ehat[k] - edotp * qhat[k]) / fac2 * pmag)
}

fn add_aberration(position: &mut Vec3, velocity: &Vec3, light_time: f64) {
    let p1mag = light_time * C_AUDAY;
    let vemag = length(velocity);
    let beta = vemag / C_AUDAY;
    let cosd = dot(position, velocity) / (p1mag * vemag + f64::MIN_POSITIVE);
    let gammai = (1.0 - beta * beta).sqrt();
    let p = beta * cosd;
    let q = (1.0 + p / (1.0 + gammai)) * light_time;
    let r = 1.0 + p;
    for k in 0..3 {
        position[k] *= gammai;
        position[k] += q * velocity[k];
        position[k] /= r;
    }
}

/// Latitude, longitude (degrees) and distance (au) in the true ecliptic of date.
pub fn ecliptic_latlon(orientation: &Orientation, apparent: &Vec3) -> (f64, f64, f64) {
    let v = mxv(&orientation.ecliptic, apparent);
    let r = length(&v);
    let lat = v[2].atan2(v[0].hypot(v[1]));
    let lon = v[1].atan2(v[0]).rem_euclid(TAU);
    (lat * 360.0 / TAU, lon * 360.0 / TAU, r)
}
