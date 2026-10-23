//! The panchanga and the Vedic reading of real charts, against published calendar facts.

mod common;

use astroceleste_engine::{
    calculate_chart, jyotish, vedic_calendar, ChartRequest, JyotishOptions, UtcInstant,
    VedicCalendar,
};
use common::kernels;

const DELHI: (f64, f64) = (28.6139, 77.2090);

fn panchanga(utc: &str) -> Option<VedicCalendar> {
    let kernels = kernels()?;
    let instant = UtcInstant::parse(utc).unwrap();
    Some(
        vedic_calendar(&kernels, instant, DELHI.0, DELHI.1)
            .unwrap()
            .unwrap(),
    )
}

fn at(text: &str) -> UtcInstant {
    UtcInstant::parse(text).unwrap()
}

fn minutes_apart(a: &str, b: &str) -> f64 {
    (at(a).seconds_since(&at(b)) / 60.0).abs()
}

#[test]
fn ugadi_2024_begins_krodhi_in_chaitra() {
    // 9 April 2024, 11:30 IST: the first day of Chaitra, Shaka 1946 (Krodhi).
    let Some(p) = panchanga("2024-04-09T06:00:00Z") else {
        return;
    };
    assert_eq!(
        (p.tithi.number, p.tithi.code, p.paksha),
        (1, "pratipada", "shukla")
    );
    // The tithi began with the new moon of 8 April 2024, 18:21 UT (the total eclipse).
    assert!(
        minutes_apart(&p.tithi.start, "2024-04-08T18:21:00Z") < 2.0,
        "{}",
        p.tithi.start
    );
    assert_eq!(p.tithi.start, p.masa.start);
    assert_eq!(
        (p.masa.amanta, p.masa.purnimanta, p.masa.adhika),
        ("chaitra", "chaitra", false)
    );
    assert_eq!((p.samvatsara_number, p.samvatsara), (38, "krodhi"));
    assert_eq!((p.shaka_year, p.vikram_year), (1946, 2081));
    assert_eq!((p.vara.code, p.vara.lord), ("mangalavara", "Mars"));
    assert_eq!(p.nakshatra.code, "ashwini");
    assert_eq!((p.ritu, p.ayana), ("vasanta", "uttarayana"));
    // Sunrise in Delhi about 06:03 IST.
    assert!(
        minutes_apart(&p.sunrise, "2024-04-09T00:33:00Z") < 5.0,
        "{}",
        p.sunrise
    );
    for anga in [&p.tithi, &p.nakshatra, &p.yoga, &p.karana] {
        assert!(at(&anga.start) <= at("2024-04-09T06:00:00Z"), "{anga:?}");
        assert!(at("2024-04-09T06:00:00Z") < at(&anga.end), "{anga:?}");
    }
    // The year before: 21 March 2024 was Shobhakrit, Shaka 1945.
    let p = panchanga("2024-03-21T06:00:00Z").unwrap();
    assert_eq!(
        (p.samvatsara, p.shaka_year, p.masa.amanta),
        ("shobhakrit", 1945, "phalguna")
    );
}

#[test]
fn the_vedic_day_begins_at_sunrise() {
    // 05:30 IST on Tuesday 9 April 2024 is still Monday's Vedic day.
    let Some(p) = panchanga("2024-04-09T00:00:00Z") else {
        return;
    };
    assert_eq!(p.vara.code, "somavara");
    assert!(at(&p.sunrise) < at("2024-04-09T00:00:00Z"));
    assert!(at("2024-04-09T00:00:00Z") < at(&p.next_sunrise));
}

#[test]
fn adhika_shravana_2023() {
    // 2023 had an intercalary Shravana (18 July - 16 August), then the regular one.
    let Some(p) = panchanga("2023-08-01T06:00:00Z") else {
        return;
    };
    assert_eq!((p.masa.amanta, p.masa.adhika), ("shravana", true));
    let p = panchanga("2023-08-25T06:00:00Z").unwrap();
    assert_eq!((p.masa.amanta, p.masa.adhika), ("shravana", false));
    // Krishna paksha of a purnimanta month runs ahead of the amanta one.
    let p = panchanga("2023-09-05T06:00:00Z").unwrap();
    assert_eq!(
        (p.paksha, p.masa.amanta, p.masa.purnimanta),
        ("krishna", "shravana", "bhadrapada")
    );
}

#[test]
fn charts_carry_the_panchanga_on_request() {
    let Some(kernels) = kernels() else {
        return;
    };
    let mut req = ChartRequest::new(at("2024-04-09T06:00:00Z"), DELHI.0, DELHI.1);
    let plain = serde_json::to_value(calculate_chart(&kernels, &req).unwrap()).unwrap();
    assert!(plain.get("vedic_calendar").is_none());
    req.vedic_calendar = true;
    let chart = serde_json::to_value(calculate_chart(&kernels, &req).unwrap()).unwrap();
    assert_eq!(chart["vedic_calendar"]["tithi"]["code"], "pratipada");

    // The reading's Moon is in the panchanga's nakshatra, and its lord rules the dasha.
    let reading = jyotish(&chart, req.instant, &JyotishOptions::default()).unwrap();
    let moon = reading.grahas.iter().find(|g| g.name == "Moon").unwrap();
    assert_eq!(moon.nakshatra.code, "ashwini");
    assert_eq!(reading.vimshottari.birth_lord, moon.nakshatra.lord);
    assert_eq!(reading.vimshottari.periods[0].lord, "South Node");

    // Outside the kernel: no panchanga, no error.
    let old = vedic_calendar(&kernels, at("1700-01-01T00:00:00Z"), DELHI.0, DELHI.1).unwrap();
    assert!(old.is_none());
}

#[test]
fn a_reading_in_the_vargas() {
    // 17 September 1950, 04:04 UT, Vadnagar (23.78 N, 72.63 E), Lahiri: the Khagole fork's
    // reference chart.
    let Some(kernels) = kernels() else {
        return;
    };
    let mut req = ChartRequest::new(at("1950-09-17T04:04:00Z"), 23.783333, 72.633333);
    req.zodiac_type = "sidereal";
    req.ayanamsa = "lahiri";
    let chart = serde_json::to_value(calculate_chart(&kernels, &req).unwrap()).unwrap();
    let reading = jyotish(&chart, req.instant, &JyotishOptions::default()).unwrap();
    let d9 = reading.vargas.iter().find(|v| v.code == "D9").unwrap();
    let sign = |name: &str| d9.placements.iter().find(|p| p.name == name).unwrap().sign;
    assert_eq!(sign("Ascendant"), "Capricorn");
    assert_eq!(sign("Moon"), "Virgo");
    assert_eq!(sign("Sun"), "Capricorn");
    let lords: Vec<&str> = reading.vimshottari.periods.iter().map(|p| p.lord).collect();
    assert_eq!(lords[..3], ["Saturn", "Mercury", "South Node"]);
    // Saturn's balance ends in February 1963 (18 February with calendar years).
    assert!(reading.vimshottari.periods[0].end.starts_with("1963-02"));
}
