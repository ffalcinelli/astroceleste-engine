//! Divisional charts (vargas): each sign is cut into `n` parts, and each part maps to a
//! sign of the varga by Parashara's rules (Brihat Parashara Hora Shastra, ch. 6-7).

use crate::error::EngineError;

/// The vargas computed: the sixteen of Parashara's shodashavarga plus D5, D6, D8 and D11.
pub const VARGAS: [u8; 20] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 16, 20, 24, 27, 30, 40, 45, 60,
];

/// How the hora (D2) is cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hora {
    /// Parashara: the Sun's hora (Leo) and the Moon's (Cancer), Leo first in odd signs.
    #[default]
    Parashara,
    /// Cyclic (parivritti): the halves run through the zodiac from Aries.
    Cyclic,
}

impl Hora {
    /// "cyclic" (any case) or "parivritti" is [`Hora::Cyclic`]; anything else is Parashara's.
    pub fn from_code(code: &str) -> Self {
        let code = code.trim();
        if code.eq_ignore_ascii_case("cyclic") || code.eq_ignore_ascii_case("parivritti") {
            Hora::Cyclic
        } else {
            Hora::Parashara
        }
    }
}

/// The sign (0 = Aries) and degree within it (0-30) of sidereal longitude `longitude` in
/// varga D`n`. The degree is the position within the part, stretched to a whole sign.
pub fn varga_position(longitude: f64, n: u8, hora: Hora) -> Result<(usize, f64), EngineError> {
    if !VARGAS.contains(&n) {
        return Err(EngineError::InvalidInput(format!("no varga D{n}")));
    }
    if !longitude.is_finite() {
        return Err(EngineError::InvalidInput(format!(
            "longitude must be finite, got {longitude}"
        )));
    }
    let longitude = longitude.rem_euclid(360.0);
    let sign = ((longitude / 30.0) as usize).min(11);
    let within = (longitude - sign as f64 * 30.0).clamp(0.0, 30.0);
    let part = ((within * f64::from(n) / 30.0) as usize).min(usize::from(n) - 1);
    let degree = (within * f64::from(n)).rem_euclid(30.0);
    // Signs by their 1-based parity: Aries (index 0) is odd.
    let odd = sign % 2 == 0;
    // Movable (cardinal), fixed and dual (mutable) signs.
    let modality = sign % 3;
    let element = sign % 4; // fire, earth, air, water

    let from = |start: usize| (start + part) % 12;
    let target = match n {
        1 => sign,
        2 => match hora {
            Hora::Parashara => {
                let first_half = part == 0;
                if odd == first_half {
                    4 // Leo, the Sun's
                } else {
                    3 // Cancer, the Moon's
                }
            }
            Hora::Cyclic => from(sign * 2),
        },
        // Drekkana: the sign itself, its 5th and its 9th.
        3 => (sign + part * 4) % 12,
        // Chaturthamsha: the sign and its kendras.
        4 => (sign + part * 3) % 12,
        // Panchamsha: the lords' odd signs (Mars, Saturn, Jupiter, Mercury, Venus), even
        // signs their even ones.
        5 => {
            if odd {
                [0, 10, 8, 2, 6][part]
            } else {
                [1, 5, 11, 9, 7][part]
            }
        }
        // Shashthamsha, Ashtamsha and Rudramsha: cyclic from Aries.
        6 | 8 | 11 => from(sign * usize::from(n)),
        // Saptamsha: odd signs from themselves, even signs from their 7th.
        7 => from(if odd { sign } else { sign + 6 }),
        // Navamsha: fire from Aries, earth from Capricorn, air from Libra, water from Cancer.
        9 => from([0, 9, 6, 3][element]),
        // Dashamsha: odd signs from themselves, even signs from their 9th.
        10 => from(if odd { sign } else { sign + 8 }),
        // Dwadashamsha: from the sign itself.
        12 => from(sign),
        // Shodashamsha and Akshavedamsha: movable from Aries, fixed from Leo, dual from
        // Sagittarius.
        16 | 45 => from([0, 4, 8][modality]),
        // Vimshamsha: movable from Aries, fixed from Sagittarius, dual from Leo.
        20 => from([0, 8, 4][modality]),
        // Siddhamsha: odd signs from Leo, even signs from Cancer.
        24 => from(if odd { 4 } else { 3 }),
        // Nakshatramsha: fire from Aries, earth from Cancer, air from Libra, water from
        // Capricorn.
        27 => from([0, 3, 6, 9][element]),
        // Trimshamsha: unequal parts ruled by Mars, Saturn, Jupiter, Mercury and Venus.
        30 => trimshamsha(within, odd),
        // Khavedamsha: odd signs from Aries, even signs from Libra.
        40 => from(if odd { 0 } else { 6 }),
        // Shashtyamsha: from the sign itself.
        60 => from(sign),
        _ => unreachable!("checked against VARGAS"),
    };
    Ok((target, degree))
}

/// Trimshamsha sign of `within` degrees of an odd or even sign.
fn trimshamsha(within: f64, odd: bool) -> usize {
    // (upper bound, sign): odd signs Mars 5°, Saturn 5°, Jupiter 8°, Mercury 7°, Venus 5°;
    // even signs the reverse with the lords' even signs.
    const ODD: [(f64, usize); 5] = [(5.0, 0), (10.0, 10), (18.0, 8), (25.0, 2), (30.0, 6)];
    const EVEN: [(f64, usize); 5] = [(5.0, 1), (12.0, 5), (20.0, 11), (25.0, 9), (30.0, 7)];
    let table = if odd { &ODD } else { &EVEN };
    table
        .iter()
        .find(|(upper, _)| within < *upper)
        .map_or(table[4].1, |(_, sign)| *sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(longitude: f64, n: u8) -> usize {
        varga_position(longitude, n, Hora::Parashara).unwrap().0
    }

    #[test]
    fn navamsha_follows_the_elements() {
        assert_eq!(sign(0.0, 9), 0); // 0° Aries: Aries
        assert_eq!(sign(29.9, 9), 8); // end of Aries: Sagittarius
        assert_eq!(sign(30.0, 9), 9); // 0° Taurus: Capricorn
        assert_eq!(sign(45.0, 9), 1); // 15° Taurus: part 4 from Capricorn, Taurus
        assert_eq!(sign(90.0, 9), 3); // 0° Cancer: Cancer
        assert_eq!(sign(359.9, 9), 11); // end of Pisces: Pisces (vargottama)
    }

    #[test]
    fn hora_parashara_and_cyclic() {
        assert_eq!(sign(5.0, 2), 4); // Aries 1st half: Leo
        assert_eq!(sign(20.0, 2), 3); // Aries 2nd half: Cancer
        assert_eq!(sign(35.0, 2), 3); // Taurus 1st half: Cancer
        assert_eq!(sign(50.0, 2), 4); // Taurus 2nd half: Leo
        let cyclic = |lon| varga_position(lon, 2, Hora::Cyclic).unwrap().0;
        assert_eq!((cyclic(5.0), cyclic(20.0), cyclic(35.0)), (0, 1, 2));
    }

    #[test]
    fn trimshamsha_boundaries_are_half_open() {
        assert_eq!(sign(4.99, 30), 0); // odd, Mars
        assert_eq!(sign(5.0, 30), 10); // odd, Saturn from 5°
        assert_eq!(sign(18.0, 30), 2); // odd, Mercury from 18°
        assert_eq!(sign(30.0 + 12.0, 30), 11); // even, Jupiter from 12°
        assert_eq!(sign(30.0 + 29.9, 30), 7); // even, Mars
    }

    #[test]
    fn other_vargas() {
        assert_eq!(sign(15.0, 3), 4); // Aries 2nd drekkana: Leo
        assert_eq!(sign(30.0 + 25.0, 7), 0); // Taurus part 5 from Scorpio: Aries
        assert_eq!(sign(30.0 + 10.0, 10), (1 + 8 + 3) % 12); // Taurus part 3 from Capricorn
        assert_eq!(sign(60.0 + 1.0, 16), 8); // Gemini (dual) from Sagittarius
        assert_eq!(sign(30.0 + 1.0, 20), 8); // Taurus (fixed) from Sagittarius
        assert_eq!(sign(30.0 + 1.0, 24), 3); // even from Cancer
        assert_eq!(sign(90.0 + 1.0, 27), 9); // water from Capricorn
        assert_eq!(sign(30.5, 40), 6); // even from Libra
        assert_eq!(sign(30.0 + 29.9, 60), (1 + 59) % 12);
        assert_eq!(sign(30.0 + 1.0, 8), 8); // Taurus: Sagittarius
        let (s, degree) = varga_position(3.5, 9, Hora::Parashara).unwrap();
        assert_eq!(s, 1);
        assert!((degree - 1.5).abs() < 1e-9);
    }

    #[test]
    fn unknown_vargas_are_rejected() {
        assert!(varga_position(10.0, 13, Hora::Parashara).is_err());
        assert!(varga_position(f64::NAN, 9, Hora::Parashara).is_err());
    }
}
