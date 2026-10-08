//! The Chinese calendar and Ba Zi, checked against published dates (Hong Kong
//! Observatory, Purple Mountain Observatory almanacs) and well-known charts.

mod common;

use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
use astroceleste_engine::{
    bazi, calculate_chart, chinese_calendar, BaziOptions, ChartRequest, ChineseCalendar, UtcInstant,
};
use common::{kernels, root};

fn at(utc: &str) -> UtcInstant {
    UtcInstant::parse(utc).unwrap()
}

fn calendar(kernels: &KernelSet, utc: &str) -> ChineseCalendar {
    chinese_calendar(kernels, at(utc)).unwrap().unwrap()
}

/// Seconds between two ISO 8601 instants.
fn seconds_between(a: &str, b: &str) -> f64 {
    at(a).seconds_since(&at(b)).abs()
}

#[test]
fn chinese_new_year() {
    let Some(kernels) = kernels() else { return };
    for (year, date) in [
        (1900, "1900-01-31"),
        (1912, "1912-02-18"),
        (1929, "1929-02-10"),
        (1949, "1949-01-29"),
        (1950, "1950-02-17"),
        (1984, "1984-02-02"),
        (1990, "1990-01-27"),
        (2000, "2000-02-05"),
        (2001, "2001-01-24"),
        (2010, "2010-02-14"),
        (2017, "2017-01-28"),
        (2020, "2020-01-25"),
        (2023, "2023-01-22"),
        (2024, "2024-02-10"),
        (2025, "2025-01-29"),
        (2026, "2026-02-17"),
        (2030, "2030-02-03"),
        (2033, "2033-01-31"),
        (2050, "2050-01-23"),
        (2100, "2100-02-09"),
    ] {
        // Noon in China on New Year's day.
        let cal = calendar(&kernels, &format!("{date}T04:00:00Z"));
        let first = cal
            .lunar_months
            .iter()
            .find(|m| m.month == 1 && !m.leap)
            .unwrap_or_else(|| panic!("{year}: no month 1 in {:?}", cal.lunar_months));
        assert_eq!((first.year, first.start.as_str()), (year, date), "{year}");
    }
}

#[test]
fn leap_months() {
    let Some(kernels) = kernels() else { return };
    for (year, month, start) in [
        (1984, 10, "1984-11-23"),
        (2001, 4, "2001-05-23"),
        (2004, 2, "2004-03-21"),
        (2006, 7, "2006-08-24"),
        (2009, 5, "2009-06-23"),
        (2012, 4, "2012-05-21"),
        (2014, 9, "2014-10-24"),
        (2017, 6, "2017-07-23"),
        (2020, 4, "2020-05-23"),
        (2023, 2, "2023-03-22"),
        (2025, 6, "2025-07-25"),
        // The "2033 problem": a month without a major term comes earlier in the year, but
        // the leap month is the eleventh.
        (2033, 11, "2033-12-22"),
    ] {
        let cal = calendar(&kernels, &format!("{start}T12:00:00Z"));
        let leap = cal
            .lunar_months
            .iter()
            .find(|m| m.leap)
            .unwrap_or_else(|| panic!("{year}: no leap month in {:?}", cal.lunar_months));
        assert_eq!(
            (leap.year, leap.month, leap.start.as_str()),
            (year, month, start),
            "{year}"
        );
    }
}

#[test]
fn no_month_is_leap_in_a_twelve_month_year() {
    let Some(kernels) = kernels() else { return };
    // 2033's seventh month has no major term, but its year has twelve months.
    let cal = calendar(&kernels, "2033-08-30T12:00:00Z");
    assert!(
        cal.lunar_months.iter().all(|m| !m.leap),
        "{:?}",
        cal.lunar_months
    );
}

#[test]
fn solar_terms_match_published_times() {
    let Some(kernels) = kernels() else { return };
    let cal = calendar(&kernels, "2000-12-25T00:00:00Z");
    let term = |name: &str| {
        cal.solar_terms
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("no {name}"))
    };
    // US Naval Observatory: equinox and solstices of 2000.
    for (name, utc) in [
        ("chunfen", "2000-03-20T07:35:00Z"),
        ("xiazhi", "2000-06-21T01:48:00Z"),
        ("qiufen", "2000-09-22T17:27:00Z"),
        ("dongzhi", "2000-12-21T13:37:00Z"),
    ] {
        let t = term(name);
        assert!(seconds_between(&t.utc, utc) < 60.0, "{name}: {}", t.utc);
    }
    assert_eq!(cal.solar_terms[0].name, "lichun");
    let last = cal.solar_terms.last().unwrap();
    assert_eq!((last.name.as_str(), last.jie), ("xiaohan", true));
    assert!(cal.solar_terms.windows(2).all(|w| w[0].utc < w[1].utc));

    // 立春 2024: 16:27 Beijing time.
    let cal = calendar(&kernels, "2024-03-01T00:00:00Z");
    assert!(seconds_between(&cal.solar_terms[0].utc, "2024-02-04T08:27:00Z") < 60.0);
}

#[test]
fn equation_of_time() {
    let Some(kernels) = kernels() else { return };
    for (utc, minutes) in [
        ("2000-11-03T12:00:00Z", 16.4),
        ("2000-02-11T12:00:00Z", -14.2),
        ("2000-04-15T12:00:00Z", 0.0),
    ] {
        let eot = calendar(&kernels, utc).equation_of_time;
        assert!((eot - minutes).abs() < 0.3, "{utc}: {eot}");
    }
}

#[test]
fn outside_the_kernels_there_is_no_calendar() {
    // The committed excerpt covers 2000 only; the calendar needs the year before.
    let mut kernels = KernelSet::new();
    let path = root().join("tests/data/de440s_2000.bsp");
    kernels.push(Kernel::new("excerpt", Spk::open(path).unwrap()).unwrap());
    assert_eq!(
        chinese_calendar(&kernels, at("2000-06-01T00:00:00Z")).unwrap(),
        None
    );
}

#[test]
fn the_chart_carries_the_calendar_only_on_request() {
    let Some(kernels) = kernels() else { return };
    let mut req = ChartRequest::new(at("1987-05-17T14:30:00Z"), 41.9, 12.5);
    assert!(calculate_chart(&kernels, &req)
        .unwrap()
        .chinese_calendar
        .is_none());
    req.chinese_calendar = true;
    let chart = calculate_chart(&kernels, &req).unwrap();
    assert_eq!(
        chart.chinese_calendar,
        chinese_calendar(&kernels, req.instant).unwrap()
    );
}

/// "stem branch" of the four pillars, year first.
fn pillars(
    kernels: &KernelSet,
    utc: &str,
    longitude: f64,
    offset: f64,
    options: &BaziOptions,
) -> [String; 4] {
    let cal = calendar(kernels, utc);
    let b = bazi(&cal, at(utc), longitude, offset, options).unwrap();
    [&b.year, &b.month, &b.day, &b.hour].map(|p| format!("{} {}", p.stem, p.branch))
}

#[test]
fn published_charts() {
    let Some(kernels) = kernels() else { return };
    // Bruce Lee, San Francisco, 1940-11-27 07:12 PST: 庚辰 丁亥 甲戌 戊辰.
    assert_eq!(
        pillars(
            &kernels,
            "1940-11-27T15:12:00Z",
            -122.42,
            -480.0,
            &BaziOptions::default()
        ),
        ["geng chen", "ding hai", "jia xu", "wu chen"]
    );
    // Mao Zedong, Shaoshan, 1893-12-26, 辰 hour: 癸巳 甲子 丁酉 甲辰.
    let civil = BaziOptions {
        solar_time: false,
        ..BaziOptions::default()
    };
    assert_eq!(
        pillars(&kernels, "1893-12-26T00:00:00Z", 112.53, 480.0, &civil),
        ["gui si", "jia zi", "ding you", "jia chen"]
    );
}

#[test]
fn day_pillar_anchors() {
    let Some(kernels) = kernels() else { return };
    let civil = BaziOptions {
        solar_time: false,
        ..BaziOptions::default()
    };
    // Noon in Beijing.
    for (date, day) in [("1949-10-01", "jia zi"), ("2000-01-01", "wu wu")] {
        let p = pillars(&kernels, &format!("{date}T04:00:00Z"), 116.4, 480.0, &civil);
        assert_eq!(p[2], day, "{date}");
    }
}

#[test]
fn the_year_and_month_change_at_the_jie_term() {
    let Some(kernels) = kernels() else { return };
    let lichun = calendar(&kernels, "2024-03-01T00:00:00Z").solar_terms[0]
        .utc
        .clone();
    let lichun = at(&lichun);
    let options = BaziOptions::default();
    let before = lichun.add_micros(-60_000_000).isoformat();
    let after = lichun.add_micros(60_000_000).isoformat();
    let p = pillars(&kernels, &before, 116.4, 480.0, &options);
    assert_eq!(p[..2], ["gui mao", "yi chou"]);
    let p = pillars(&kernels, &after, 116.4, 480.0, &options);
    assert_eq!(p[..2], ["jia chen", "bing yin"]);
}

#[test]
fn zi_hour_conventions() {
    let Some(kernels) = kernels() else { return };
    // 23:30 civil time in Beijing on 2000-01-01 (a 戊午 day).
    let utc = "2000-01-01T15:30:00Z";
    let next_day = BaziOptions {
        solar_time: false,
        ..BaziOptions::default()
    };
    let split = BaziOptions {
        zi_hour: "split",
        ..next_day.clone()
    };
    let a = pillars(&kernels, utc, 116.4, 480.0, &next_day);
    let b = pillars(&kernels, utc, 116.4, 480.0, &split);
    assert_eq!(a[2], "ji wei");
    assert_eq!(b[2], "wu wu");
    // Either way the hour is the 子 hour of the 己 day: 甲子.
    assert_eq!(a[3], "jia zi");
    assert_eq!(b[3], "jia zi");
}

#[test]
fn solar_time_moves_the_hour() {
    let Some(kernels) = kernels() else { return };
    // 12:50 civil time in Ürümqi (87.6° E) on China Standard Time is 10:43 solar.
    let utc = "2010-06-01T04:50:00Z";
    let cal = calendar(&kernels, utc);
    let solar = bazi(&cal, at(utc), 87.6, 480.0, &BaziOptions::default()).unwrap();
    assert_eq!(solar.civil_time, "2010-06-01T12:50:00");
    assert!(
        solar.solar_time.starts_with("2010-06-01T10:4"),
        "{}",
        solar.solar_time
    );
    assert_eq!(solar.hour.branch, "si");
    let civil = BaziOptions {
        solar_time: false,
        ..BaziOptions::default()
    };
    let civil = bazi(&cal, at(utc), 87.6, 480.0, &civil).unwrap();
    assert_eq!(civil.hour.branch, "wu");
}

#[test]
fn luck_pillars() {
    let Some(kernels) = kernels() else { return };
    let utc = "1940-11-27T15:12:00Z";
    let cal = calendar(&kernels, utc);
    let birth = at(utc);
    assert!(bazi(&cal, birth, -122.42, -480.0, &BaziOptions::default())
        .unwrap()
        .luck
        .is_none());

    // A 庚 (yang) year: forward for a man, from the month pillar 丁亥.
    let man = BaziOptions {
        sex: Some("male"),
        ..BaziOptions::default()
    };
    let b = bazi(&cal, birth, -122.42, -480.0, &man).unwrap();
    let luck = b.luck.unwrap();
    assert_eq!(luck.direction, "forward");
    let to_next = at(&b.next_term.utc).seconds_since(&birth) / 86_400.0;
    assert!((luck.start_age - to_next / 3.0).abs() < 1e-9);
    let names: Vec<String> = luck
        .pillars
        .iter()
        .take(3)
        .map(|p| format!("{} {}", p.pillar.stem, p.pillar.branch))
        .collect();
    assert_eq!(names, ["wu zi", "ji chou", "geng yin"]);
    assert_eq!(luck.pillars.len(), 10);
    assert_eq!(luck.pillars[0].end, luck.pillars[1].start);

    let woman = BaziOptions {
        sex: Some("female"),
        ..BaziOptions::default()
    };
    let luck = bazi(&cal, birth, -122.42, -480.0, &woman)
        .unwrap()
        .luck
        .unwrap();
    assert_eq!(luck.direction, "backward");
    assert_eq!(
        (luck.pillars[0].pillar.stem, luck.pillars[0].pillar.branch),
        ("bing", "xu")
    );
}

#[test]
fn ten_gods_hidden_stems_and_balance() {
    let Some(kernels) = kernels() else { return };
    let utc = "1940-11-27T15:12:00Z";
    let cal = calendar(&kernels, utc);
    let b = bazi(&cal, at(utc), -122.42, -480.0, &BaziOptions::default()).unwrap();
    // Day Master 甲 wood.
    assert_eq!((b.day_master, b.day_master_element), ("jia", "wood"));
    assert_eq!(b.day.ten_god, None);
    assert_eq!(b.year.ten_god, Some("seven_killings")); // 庚 yang metal
    assert_eq!(b.month.ten_god, Some("hurting_officer")); // 丁 yin fire
    assert_eq!(b.hour.ten_god, Some("indirect_wealth")); // 戊 yang earth
    let hidden: Vec<_> = b.month.hidden_stems.iter().map(|h| h.stem).collect();
    assert_eq!(hidden, ["ren", "jia"]); // 亥
    assert_eq!(b.month.hidden_stems[0].ten_god, "indirect_resource");
    assert_eq!(b.year.nayin, "wax_metal"); // 庚辰 白蠟金
    assert_eq!(b.day.life_stage, "nurture"); // 甲 in 戌 (養)
    assert_eq!(b.year.life_stage, "decline"); // 甲 in 辰 (衰)
    let e = &b.elements;
    let total = e.wood + e.fire + e.earth + e.metal + e.water;
    assert!((total - 8.0).abs() < 1e-9, "{e:?}");
}

#[test]
fn lunar_date_and_popular_animal() {
    let Some(kernels) = kernels() else { return };
    // 2020-01-30, after the New Year (Rat) but before 立春 (still a Pig year in Ba Zi).
    let utc = "2020-01-30T04:00:00Z";
    let cal = calendar(&kernels, utc);
    let b = bazi(&cal, at(utc), 116.4, 480.0, &BaziOptions::default()).unwrap();
    assert_eq!(b.year.animal, "pig");
    let lunar = b.lunar_date.unwrap();
    assert_eq!(
        (lunar.year, lunar.month, lunar.day, lunar.leap, lunar.animal),
        (2020, 1, 6, false, "rat")
    );
}

#[test]
fn bad_options_are_rejected() {
    let Some(kernels) = kernels() else { return };
    let utc = "2020-01-30T04:00:00Z";
    let cal = calendar(&kernels, utc);
    for options in [
        BaziOptions {
            zi_hour: "noon",
            ..BaziOptions::default()
        },
        BaziOptions {
            sex: Some("other"),
            ..BaziOptions::default()
        },
    ] {
        assert!(bazi(&cal, at(utc), 116.4, 480.0, &options).is_err());
    }
    // A calendar cast for another year does not cover the birth.
    assert!(bazi(
        &cal,
        at("1990-01-01T00:00:00Z"),
        116.4,
        480.0,
        &BaziOptions::default()
    )
    .is_err());
}
