//! The Vimshottari dasha: 120 years of planetary periods, starting with the lord of the
//! Moon's nakshatra at birth, each period divided among the nine lords in the same order
//! and in proportion to their years.

use serde::Serialize;

use super::{nakshatra, DASHA_LORDS};
use crate::error::EngineError;
use crate::instant::UtcInstant;

/// The whole cycle, years.
const CYCLE_YEARS: f64 = 120.0;
/// Levels below the mahadashas: antardasha, pratyantardasha, sookshma, prana.
pub const MAX_DASHA_LEVELS: usize = 5;
/// Most levels returned in one tree below `path`.
pub const MAX_DASHA_DEPTH: usize = 3;

/// The length of a dasha year: (code, days).
const YEARS: [(&str, f64); 4] = [
    ("julian", 365.25),
    ("gregorian", 365.2425),
    ("sidereal", 365.256_363),
    ("savana", 360.0),
];

/// A dasha period and, to the depth asked, its subperiods.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashaPeriod {
    /// The ruling body (engine names: "North Node" is Rahu, "South Node" Ketu).
    pub lord: &'static str,
    /// ISO 8601 UTC, to the second (the first mahadasha starts before the birth).
    pub start: String,
    /// When it ends, ISO 8601 UTC to the second.
    pub end: String,
    /// Its subperiods, to the depth asked (left out when empty).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub periods: Vec<DashaPeriod>,
}

/// The Vimshottari dasha of a birth, in a [`Jyotish`](crate::Jyotish) reading.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Vimshottari {
    /// Dasha year: "julian" (365.25 days), "gregorian", "sidereal" or "savana" (360).
    pub year: &'static str,
    /// The lord of the Moon's nakshatra, whose mahadasha runs at birth.
    pub birth_lord: &'static str,
    /// Years of that mahadasha left at birth.
    pub balance_years: f64,
    /// The lords leading to `periods` (empty: `periods` are the mahadashas).
    pub path: Vec<&'static str>,
    /// The periods under `path`, `depth` levels deep.
    pub periods: Vec<DashaPeriod>,
    /// The periods running at the date asked, mahadasha first, down to the prana dasha;
    /// empty without a date or outside the 120 years.
    pub current: Vec<DashaPeriod>,
}

/// One period by its offsets from the birth, in days.
#[derive(Clone, Copy)]
struct Span {
    lord: usize,
    start: f64,
    days: f64,
}

impl Span {
    fn children(self) -> impl Iterator<Item = Span> {
        let mut start = self.start;
        (0..9).map(move |k| {
            let lord = (self.lord + k) % 9;
            let days = self.days * DASHA_LORDS[lord].1 / CYCLE_YEARS;
            let child = Span { lord, start, days };
            start += days;
            child
        })
    }
}

/// The Vimshottari dasha from `birth` and the Moon's sidereal longitude: the periods under
/// `path` (lords from the mahadasha down) to `depth` levels, and those running at `date`.
pub fn vimshottari(
    birth: UtcInstant,
    moon_longitude: f64,
    year: &str,
    path: &[String],
    depth: usize,
    date: Option<UtcInstant>,
) -> Result<Vimshottari, EngineError> {
    let (year, year_days) = YEARS
        .iter()
        .copied()
        .find(|(code, _)| code.eq_ignore_ascii_case(year.trim()))
        .ok_or_else(|| EngineError::InvalidInput(format!("unknown dasha year: {year}")))?;
    if !(1..=MAX_DASHA_DEPTH).contains(&depth) || path.len() + depth > MAX_DASHA_LEVELS {
        return Err(EngineError::InvalidInput(format!(
            "dasha depth must be 1-{MAX_DASHA_DEPTH}, with at most {MAX_DASHA_LEVELS} levels in all"
        )));
    }
    if !moon_longitude.is_finite() {
        return Err(EngineError::InvalidInput(format!(
            "moon longitude must be finite, got {moon_longitude}"
        )));
    }

    let moon = nakshatra(moon_longitude);
    let first = (usize::from(moon.number) - 1) % 9;
    let first_days = DASHA_LORDS[first].1 * year_days;
    let cycle = Span {
        lord: first,
        start: -moon.elapsed * first_days,
        days: CYCLE_YEARS * year_days,
    };

    let mut parent = cycle;
    let mut lords = Vec::with_capacity(path.len());
    for name in path {
        parent = parent
            .children()
            .find(|c| DASHA_LORDS[c.lord].0.eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| EngineError::InvalidInput(format!("not a dasha lord: {name}")))?;
        lords.push(DASHA_LORDS[parent.lord].0);
    }

    let at = |days: f64| -> String {
        let micros = (days * 86_400.0).round() as i64 * 1_000_000;
        birth.add_micros(micros).isoformat()
    };
    fn tree(span: Span, depth: usize, at: &dyn Fn(f64) -> String) -> Vec<DashaPeriod> {
        if depth == 0 {
            return Vec::new();
        }
        span.children()
            .map(|c| DashaPeriod {
                lord: DASHA_LORDS[c.lord].0,
                start: at(c.start),
                end: at(c.start + c.days),
                periods: tree(c, depth - 1, at),
            })
            .collect()
    }

    let mut current = Vec::new();
    if let Some(date) = date {
        let offset = date.seconds_since(&birth) / 86_400.0;
        let mut span = cycle;
        if offset >= span.start && offset < span.start + span.days {
            for _ in 0..MAX_DASHA_LEVELS {
                let Some(c) = span
                    .children()
                    .find(|c| offset < c.start + c.days)
                    .or_else(|| span.children().last())
                else {
                    break;
                };
                current.push(DashaPeriod {
                    lord: DASHA_LORDS[c.lord].0,
                    start: at(c.start),
                    end: at(c.start + c.days),
                    periods: Vec::new(),
                });
                span = c;
            }
        }
    }

    Ok(Vimshottari {
        year,
        birth_lord: DASHA_LORDS[first].0,
        balance_years: (1.0 - moon.elapsed) * DASHA_LORDS[first].1,
        path: lords,
        periods: tree(parent, depth, &at),
        current,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn birth() -> UtcInstant {
        UtcInstant::parse("2000-01-01T00:00:00Z").unwrap()
    }

    #[test]
    fn mahadashas_start_with_the_moons_lord_and_fill_120_years() {
        // 10° sidereal: Ashwini (Ketu), three quarters elapsed.
        let d = vimshottari(birth(), 10.0, "julian", &[], 1, None).unwrap();
        assert_eq!(d.birth_lord, "South Node");
        assert!((d.balance_years - 7.0 * 0.25).abs() < 1e-12);
        let lords: Vec<&str> = d.periods.iter().map(|p| p.lord).collect();
        assert_eq!(
            lords,
            [
                "South Node",
                "Venus",
                "Sun",
                "Moon",
                "Mars",
                "North Node",
                "Jupiter",
                "Saturn",
                "Mercury"
            ]
        );
        // Ketu's balance: 1.75 years of 365.25 days after the birth.
        assert_eq!(d.periods[0].end, "2001-10-01T04:30:00+00:00");
        assert_eq!(d.periods[1].start, d.periods[0].end);
        let start = UtcInstant::parse(&d.periods[0].start).unwrap();
        let end = UtcInstant::parse(&d.periods[8].end).unwrap();
        assert!((end.seconds_since(&start) / 86_400.0 - 120.0 * 365.25).abs() < 1e-3);
    }

    #[test]
    fn subperiods_follow_their_parent() {
        let path = ["Venus".to_string()];
        let d = vimshottari(birth(), 10.0, "savana", &path, 2, None).unwrap();
        assert_eq!(d.path, ["Venus"]);
        assert_eq!(d.periods[0].lord, "Venus");
        assert_eq!(d.periods[1].lord, "Sun");
        assert_eq!(d.periods[0].periods.len(), 9);
        // Venus-Venus: 20 × 20 / 120 years of 360 days.
        let start = UtcInstant::parse(&d.periods[0].start).unwrap();
        let end = UtcInstant::parse(&d.periods[0].end).unwrap();
        assert!((end.seconds_since(&start) / 86_400.0 - 20.0 * 20.0 / 120.0 * 360.0).abs() < 1e-3);
    }

    #[test]
    fn current_periods_contain_the_date() {
        let date = UtcInstant::parse("2030-06-15T00:00:00Z").unwrap();
        let d = vimshottari(birth(), 200.0, "julian", &[], 1, Some(date)).unwrap();
        assert_eq!(d.current.len(), MAX_DASHA_LEVELS);
        for p in &d.current {
            assert!(UtcInstant::parse(&p.start).unwrap() <= date, "{p:?}");
            assert!(date < UtcInstant::parse(&p.end).unwrap(), "{p:?}");
        }
        let late = UtcInstant::parse("2200-01-01T00:00:00Z").unwrap();
        assert!(vimshottari(birth(), 200.0, "julian", &[], 1, Some(late))
            .unwrap()
            .current
            .is_empty());
    }

    #[test]
    fn bad_options_are_rejected() {
        assert!(vimshottari(birth(), 10.0, "lunar", &[], 1, None).is_err());
        assert!(vimshottari(birth(), 10.0, "julian", &[], 4, None).is_err());
        assert!(vimshottari(birth(), 10.0, "julian", &["Pluto".into()], 1, None).is_err());
        let deep: Vec<String> = ["Venus", "Venus", "Venus"].map(String::from).to_vec();
        assert!(vimshottari(birth(), 10.0, "julian", &deep, 3, None).is_err());
    }
}
