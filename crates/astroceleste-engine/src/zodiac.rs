//! Zodiac signs, house placement and ayanamsas (`charts/calc/zodiac.py`).

use crate::pyfloat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sign {
    pub name: &'static str,
    pub symbol: &'static str,
    pub element: &'static str,
    pub quality: &'static str,
}

pub const ZODIAC_SIGNS: [Sign; 12] = [
    Sign {
        name: "Aries",
        symbol: "♈",
        element: "Fire",
        quality: "Cardinal",
    },
    Sign {
        name: "Taurus",
        symbol: "♉",
        element: "Earth",
        quality: "Fixed",
    },
    Sign {
        name: "Gemini",
        symbol: "♊",
        element: "Air",
        quality: "Mutable",
    },
    Sign {
        name: "Cancer",
        symbol: "♋",
        element: "Water",
        quality: "Cardinal",
    },
    Sign {
        name: "Leo",
        symbol: "♌",
        element: "Fire",
        quality: "Fixed",
    },
    Sign {
        name: "Virgo",
        symbol: "♍",
        element: "Earth",
        quality: "Mutable",
    },
    Sign {
        name: "Libra",
        symbol: "♎",
        element: "Air",
        quality: "Cardinal",
    },
    Sign {
        name: "Scorpio",
        symbol: "♏",
        element: "Water",
        quality: "Fixed",
    },
    Sign {
        name: "Sagittarius",
        symbol: "♐",
        element: "Fire",
        quality: "Mutable",
    },
    Sign {
        name: "Capricorn",
        symbol: "♑",
        element: "Earth",
        quality: "Cardinal",
    },
    Sign {
        name: "Aquarius",
        symbol: "♒",
        element: "Air",
        quality: "Fixed",
    },
    Sign {
        name: "Pisces",
        symbol: "♓",
        element: "Water",
        quality: "Mutable",
    },
];

/// A longitude as sign, whole degree and rounded minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZodiacPosition {
    pub sign: Sign,
    pub degree: i64,
    pub minute: i64,
}

/// `longitude_to_zodiac`: minutes are rounded, carrying into the next degree and sign.
pub fn longitude_to_zodiac(lon: f64) -> ZodiacPosition {
    let norm = pyfloat::rem(lon, 360.0);
    let mut sign_index = pyfloat::floordiv(norm, 30.0) as usize;
    let deg_in_sign = norm - (sign_index as f64 * 30.0);
    let mut degree = deg_in_sign as i64;
    let mut minute = pyfloat::round_int((deg_in_sign - degree as f64) * 60.0);
    if minute == 60 {
        minute = 0;
        degree += 1;
        if degree == 30 {
            degree = 0;
            sign_index = (sign_index + 1) % 12;
        }
    }
    ZodiacPosition {
        sign: ZODIAC_SIGNS[sign_index],
        degree,
        minute,
    }
}

/// House (1-12) containing `lon`, given the twelve cusp longitudes.
pub fn determine_house(lon: f64, cusps: &[f64]) -> u8 {
    if cusps.len() != 12 {
        return 1;
    }
    for h in 0..12 {
        let h1 = cusps[h];
        let h2 = cusps[(h + 1) % 12];
        let inside = if h2 < h1 {
            lon >= h1 || lon < h2
        } else {
            h1 <= lon && lon < h2
        };
        if inside {
            return h as u8 + 1;
        }
    }
    1
}

/// `format_degrees_to_dms`: `DD° MM' SS"`.
pub fn format_degrees_to_dms(value: f64) -> String {
    let norm = pyfloat::rem(pyfloat::rem(value, 360.0) + 360.0, 360.0);
    let mut deg = norm as i64;
    let rem_min = (norm - deg as f64) * 60.0;
    let mut mnt = rem_min as i64;
    let mut sec = pyfloat::round_int((rem_min - mnt as f64) * 60.0);
    if sec == 60 {
        sec = 0;
        mnt += 1;
    }
    if mnt == 60 {
        mnt = 0;
        deg += 1;
    }
    deg = deg.rem_euclid(360);
    format!("{deg}° {mnt:02}' {sec:02}\"")
}

pub const PRECESSION_RATE_ARCSEC_YEAR: f64 = 50.290966;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ayanamsa {
    pub code: &'static str,
    pub name: &'static str,
    pub offset_j2000: f64,
}

pub const AYANAMSA_CATALOG: [Ayanamsa; 14] = [
    Ayanamsa {
        code: "lahiri",
        name: "Lahiri (Chitra Paksha)",
        offset_j2000: 23.857092,
    },
    Ayanamsa {
        code: "fagan_bradley",
        name: "Fagan-Bradley",
        offset_j2000: 24.743118,
    },
    Ayanamsa {
        code: "krishnamurti",
        name: "Krishnamurti (KP)",
        offset_j2000: 23.766782,
    },
    Ayanamsa {
        code: "raman",
        name: "B.V. Raman",
        offset_j2000: 22.404764,
    },
    Ayanamsa {
        code: "true_citra",
        name: "True Chitra",
        offset_j2000: 23.857187,
    },
    Ayanamsa {
        code: "yukteshwar",
        name: "Sri Yukteswar",
        offset_j2000: 21.054378,
    },
    Ayanamsa {
        code: "deluce",
        name: "De Luce",
        offset_j2000: 23.818451,
    },
    Ayanamsa {
        code: "ushashashi",
        name: "Usha-Shashi",
        offset_j2000: 20.016333,
    },
    Ayanamsa {
        code: "jn_bhasin",
        name: "J.N. Bhasin",
        offset_j2000: 23.324889,
    },
    Ayanamsa {
        code: "aldebaran_15tau",
        name: "Aldebaran 15° Taurus",
        offset_j2000: 24.960222,
    },
    Ayanamsa {
        code: "hipparchos",
        name: "Hipparchos",
        offset_j2000: 23.238889,
    },
    Ayanamsa {
        code: "sassanian",
        name: "Sassanian",
        offset_j2000: 22.846667,
    },
    Ayanamsa {
        code: "galcent_0sag",
        name: "Galactic Center 0° Sagittarius",
        offset_j2000: 26.998611,
    },
    Ayanamsa {
        code: "j2000",
        name: "J2000.0 Epoch",
        offset_j2000: 0.0,
    },
];

pub const DEFAULT_AYANAMSA: &str = "galcent_0sag";

/// Catalog entry for a code (case-insensitive), falling back to the default.
pub fn ayanamsa(code: &str) -> &'static Ayanamsa {
    let code = code.trim().to_lowercase();
    AYANAMSA_CATALOG
        .iter()
        .find(|a| a.code == code)
        .or_else(|| AYANAMSA_CATALOG.iter().find(|a| a.code == DEFAULT_AYANAMSA))
        .unwrap()
}

#[derive(Debug, Clone, PartialEq)]
pub struct AyanamsaInfo {
    pub code: &'static str,
    pub name: &'static str,
    /// Degrees, rounded to 6 decimals.
    pub value: f64,
    pub formatted: String,
    pub precession_rate_arcsec_yr: f64,
}

/// `get_ayanamsa_info` as production computes it (no Swiss Ephemeris): J2000 offset plus
/// linear precession.
///
/// NOTE: this reproduces the reference implementation, including its unit slip (a per-year
/// rate applied to centuries) — see the fixture discussion before changing it.
pub fn ayanamsa_info(jd: f64, code: &str) -> AyanamsaInfo {
    let meta = ayanamsa(code);
    let centuries = (jd - 2_451_545.0) / 36525.0;
    let precession_deg = centuries * (PRECESSION_RATE_ARCSEC_YEAR / 3600.0);
    let value = pyfloat::rem(meta.offset_j2000 + precession_deg, 360.0);
    AyanamsaInfo {
        code: meta.code,
        name: meta.name,
        value: pyfloat::round(value, 6),
        formatted: format_degrees_to_dms(value),
        precession_rate_arcsec_yr: pyfloat::round(PRECESSION_RATE_ARCSEC_YEAR, 4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minute_rounding_carries_into_next_sign() {
        let p = longitude_to_zodiac(29.9999);
        assert_eq!((p.sign.name, p.degree, p.minute), ("Taurus", 0, 0));
        let p = longitude_to_zodiac(-0.5);
        assert_eq!((p.sign.name, p.degree, p.minute), ("Pisces", 29, 30));
    }

    #[test]
    fn house_placement_wraps_at_aries() {
        let cusps: Vec<f64> = (0..12).map(|i| (350.0 + 30.0 * i as f64) % 360.0).collect();
        assert_eq!(determine_house(355.0, &cusps), 1);
        assert_eq!(determine_house(5.0, &cusps), 1);
        assert_eq!(determine_house(20.0, &cusps), 2);
    }
}
