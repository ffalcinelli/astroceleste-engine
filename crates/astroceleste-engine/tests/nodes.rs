//! The true (osculating) lunar node, which the reference fixtures predate: checked against
//! Swiss Ephemeris 2.10 (`swe.calc_ut(jd, swe.TRUE_NODE, FLG_SWIEPH | FLG_SPEED)`).

mod common;

use astroceleste_engine::{calculate_chart, Chart, ChartRequest, UtcInstant};
use common::kernels;

/// (UTC, Swiss Ephemeris true node longitude, its speed in degrees per day)
const SWISS_TRUE_NODE: [(&str, f64, f64); 6] = [
    ("1870-03-21T00:00:00Z", 116.13754, -0.11037),
    ("1950-09-17T04:04:00Z", 358.37915, 0.00559),
    ("1990-05-15T14:30:00Z", 310.25571, -0.01980),
    ("2000-01-01T12:00:00Z", 123.95402, -0.05474),
    ("2024-04-08T18:00:00Z", 15.62684, -0.00079),
    ("2051-11-03T06:15:00Z", 203.56775, -0.01721),
];

fn chart(utc: &str, node_type: &str, zodiac_type: &str) -> Option<Chart> {
    let kernels = kernels()?;
    let mut req = ChartRequest::new(UtcInstant::parse(utc).unwrap(), 41.9028, 12.4964);
    req.node_type = node_type;
    req.zodiac_type = zodiac_type;
    req.ayanamsa = "lahiri";
    Some(calculate_chart(&kernels, &req).unwrap())
}

fn node(chart: &Chart, name: &str) -> (f64, f64, bool) {
    let p = chart.planets.iter().find(|p| p.name == name).unwrap();
    (p.ecliptic_longitude, p.speed, p.is_retrograde)
}

fn gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn true_node_matches_swiss_ephemeris() {
    for (utc, longitude, speed) in SWISS_TRUE_NODE {
        let Some(chart) = chart(utc, "true", "tropical") else {
            return;
        };
        assert_eq!(chart.node_type, "true");
        let (north, north_speed, retrograde) = node(&chart, "North Node");
        assert!(
            gap(north, longitude) < 2.0 / 3600.0,
            "{utc}: {north:.5} vs {longitude:.5}"
        );
        assert!(
            (north_speed - speed).abs() < 0.005,
            "{utc}: speed {north_speed:.5} vs {speed:.5}"
        );
        assert_eq!(retrograde, north_speed < 0.0, "{utc}");
        let (south, south_speed, _) = node(&chart, "South Node");
        assert!(gap(south, north + 180.0) < 1e-9, "{utc}");
        assert_eq!(south_speed, north_speed);
    }
}

#[test]
fn mean_node_is_the_default() {
    let Some(default) = chart("1990-05-15T14:30:00Z", "", "tropical") else {
        return;
    };
    let mean = chart("1990-05-15T14:30:00Z", "mean", "tropical").unwrap();
    assert_eq!(default.node_type, "mean");
    assert_eq!(node(&default, "North Node"), node(&mean, "North Node"));
    // About 1.08° from the true node that day.
    let true_node = chart("1990-05-15T14:30:00Z", "osculating", "tropical").unwrap();
    let apart = gap(
        node(&default, "North Node").0,
        node(&true_node, "North Node").0,
    );
    assert!(apart > 1.0 && apart < 1.2, "{apart}");
}

#[test]
fn sidereal_true_node_is_shifted_by_the_ayanamsa() {
    let Some(tropical) = chart("1950-09-17T04:04:00Z", "true", "tropical") else {
        return;
    };
    let sidereal = chart("1950-09-17T04:04:00Z", "true", "sidereal").unwrap();
    let shift = sidereal.ayanamsa_value.unwrap();
    let expected = (node(&tropical, "North Node").0 - shift).rem_euclid(360.0);
    assert!(gap(node(&sidereal, "North Node").0, expected) < 1e-5);
}
