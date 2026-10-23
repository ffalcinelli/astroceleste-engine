//! The minor bodies (main asteroids and dwarf planets) in complete charts, which the
//! reference fixtures predate. Their tables are checked against JPL Horizons in
//! `src/minor_bodies.rs`.

mod common;

use astroceleste_engine::{
    calculate_chart, calculate_transit_chart, is_minor_body, ChartRequest, UtcInstant, MINOR_BODIES,
};
use common::kernels;
use serde_json::Value;

fn chart_json(utc: &str) -> Option<Value> {
    let kernels = kernels()?;
    let req = ChartRequest::new(UtcInstant::parse(utc).unwrap(), 41.9028, 12.4964);
    Some(serde_json::to_value(calculate_chart(&kernels, &req).unwrap()).unwrap())
}

fn planet<'a>(chart: &'a Value, name: &str) -> &'a Value {
    chart["planets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("{name} missing"))
}

#[test]
fn charts_carry_every_minor_body_before_the_angles() {
    let Some(chart) = chart_json("1987-05-17T14:30:00Z") else {
        return;
    };
    let names: Vec<&str> = chart["planets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    let lilith = names.iter().position(|n| *n == "Lilith").unwrap();
    assert_eq!(
        names[lilith + 1..lilith + 1 + MINOR_BODIES.len()],
        MINOR_BODIES
    );
    assert_eq!(names[names.len() - 2..], ["Ascendant", "Midheaven"]);
    assert_eq!(chart["unavailable_bodies"], Value::Array(vec![]));

    for name in MINOR_BODIES {
        let p = planet(&chart, name);
        let house = p["house"].as_u64().unwrap();
        assert!((1..=12).contains(&house), "{name}");
        assert!(p.get("condition").is_none(), "{name} has a condition");
        assert_eq!(p["is_retrograde"], p["speed"].as_f64().unwrap() < 0.0);
    }
    assert_eq!(planet(&chart, "Ceres")["symbol"], "⚳");
    assert_eq!(planet(&chart, "Vesta")["symbol"], "⚶");
    assert_eq!(planet(&chart, "Eris")["symbol"], "");
    assert_eq!(chart["orb_settings"]["planet_orbs"]["Ceres"], 3.0);
}

#[test]
fn positions_follow_the_chart_instant_in_ut() {
    // 2000-01-01 12:00 TT (ΔT 63.8 s); Horizons gives Ceres at 184.4528993°.
    let Some(chart) = chart_json("2000-01-01T11:58:56.2Z") else {
        return;
    };
    let lon = planet(&chart, "Ceres")["ecliptic_longitude"]
        .as_f64()
        .unwrap();
    assert!((lon - 184.4528993).abs() * 3600.0 < 3.0, "{lon}");
}

#[test]
fn minor_bodies_aspect_but_stay_out_of_stars_and_temperament() {
    let Some(kernels) = kernels() else { return };
    let mut checked_house_1 = false;
    let mut aspected = false;
    for hour in 0..24 {
        let utc = format!("1987-05-17T{hour:02}:30:00Z");
        let req = ChartRequest::new(UtcInstant::parse(&utc).unwrap(), 41.9028, 12.4964);
        let chart = serde_json::to_value(calculate_chart(&kernels, &req).unwrap()).unwrap();
        for a in chart["aspects"].as_array().unwrap() {
            let (b1, b2) = (a["body1"].as_str().unwrap(), a["body2"].as_str().unwrap());
            if is_minor_body(b1) || is_minor_body(b2) {
                aspected = true;
                // Fixed-star conjunctions (no `is_major`) are judged against the
                // traditional points only.
                assert!(a.get("is_major").is_some(), "{a}");
            }
        }
        let in_house_1 = MINOR_BODIES
            .iter()
            .any(|name| planet(&chart, name)["house"] == 1);
        if in_house_1 {
            checked_house_1 = true;
            for f in chart["temperament"]["breakdown"].as_array().unwrap() {
                let details = f["details"].as_str().unwrap();
                assert!(
                    !MINOR_BODIES.iter().any(|n| details.starts_with(n)),
                    "{details}"
                );
            }
        }
    }
    assert!(checked_house_1 && aspected);
}

#[test]
fn transits_cross_aspect_minor_bodies() {
    let Some(kernels) = kernels() else { return };
    let Some(natal) = chart_json("1987-05-17T14:30:00Z") else {
        return;
    };
    let req = ChartRequest::new(
        UtcInstant::parse("2026-10-23T12:00:00Z").unwrap(),
        41.9028,
        12.4964,
    );
    let transit = calculate_transit_chart(&kernels, &natal["planets"], &req).unwrap();
    let transit = serde_json::to_value(transit).unwrap();
    let placed = transit["transit_planets"].as_array().unwrap();
    for name in MINOR_BODIES {
        assert!(placed.iter().any(|p| p["name"] == name), "{name} missing");
    }
    let cross = transit["cross_aspects"].as_array().unwrap();
    assert!(cross.iter().any(|a| a
        .as_object()
        .unwrap()
        .values()
        .any(|v| { v.as_str().is_some_and(is_minor_body) })));
}
