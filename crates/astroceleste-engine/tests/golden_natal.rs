//! Natal planets and houses against the reference implementation's golden charts
//! (tests/fixtures/natal.json). Needs the full de440s kernel; skipped without it.

use std::path::PathBuf;

use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
use astroceleste_engine::houses::{calculate_houses, HouseSystem};
use astroceleste_engine::planets::calculate_planets;
use astroceleste_engine::time::julian_day;
use astroceleste_engine::zodiac::{ayanamsa_info, determine_house, longitude_to_zodiac};
use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn kernels() -> Option<KernelSet> {
    let path = root().join("kernels/de440s.bsp");
    if !path.exists() {
        eprintln!("skipping: {} not found", path.display());
        return None;
    }
    let mut set = KernelSet::new();
    set.push(Kernel::new("de440s.bsp", Spk::open(path).unwrap()).unwrap());
    Some(set)
}

/// "YYYY-MM-DDTHH:MM:SS" (UTC) → Julian day, as the reference computes it.
fn jd_of(utc: &str) -> f64 {
    let (date, time) = utc.split_once('T').unwrap();
    let d: Vec<i64> = date.split('-').map(|p| p.parse().unwrap()).collect();
    let t: Vec<f64> = time.split(':').map(|p| p.parse().unwrap()).collect();
    let hour = t[0] + t[1] / 60.0 + t[2] / 3600.0;
    julian_day(d[0] as i32, d[1] as u32, d[2] as u32, hour)
}

fn angle_diff(a: f64, b: f64) -> f64 {
    ((a - b + 180.0).rem_euclid(360.0) - 180.0).abs()
}

#[test]
fn natal_planets_and_houses_match() {
    let Some(kernels) = kernels() else { return };
    let text = std::fs::read_to_string(root().join("tests/fixtures/natal.json")).unwrap();
    let cases: Vec<Value> = serde_json::from_str(&text).unwrap();
    let (mut worst_lon, mut worst_speed, mut worst_cusp) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut failures = Vec::new();

    for case in &cases {
        let input = &case["input"];
        let want = &case["output"];
        let id = input["id"].as_str().unwrap();
        let jd = jd_of(input["utc"].as_str().unwrap());
        let (lat, lon) = (
            input["lat"].as_f64().unwrap(),
            input["lon"].as_f64().unwrap(),
        );
        let sidereal = input["zodiac_type"] == "sidereal";
        let shift = if sidereal {
            let info = ayanamsa_info(jd, input["ayanamsa"].as_str().unwrap());
            if Some(info.value) != want["ayanamsa_value"].as_f64() {
                failures.push(format!(
                    "{id}: ayanamsa {} vs {}",
                    info.value, want["ayanamsa_value"]
                ));
            }
            if info.formatted != want["ayanamsa_formatted"].as_str().unwrap() {
                failures.push(format!("{id}: ayanamsa_formatted {}", info.formatted));
            }
            info.value
        } else {
            0.0
        };

        let system = HouseSystem::from_code(input["house_system"].as_str().unwrap());
        let houses = calculate_houses(jd, lat, lon, system, shift);
        for (i, h) in want["houses"].as_array().unwrap().iter().enumerate() {
            let expected = h["ecliptic_longitude"].as_f64().unwrap();
            worst_cusp = worst_cusp.max(angle_diff(houses.cusps[i], expected));
            let z = longitude_to_zodiac(houses.cusps[i]);
            if (z.sign.name, z.degree, z.minute)
                != (
                    h["sign"].as_str().unwrap(),
                    h["degree"].as_i64().unwrap(),
                    h["minute"].as_i64().unwrap(),
                )
            {
                failures.push(format!("{id}: cusp {} zodiac {:?}", i + 1, z));
            }
        }

        let planets = calculate_planets(&kernels, jd, shift).unwrap();
        let unavailable: Vec<&str> = want["unavailable_bodies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let mut got_unavailable = planets.unavailable.clone();
        got_unavailable.sort();
        if got_unavailable != unavailable {
            failures.push(format!(
                "{id}: unavailable {got_unavailable:?} vs {unavailable:?}"
            ));
        }

        let mut bodies: Vec<(&str, f64, f64, bool)> = planets
            .bodies
            .iter()
            .map(|b| (b.name, b.longitude, b.speed, b.is_retrograde))
            .collect();
        bodies.push(("Ascendant", houses.ascendant, 0.0, false));
        bodies.push(("Midheaven", houses.midheaven, 0.0, false));
        let want_planets = want["planets"].as_array().unwrap();
        if want_planets.len() != bodies.len() {
            failures.push(format!(
                "{id}: {} bodies vs {}",
                bodies.len(),
                want_planets.len()
            ));
            continue;
        }
        for (p, (name, lon, speed, retro)) in want_planets.iter().zip(&bodies) {
            if p["name"] != *name {
                failures.push(format!("{id}: body order {name} vs {}", p["name"]));
                continue;
            }
            worst_lon = worst_lon.max(angle_diff(*lon, p["ecliptic_longitude"].as_f64().unwrap()));
            worst_speed = worst_speed.max((speed - p["speed"].as_f64().unwrap()).abs());
            if p["is_retrograde"].as_bool().unwrap() != *retro {
                failures.push(format!("{id}: {name} retrograde"));
            }
            let house = determine_house(*lon, &houses.cusps);
            if p["house"].as_i64().unwrap() != house as i64 {
                failures.push(format!("{id}: {name} house {house} vs {}", p["house"]));
            }
            let z = longitude_to_zodiac(*lon);
            if (z.sign.name, z.degree, z.minute)
                != (
                    p["sign"].as_str().unwrap(),
                    p["degree"].as_i64().unwrap(),
                    p["minute"].as_i64().unwrap(),
                )
            {
                failures.push(format!("{id}: {name} zodiac {z:?}"));
            }
        }
    }

    println!("worst longitude {worst_lon:e}°, speed {worst_speed:e}°/day, cusp {worst_cusp:e}°");
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(worst_lon < 1e-9, "longitude drift {worst_lon}");
    assert!(worst_speed < 1e-7, "speed drift {worst_speed}");
    assert!(worst_cusp < 1e-9, "cusp drift {worst_cusp}");
}
