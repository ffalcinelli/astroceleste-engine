//! The panchanga of a moment: the Vedic day (from sunrise) and its weekday, the five angas
//! (tithi, nakshatra, yoga, karana, vara) running at the moment with their start and end,
//! the lunar month and the year.
//!
//! The sidereal angles use the Lahiri ayanamsa, the panchanga standard. Months are amanta
//! (new moon to new moon), named after the sidereal sign the Sun enters during them; a
//! month with no such ingress is adhika (intercalary) and takes the next month's name. The
//! year (samvatsara, Shaka and Vikram eras) starts with the month of Chaitra.

use serde::Serialize;

use super::{nakshatra, NAKSHATRAS, NAKSHATRA_SPAN};
use crate::almanac::rise_set;
use crate::ephemeris::KernelSet;
use crate::error::EngineError;
use crate::instant::UtcInstant;
use crate::search::{crossing, instant_of, Sky, ELONGATION_RATE, SUN_RATE, SYNODIC_MONTH};
use crate::zodiac::ayanamsa_value;

/// The ayanamsa of the panchanga.
const AYANAMSA: &str = "lahiri";
/// Mean sidereal motion of the Moon, degrees per day.
const MOON_RATE: f64 = 13.176_358;

/// The fifteen tithis of a fortnight; the fifteenth is purnima (full moon) in the bright
/// half and amavasya (new moon) in the dark half.
const TITHIS: [&str; 14] = [
    "pratipada",
    "dvitiya",
    "tritiya",
    "chaturthi",
    "panchami",
    "shashthi",
    "saptami",
    "ashtami",
    "navami",
    "dashami",
    "ekadashi",
    "dvadashi",
    "trayodashi",
    "chaturdashi",
];

/// The weekdays from Sunday, with their lords.
const VARAS: [(&str, &str); 7] = [
    ("ravivara", "Sun"),
    ("somavara", "Moon"),
    ("mangalavara", "Mars"),
    ("budhavara", "Mercury"),
    ("guruvara", "Jupiter"),
    ("shukravara", "Venus"),
    ("shanivara", "Saturn"),
];

const YOGAS: [&str; 27] = [
    "vishkambha",
    "priti",
    "ayushman",
    "saubhagya",
    "shobhana",
    "atiganda",
    "sukarman",
    "dhriti",
    "shula",
    "ganda",
    "vriddhi",
    "dhruva",
    "vyaghata",
    "harshana",
    "vajra",
    "siddhi",
    "vyatipata",
    "variyana",
    "parigha",
    "shiva",
    "siddha",
    "sadhya",
    "shubha",
    "shukla",
    "brahma",
    "indra",
    "vaidhriti",
];

/// The seven movable karanas, which repeat eight times from the second half-tithi.
const MOVABLE_KARANAS: [&str; 7] = [
    "bava", "balava", "kaulava", "taitila", "gara", "vanija", "vishti",
];

/// The lunar months from Chaitra.
const MASAS: [&str; 12] = [
    "chaitra",
    "vaishakha",
    "jyeshtha",
    "ashadha",
    "shravana",
    "bhadrapada",
    "ashvina",
    "kartika",
    "margashirsha",
    "pausha",
    "magha",
    "phalguna",
];

/// The seasons, two months each from Chaitra.
const RITUS: [&str; 6] = [
    "vasanta", "grishma", "varsha", "sharad", "hemanta", "shishira",
];

/// The sixty years of the Jupiter cycle, from Prabhava.
const SAMVATSARAS: [&str; 60] = [
    "prabhava",
    "vibhava",
    "shukla",
    "pramoda",
    "prajapati",
    "angirasa",
    "shrimukha",
    "bhava",
    "yuva",
    "dhatri",
    "ishvara",
    "bahudhanya",
    "pramathi",
    "vikrama",
    "vrisha",
    "chitrabhanu",
    "svabhanu",
    "tarana",
    "parthiva",
    "vyaya",
    "sarvajit",
    "sarvadhari",
    "virodhi",
    "vikriti",
    "khara",
    "nandana",
    "vijaya",
    "jaya",
    "manmatha",
    "durmukhi",
    "hevilambi",
    "vilambi",
    "vikari",
    "sharvari",
    "plava",
    "shubhakrit",
    "shobhakrit",
    "krodhi",
    "vishvavasu",
    "parabhava",
    "plavanga",
    "kilaka",
    "saumya",
    "sadharana",
    "virodhakrit",
    "paridhavi",
    "pramadi",
    "ananda",
    "rakshasa",
    "nala",
    "pingala",
    "kalayukti",
    "siddharthi",
    "raudra",
    "durmati",
    "dundubhi",
    "rudhirodgari",
    "raktakshi",
    "krodhana",
    "akshaya",
];

/// One of the angas running at the moment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Anga {
    /// 1-based, in its cycle (tithi 1-30, nakshatra and yoga 1-27, karana 1-60).
    pub number: u8,
    /// Stable code.
    pub code: &'static str,
    /// When it began, ISO 8601 UTC to the second.
    pub start: String,
    /// When it ends, ISO 8601 UTC to the second.
    pub end: String,
}

/// The weekday of the Vedic day, which begins at sunrise.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Vara {
    /// 1 (Sunday) to 7 (Saturday).
    pub number: u8,
    /// Stable code, e.g. "ravivara".
    pub code: &'static str,
    /// The ruling body.
    pub lord: &'static str,
}

/// The lunar month.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Masa {
    /// The amanta month (new moon to new moon).
    pub amanta: &'static str,
    /// The purnimanta month (full moon to full moon): the next one in the dark half.
    pub purnimanta: &'static str,
    /// Intercalary: no sidereal ingress of the Sun during the month.
    pub adhika: bool,
    /// The new moon that begins the amanta month, ISO 8601 UTC to the second.
    pub start: String,
    /// The new moon that ends it.
    pub end: String,
}

/// The panchanga of a moment and place: [`vedic_calendar`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VedicCalendar {
    /// The ayanamsa of the sidereal angles: "lahiri".
    pub ayanamsa: &'static str,
    /// The sunrise that begins the Vedic day, the sunset and the next sunrise, ISO 8601
    /// UTC. Where the Sun does not rise or set, half-day defaults stand in.
    pub sunrise: String,
    /// Sunset of the Vedic day.
    pub sunset: String,
    /// The sunrise that ends the Vedic day.
    pub next_sunrise: String,
    /// The weekday of the Vedic day.
    pub vara: Vara,
    /// The lunar day: each 12° of the Moon's elongation from the Sun.
    pub tithi: Anga,
    /// "shukla" (waxing, tithis 1-15) or "krishna" (waning, 16-30).
    pub paksha: &'static str,
    /// The Moon's nakshatra.
    pub nakshatra: Anga,
    /// The quarter of the nakshatra the Moon is in, 1 to 4.
    pub pada: u8,
    /// The sum of the Sun's and the Moon's sidereal longitudes, in 27 parts.
    pub yoga: Anga,
    /// Half a tithi.
    pub karana: Anga,
    /// The lunar month.
    pub masa: Masa,
    /// The season.
    pub ritu: &'static str,
    /// "uttarayana" (the Sun in sidereal Capricorn to Gemini) or "dakshinayana".
    pub ayana: &'static str,
    /// The year of the sixty-year cycle, 1 (Prabhava) to 60 (Akshaya), as reckoned in
    /// southern India (with the Shaka era).
    pub samvatsara_number: u8,
    /// Its code.
    pub samvatsara: &'static str,
    /// Year of the Shaka era.
    pub shaka_year: i32,
    /// Year of the Vikram era.
    pub vikram_year: i32,
}

/// The panchanga at `instant` for a place, or `None` when the loaded kernels do not cover
/// the lunar month around it.
pub fn vedic_calendar(
    kernels: &KernelSet,
    instant: UtcInstant,
    latitude: f64,
    longitude: f64,
) -> Result<Option<VedicCalendar>, EngineError> {
    match calendar(kernels, instant, latitude, longitude) {
        Err(EngineError::OutOfRange { .. }) => Ok(None),
        other => other.map(Some),
    }
}

fn iso(jd: f64) -> String {
    instant_of(jd).isoformat()
}

/// The anga of `f` (degrees growing at about `rate` per day) at `jd`, in parts of `span`:
/// its 0-based index and the moments it began and ends.
fn anga(
    f: impl Fn(f64) -> Result<f64, EngineError>,
    jd: f64,
    span: f64,
    parts: usize,
    rate: f64,
) -> Result<(usize, f64, f64), EngineError> {
    let value = f(jd)?.rem_euclid(360.0);
    let index = ((value / span) as usize).min(parts - 1);
    let from = index as f64 * span;
    let start = crossing(&f, from, jd - (value - from) / rate, rate)?;
    let end = crossing(&f, from + span, jd + (from + span - value) / rate, rate)?;
    Ok((index, start, end))
}

fn calendar(
    kernels: &KernelSet,
    instant: UtcInstant,
    latitude: f64,
    longitude: f64,
) -> Result<VedicCalendar, EngineError> {
    let sky = Sky { kernels };
    let jd = instant.julian_day();
    let ayanamsa = |jd: f64| ayanamsa_value(jd, AYANAMSA);
    let sidereal_sun = |jd: f64| -> Result<f64, EngineError> { Ok(sky.sun(jd)? - ayanamsa(jd)) };
    let sidereal_moon = |jd: f64| -> Result<f64, EngineError> { Ok(sky.moon(jd)? - ayanamsa(jd)) };

    // The Vedic day begins at the sunrise at or before the moment (rise_set allows a
    // little slack after it).
    let (mut rise, mut set, mut next_rise) = rise_set(kernels, jd, latitude, longitude)?;
    if rise > jd {
        (rise, set, next_rise) = rise_set(kernels, rise - 0.2, latitude, longitude)?;
    }
    // The weekday of the sunrise's date in local mean time.
    let weekday = ((rise + longitude / 360.0 + 1.5).floor() as i64).rem_euclid(7) as usize;

    let (tithi, tithi_start, tithi_end) =
        anga(|jd| sky.elongation(jd), jd, 12.0, 30, ELONGATION_RATE)?;
    let paksha = if tithi < 15 { "shukla" } else { "krishna" };
    let tithi_code = match tithi {
        14 => "purnima",
        29 => "amavasya",
        t => TITHIS[t % 15],
    };

    let (star, star_start, star_end) = anga(sidereal_moon, jd, NAKSHATRA_SPAN, 27, MOON_RATE)?;
    let pada = nakshatra(sidereal_moon(jd)?).pada;

    let (yoga, yoga_start, yoga_end) = anga(
        |jd| Ok(sidereal_sun(jd)? + sidereal_moon(jd)?),
        jd,
        NAKSHATRA_SPAN,
        27,
        MOON_RATE + SUN_RATE,
    )?;

    let (karana, karana_start, karana_end) =
        anga(|jd| sky.elongation(jd), jd, 6.0, 60, ELONGATION_RATE)?;
    let karana_code = match karana {
        0 => "kimstughna",
        57 => "shakuni",
        58 => "chatushpada",
        59 => "naga",
        k => MOVABLE_KARANAS[(k - 1) % 7],
    };

    // The amanta month, named after the sign the Sun enters during it.
    let month_start = sky.new_moon_before(jd)?;
    let month_end = sky.new_moon_near(month_start + SYNODIC_MONTH)?;
    let sign_at = |jd: f64| -> Result<usize, EngineError> {
        Ok(((sidereal_sun(jd)?.rem_euclid(360.0) / 30.0) as usize).min(11))
    };
    let first_sign = sign_at(month_start)?;
    let adhika = sign_at(month_end)? == first_sign;
    let masa = (first_sign + 1) % 12;
    let purnimanta = if tithi < 15 { masa } else { (masa + 1) % 12 };

    let sun_now = sidereal_sun(jd)?.rem_euclid(360.0);
    let ayana = if !(90.0..270.0).contains(&sun_now) {
        "uttarayana"
    } else {
        "dakshinayana"
    };

    // The year began with the last Chaitra, which starts in March or April: January and
    // February, and Magha and Phalguna in March or April, belong to the year before's.
    let (civil_year, civil_month, ..) = instant.civil();
    let year_start = if civil_month < 3 || (civil_month <= 4 && masa >= 10) {
        civil_year - 1
    } else {
        civil_year
    };
    let shaka_year = year_start - 78;
    let samvatsara = (shaka_year + 11).rem_euclid(60) as usize; // 0-based

    Ok(VedicCalendar {
        ayanamsa: AYANAMSA,
        sunrise: iso(rise),
        sunset: iso(set),
        next_sunrise: iso(next_rise),
        vara: Vara {
            number: weekday as u8 + 1,
            code: VARAS[weekday].0,
            lord: VARAS[weekday].1,
        },
        tithi: Anga {
            number: tithi as u8 + 1,
            code: tithi_code,
            start: iso(tithi_start),
            end: iso(tithi_end),
        },
        paksha,
        nakshatra: Anga {
            number: star as u8 + 1,
            code: NAKSHATRAS[star],
            start: iso(star_start),
            end: iso(star_end),
        },
        pada,
        yoga: Anga {
            number: yoga as u8 + 1,
            code: YOGAS[yoga],
            start: iso(yoga_start),
            end: iso(yoga_end),
        },
        karana: Anga {
            number: karana as u8 + 1,
            code: karana_code,
            start: iso(karana_start),
            end: iso(karana_end),
        },
        masa: Masa {
            amanta: MASAS[masa],
            purnimanta: MASAS[purnimanta],
            adhika,
            start: iso(month_start),
            end: iso(month_end),
        },
        ritu: RITUS[masa / 2],
        ayana,
        samvatsara_number: samvatsara as u8 + 1,
        samvatsara: SAMVATSARAS[samvatsara],
        shaka_year,
        vikram_year: year_start + 57,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn karana_and_year_codes() {
        assert_eq!(MOVABLE_KARANAS[(56 - 1) % 7], "vishti");
        assert_eq!(SAMVATSARAS[(1946 + 11) % 60], "krodhi"); // Shaka 1946 = 2024-25
    }
}
