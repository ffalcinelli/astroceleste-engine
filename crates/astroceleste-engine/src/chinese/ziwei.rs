//! Zi Wei Dou Shu (紫微斗數), the Purple Star chart of a birth, cast from its
//! [`ChineseCalendar`]. No kernel is needed.
//!
//! The chart follows the classic rules of the *Zi Wei Dou Shu Quanshu*:
//!
//! - The year is the lunar year (it changes at the New Year), the month and day are the lunar
//!   ones, and the hour is the birth hour's branch on the clock the Four Pillars use (true
//!   solar or civil time, either Zi-hour convention). A birth in the second half of a leap
//!   month counts in the next month (`leap_month = "split"`, the default) or in the month it
//!   repeats (`"same"`).
//! - The life palace (命宮) is counted from 寅 forward to the month and back to the hour, the
//!   body palace (身宮) forward to the hour. Palace stems follow from the year stem (五虎遁).
//! - The five-element bureau (五行局) is the NaYin element of the life palace's stem and
//!   branch. Zi Wei is placed from the bureau and the lunar day, Tian Fu opposite it across
//!   the 寅-申 axis, and each leads its group of stars.
//! - The auxiliary and malefic stars follow the month (左輔, 右弼), the hour (文昌, 文曲,
//!   地空, 地劫), the year stem (天魁, 天鉞, 祿存, 擎羊, 陀羅) and the year branch (天馬,
//!   and with the hour 火星 and 鈴星).
//! - The Four Transformations (四化) are those of the year stem; for 庚 the 科 is 太陰 and
//!   the 忌 天同, for 壬 the 科 is 左輔.
//! - Brightness (廟旺得利平不陷) follows the common table, as published by the iztro project
//!   (MIT, see NOTICE).
//! - The decade limits (大限) run forward from the life palace for a yang year and a man or
//!   a yin year and a woman, backward otherwise, from the bureau's number of years (nominal
//!   age, 虛歲). The small limits (小限) start from 辰, 戌, 未 or 丑 by the year branch, forward
//!   for a man and backward for a woman.

use serde::Serialize;

use super::bazi::{bazi, nayin, BaziOptions, LunarDate};
use super::calendar::ChineseCalendar;
use super::{cycle_index, year_cycle_index, BRANCHES, STEMS};
use crate::error::EngineError;
use crate::instant::UtcInstant;

/// The twelve palaces, counted backward through the branches from the life palace.
const PALACES: [&str; 12] = [
    "life", "siblings", "spouse", "children", "wealth", "health", "travel", "friends", "career",
    "property", "fortune", "parents",
];

const ZI_WEI: &str = "zi_wei";
const TIAN_JI: &str = "tian_ji";
const TAI_YANG: &str = "tai_yang";
const WU_QU: &str = "wu_qu";
const TIAN_TONG: &str = "tian_tong";
const LIAN_ZHEN: &str = "lian_zhen";
const TIAN_FU: &str = "tian_fu";
const TAI_YIN: &str = "tai_yin";
const TAN_LANG: &str = "tan_lang";
const JU_MEN: &str = "ju_men";
const TIAN_XIANG: &str = "tian_xiang";
const TIAN_LIANG: &str = "tian_liang";
const QI_SHA: &str = "qi_sha";
const PO_JUN: &str = "po_jun";
const ZUO_FU: &str = "zuo_fu";
const YOU_BI: &str = "you_bi";
const WEN_CHANG: &str = "wen_chang";
const WEN_QU: &str = "wen_qu";
const TIAN_KUI: &str = "tian_kui";
const TIAN_YUE: &str = "tian_yue";
const LU_CUN: &str = "lu_cun";
const TIAN_MA: &str = "tian_ma";
const QING_YANG: &str = "qing_yang";
const TUO_LUO: &str = "tuo_luo";
const HUO_XING: &str = "huo_xing";
const LING_XING: &str = "ling_xing";
const DI_KONG: &str = "di_kong";
const DI_JIE: &str = "di_jie";

/// The Zi Wei group: each star's offset from Zi Wei, counted backward.
const ZI_WEI_GROUP: [(&str, usize); 6] = [
    (ZI_WEI, 0),
    (TIAN_JI, 1),
    (TAI_YANG, 3),
    (WU_QU, 4),
    (TIAN_TONG, 5),
    (LIAN_ZHEN, 8),
];

/// The Tian Fu group: each star's offset from Tian Fu, counted forward.
const TIAN_FU_GROUP: [(&str, usize); 8] = [
    (TIAN_FU, 0),
    (TAI_YIN, 1),
    (TAN_LANG, 2),
    (JU_MEN, 3),
    (TIAN_XIANG, 4),
    (TIAN_LIANG, 5),
    (QI_SHA, 6),
    (PO_JUN, 10),
];

/// The Four Transformations (祿, 權, 科, 忌) of each year stem.
const TRANSFORMATIONS: [[&str; 4]; 10] = [
    [LIAN_ZHEN, PO_JUN, WU_QU, TAI_YANG],       // 甲
    [TIAN_JI, TIAN_LIANG, ZI_WEI, TAI_YIN],     // 乙
    [TIAN_TONG, TIAN_JI, WEN_CHANG, LIAN_ZHEN], // 丙
    [TAI_YIN, TIAN_TONG, TIAN_JI, JU_MEN],      // 丁
    [TAN_LANG, TAI_YIN, YOU_BI, TIAN_JI],       // 戊
    [WU_QU, TAN_LANG, TIAN_LIANG, WEN_QU],      // 己
    [TAI_YANG, WU_QU, TAI_YIN, TIAN_TONG],      // 庚
    [JU_MEN, TAI_YANG, WEN_QU, WEN_CHANG],      // 辛
    [TIAN_LIANG, ZI_WEI, ZUO_FU, WU_QU],        // 壬
    [PO_JUN, JU_MEN, TAI_YIN, TAN_LANG],        // 癸
];

const TRANSFORMATION_KINDS: [&str; 4] = ["lu", "quan", "ke", "ji"];

const MIAO: Option<&str> = Some("miao");
const WANG: Option<&str> = Some("wang");
const DE: Option<&str> = Some("de");
const LI: Option<&str> = Some("li");
const PING: Option<&str> = Some("ping");
const BU: Option<&str> = Some("bu");
const XIAN: Option<&str> = Some("xian");

/// Brightness of a star in each branch, from 寅 (as the iztro project publishes it).
const BRIGHTNESS: [(&str, [Option<&str>; 12]); 20] = [
    (
        ZI_WEI,
        [
            WANG, WANG, DE, WANG, MIAO, MIAO, WANG, WANG, DE, WANG, PING, MIAO,
        ],
    ),
    (
        TIAN_JI,
        [
            DE, WANG, LI, PING, MIAO, XIAN, DE, WANG, LI, PING, MIAO, XIAN,
        ],
    ),
    (
        TAI_YANG,
        [
            WANG, MIAO, WANG, WANG, WANG, DE, DE, PING, BU, XIAN, XIAN, BU,
        ],
    ),
    (
        WU_QU,
        [
            DE, LI, MIAO, PING, WANG, MIAO, DE, LI, MIAO, PING, WANG, MIAO,
        ],
    ),
    (
        TIAN_TONG,
        [
            LI, PING, PING, MIAO, XIAN, BU, WANG, PING, PING, MIAO, WANG, BU,
        ],
    ),
    (
        LIAN_ZHEN,
        [
            MIAO, PING, LI, XIAN, PING, LI, MIAO, PING, LI, XIAN, PING, LI,
        ],
    ),
    (
        TIAN_FU,
        [
            MIAO, DE, MIAO, DE, WANG, MIAO, DE, WANG, MIAO, DE, MIAO, MIAO,
        ],
    ),
    (
        TAI_YIN,
        [
            WANG, XIAN, XIAN, XIAN, BU, BU, LI, WANG, WANG, MIAO, MIAO, MIAO,
        ],
    ),
    (
        TAN_LANG,
        [
            PING, LI, MIAO, XIAN, WANG, MIAO, PING, LI, MIAO, XIAN, WANG, MIAO,
        ],
    ),
    (
        JU_MEN,
        [
            MIAO, MIAO, XIAN, WANG, WANG, BU, MIAO, MIAO, XIAN, WANG, WANG, BU,
        ],
    ),
    (
        TIAN_XIANG,
        [MIAO, XIAN, DE, DE, MIAO, DE, MIAO, XIAN, DE, DE, MIAO, MIAO],
    ),
    (
        TIAN_LIANG,
        [
            MIAO, MIAO, MIAO, XIAN, MIAO, WANG, XIAN, DE, MIAO, XIAN, MIAO, WANG,
        ],
    ),
    (
        QI_SHA,
        [
            MIAO, WANG, MIAO, PING, WANG, MIAO, MIAO, WANG, MIAO, PING, WANG, MIAO,
        ],
    ),
    (
        PO_JUN,
        [
            DE, XIAN, WANG, PING, MIAO, WANG, DE, XIAN, WANG, PING, MIAO, WANG,
        ],
    ),
    (
        WEN_CHANG,
        [XIAN, LI, DE, MIAO, XIAN, LI, DE, MIAO, XIAN, LI, DE, MIAO],
    ),
    (
        WEN_QU,
        [
            PING, WANG, DE, MIAO, XIAN, WANG, DE, MIAO, XIAN, WANG, DE, MIAO,
        ],
    ),
    (
        HUO_XING,
        [MIAO, LI, XIAN, DE, MIAO, LI, XIAN, DE, MIAO, LI, XIAN, DE],
    ),
    (
        LING_XING,
        [MIAO, LI, XIAN, DE, MIAO, LI, XIAN, DE, MIAO, LI, XIAN, DE],
    ),
    (
        QING_YANG,
        [
            None, XIAN, MIAO, None, XIAN, MIAO, None, XIAN, MIAO, None, XIAN, MIAO,
        ],
    ),
    (
        TUO_LUO,
        [
            XIAN, None, MIAO, XIAN, None, MIAO, XIAN, None, MIAO, XIAN, None, MIAO,
        ],
    ),
];

/// The life master (命主) by the life palace's branch.
const LIFE_MASTERS: [&str; 12] = [
    TAN_LANG, JU_MEN, LU_CUN, WEN_QU, LIAN_ZHEN, WU_QU, PO_JUN, WU_QU, LIAN_ZHEN, WEN_QU, LU_CUN,
    JU_MEN,
];

/// The body master (身主) by the year branch.
const BODY_MASTERS: [&str; 12] = [
    HUO_XING, TIAN_XIANG, TIAN_LIANG, TIAN_TONG, WEN_CHANG, TIAN_JI, HUO_XING, TIAN_XIANG,
    TIAN_LIANG, TIAN_TONG, WEN_CHANG, TIAN_JI,
];

/// Branch indices.
const ZI: usize = 0;
const CHOU: usize = 1;
const YIN: usize = 2;
const MAO: usize = 3;
const CHEN: usize = 4;
const SI: usize = 5;
const WU: usize = 6;
const WEI: usize = 7;
const SHEN: usize = 8;
const YOU: usize = 9;
const XU: usize = 10;
const HAI: usize = 11;

/// How to cast the chart.
#[derive(Debug, Clone)]
pub struct ZiWeiOptions<'a> {
    /// Take the birth hour on true solar time at the birthplace (`true`, the default) or on
    /// civil time, as for the Four Pillars.
    pub solar_time: bool,
    /// "next_day" (default): from 23:00 the birth counts on the next lunar day; "split": the
    /// day changes at midnight.
    pub zi_hour: &'a str,
    /// "male" or "female": sets the direction of the decade and small limits, which are left
    /// out without it.
    pub sex: Option<&'a str>,
    /// "split" (default): a birth after the 15th of a leap month counts in the next month;
    /// "same": every day of a leap month counts in the month it repeats.
    pub leap_month: &'a str,
}

impl Default for ZiWeiOptions<'_> {
    fn default() -> Self {
        ZiWeiOptions {
            solar_time: true,
            zi_hour: "next_day",
            sex: None,
            leap_month: "split",
        }
    }
}

/// A star in a palace.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZiWeiStar {
    /// Star code, e.g. "zi_wei", "wen_chang".
    pub star: &'static str,
    /// "major" (the fourteen), "auxiliary" or "malefic".
    pub kind: &'static str,
    /// Brightness in this branch: "miao", "wang", "de", "li", "ping", "bu" or "xian"; `None`
    /// where the table gives none.
    pub brightness: Option<&'static str>,
    /// The natal transformation it takes: "lu", "quan", "ke" or "ji".
    pub transformation: Option<&'static str>,
}

/// A range of nominal ages (虛歲), inclusive.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgeRange {
    /// First age.
    pub start: u8,
    /// Last age.
    pub end: u8,
}

/// One of the twelve palaces.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZiWeiPalace {
    /// Palace code, from "life" to "parents".
    pub name: &'static str,
    /// Earthly Branch of its place on the board.
    pub branch: &'static str,
    /// Its Heavenly Stem.
    pub stem: &'static str,
    /// Whether the body palace falls here.
    pub is_body: bool,
    /// Its stars: the major ones first.
    pub stars: Vec<ZiWeiStar>,
    /// The decade limit it rules; `None` without the native's sex.
    pub decade: Option<AgeRange>,
    /// The nominal ages whose small limit falls here, up to 120; empty without the sex.
    pub small_limit_ages: Vec<u8>,
}

/// A natal transformation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZiWeiTransformation {
    /// "lu" (祿), "quan" (權), "ke" (科) or "ji" (忌).
    pub kind: &'static str,
    /// The star transformed.
    pub star: &'static str,
    /// The palace it is in.
    pub palace: &'static str,
}

/// The five-element bureau.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Bureau {
    /// "water", "wood", "metal", "earth" or "fire".
    pub element: &'static str,
    /// 2 (water) to 6 (fire): the age the first decade starts at.
    pub number: u8,
}

/// The Purple Star chart of a birth.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZiWei {
    /// "solar" or "civil": the clock the birth hour was taken on.
    pub time_basis: &'static str,
    /// "next_day" or "split".
    pub zi_hour: &'static str,
    /// The lunar date of birth.
    pub lunar_date: LunarDate,
    /// The month the chart is cast with (a leap month may count as the next one).
    pub month: u8,
    /// The birth hour's branch.
    pub hour_branch: &'static str,
    /// Branch of the life palace (命宮).
    pub life_palace: &'static str,
    /// Branch of the body palace (身宮).
    pub body_palace: &'static str,
    /// The five-element bureau (五行局).
    pub bureau: Bureau,
    /// The life master (命主), a star code.
    pub life_master: &'static str,
    /// The body master (身主), a star code.
    pub body_master: &'static str,
    /// The twelve palaces, from the life palace in palace order.
    pub palaces: Vec<ZiWeiPalace>,
    /// The four natal transformations, 祿 first.
    pub transformations: Vec<ZiWeiTransformation>,
    /// "forward" or "backward" (through the branches) for the decade limits; `None`
    /// without the native's sex.
    pub decade_direction: Option<&'static str>,
}

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidInput(message.to_string())
}

fn wrap(index: i64) -> usize {
    index.rem_euclid(12) as usize
}

/// The branch Zi Wei sits in for a bureau and a lunar day (1-30).
pub(crate) fn zi_wei_branch(bureau: u8, day: u8) -> usize {
    let (bureau, day) = (i64::from(bureau), i64::from(day));
    let quotient = (day + bureau - 1) / bureau;
    let added = quotient * bureau - day;
    let base = YIN as i64 + quotient - 1;
    wrap(if added % 2 == 1 {
        base - added
    } else {
        base + added
    })
}

/// The bureau of a life palace's stem and branch: its NaYin element.
fn bureau_of(stem: usize, branch: usize) -> Bureau {
    let nayin = nayin(cycle_index(stem, branch) / 2);
    let (element, number) = [
        ("water", 2),
        ("wood", 3),
        ("metal", 4),
        ("earth", 5),
        ("fire", 6),
    ]
    .into_iter()
    .find(|(element, _)| nayin.ends_with(element))
    .unwrap_or(("fire", 6));
    Bureau { element, number }
}

fn brightness(star: &str, branch: usize) -> Option<&'static str> {
    BRIGHTNESS
        .iter()
        .find(|(s, _)| *s == star)
        .and_then(|(_, table)| table[wrap(branch as i64 - YIN as i64)])
}

/// Cast the Purple Star chart of a birth at `birth` (UTC), at `longitude` (degrees east)
/// where civil time was `utc_offset_minutes` ahead of UTC, from its `calendar`.
pub fn zi_wei(
    calendar: &ChineseCalendar,
    birth: UtcInstant,
    longitude: f64,
    utc_offset_minutes: f64,
    options: &ZiWeiOptions,
) -> Result<ZiWei, EngineError> {
    let split_leap = match options.leap_month {
        "" | "split" => true,
        "same" => false,
        _ => return Err(invalid("leap_month must be \"split\" or \"same\"")),
    };
    let male = match options.sex {
        None | Some("") => None,
        Some("male") => Some(true),
        Some("female") => Some(false),
        Some(_) => return Err(invalid("sex must be \"male\" or \"female\"")),
    };
    let pillars = bazi(
        calendar,
        birth,
        longitude,
        utc_offset_minutes,
        &BaziOptions {
            solar_time: options.solar_time,
            zi_hour: options.zi_hour,
            sex: None,
            luck_pillars: 0,
        },
    )?;
    let lunar = pillars
        .lunar_date
        .clone()
        .ok_or_else(|| invalid("the calendar does not cover the birth date"))?;
    let hour = BRANCHES
        .iter()
        .position(|b| *b == pillars.hour.branch)
        .unwrap_or(ZI);
    let month = if lunar.leap && split_leap && lunar.day > 15 {
        lunar.month % 12 + 1
    } else {
        lunar.month
    };
    let year = year_cycle_index(lunar.year);
    let (year_stem, year_branch) = (year % 10, year % 12);

    // Life and body palaces, palace stems and the bureau.
    let month_branch = YIN as i64 + i64::from(month) - 1;
    let life = wrap(month_branch - hour as i64);
    let body = wrap(month_branch + hour as i64);
    let yin_stem = year_stem % 5 * 2 + 2;
    let stem_of = |branch: usize| (yin_stem + wrap(branch as i64 - YIN as i64)) % 10;
    let bureau = bureau_of(stem_of(life), life);

    // Stars, by branch.
    let mut placed: Vec<(usize, &'static str, &'static str)> = Vec::with_capacity(28);
    let zi_wei_at = zi_wei_branch(bureau.number, lunar.day) as i64;
    for (star, offset) in ZI_WEI_GROUP {
        placed.push((wrap(zi_wei_at - offset as i64), star, "major"));
    }
    let tian_fu_at = 4 - zi_wei_at;
    for (star, offset) in TIAN_FU_GROUP {
        placed.push((wrap(tian_fu_at + offset as i64), star, "major"));
    }
    let (m, h) = (i64::from(month) - 1, hour as i64);
    let (kui, yue) = match year_stem {
        0 | 4 | 6 => (CHOU, WEI),
        1 | 5 => (ZI, SHEN),
        2 | 3 => (HAI, YOU),
        7 => (WU, YIN),
        _ => (MAO, SI),
    };
    let lu_cun = [YIN, MAO, SI, WU, SI, WU, SHEN, YOU, HAI, ZI][year_stem] as i64;
    let trine = year_branch % 4; // 申子辰, 巳酉丑, 寅午戌, 亥卯未
    let (horse, fire, bell, small_start) = match trine {
        0 => (YIN, YIN, XU, XU),      // 申子辰
        1 => (HAI, MAO, XU, WEI),     // 巳酉丑
        2 => (SHEN, CHOU, MAO, CHEN), // 寅午戌
        _ => (SI, YOU, XU, CHOU),     // 亥卯未
    };
    for (branch, star, kind) in [
        (wrap(CHEN as i64 + m), ZUO_FU, "auxiliary"),
        (wrap(XU as i64 - m), YOU_BI, "auxiliary"),
        (wrap(XU as i64 - h), WEN_CHANG, "auxiliary"),
        (wrap(CHEN as i64 + h), WEN_QU, "auxiliary"),
        (kui, TIAN_KUI, "auxiliary"),
        (yue, TIAN_YUE, "auxiliary"),
        (wrap(lu_cun), LU_CUN, "auxiliary"),
        (horse, TIAN_MA, "auxiliary"),
        (wrap(lu_cun + 1), QING_YANG, "malefic"),
        (wrap(lu_cun - 1), TUO_LUO, "malefic"),
        (wrap(fire as i64 + h), HUO_XING, "malefic"),
        (wrap(bell as i64 + h), LING_XING, "malefic"),
        (wrap(HAI as i64 - h), DI_KONG, "malefic"),
        (wrap(HAI as i64 + h), DI_JIE, "malefic"),
    ] {
        placed.push((branch, star, kind));
    }

    let transformed = TRANSFORMATIONS[year_stem];
    let transformation_of = |star: &str| {
        transformed
            .iter()
            .position(|s| *s == star)
            .map(|k| TRANSFORMATION_KINDS[k])
    };

    // Limits.
    let forward = male.map(|male| male == (year_stem % 2 == 0));
    let palace_name = |branch: usize| PALACES[wrap(life as i64 - branch as i64)];

    let palaces = (0..12)
        .map(|i| {
            let branch = wrap(life as i64 - i);
            let stars = placed
                .iter()
                .filter(|(b, ..)| *b == branch)
                .map(|&(_, star, kind)| ZiWeiStar {
                    star,
                    kind,
                    brightness: brightness(star, branch),
                    transformation: transformation_of(star),
                })
                .collect();
            let decade = forward.map(|forward| {
                let steps = if forward {
                    wrap(branch as i64 - life as i64)
                } else {
                    wrap(life as i64 - branch as i64)
                } as u8;
                let start = bureau.number + 10 * steps;
                AgeRange {
                    start,
                    end: start + 9,
                }
            });
            let small_limit_ages = male
                .map(|male| {
                    let steps = if male {
                        wrap(branch as i64 - small_start as i64)
                    } else {
                        wrap(small_start as i64 - branch as i64)
                    } as u8;
                    (0..10).map(|j| steps + 1 + 12 * j).collect()
                })
                .unwrap_or_default();
            ZiWeiPalace {
                name: PALACES[i as usize],
                branch: BRANCHES[branch],
                stem: STEMS[stem_of(branch)],
                is_body: branch == body,
                stars,
                decade,
                small_limit_ages,
            }
        })
        .collect();

    let transformations = transformed
        .iter()
        .zip(TRANSFORMATION_KINDS)
        .filter_map(|(&star, kind)| {
            let &(branch, ..) = placed.iter().find(|(_, s, _)| *s == star)?;
            Some(ZiWeiTransformation {
                kind,
                star,
                palace: palace_name(branch),
            })
        })
        .collect();

    Ok(ZiWei {
        time_basis: pillars.time_basis,
        zi_hour: pillars.zi_hour,
        lunar_date: lunar,
        month,
        hour_branch: BRANCHES[hour],
        life_palace: BRANCHES[life],
        body_palace: BRANCHES[body],
        bureau,
        life_master: LIFE_MASTERS[life],
        body_master: BODY_MASTERS[year_branch],
        palaces,
        transformations,
        decade_direction: forward.map(|f| if f { "forward" } else { "backward" }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zi_wei_from_the_bureau_and_the_day() {
        // The examples of the placement rhyme: day 27, wood 3 → 戌; day 13, fire 6 → 亥;
        // day 6, earth 5 → 未; and the first two days of the water bureau.
        assert_eq!(BRANCHES[zi_wei_branch(3, 27)], "xu");
        assert_eq!(BRANCHES[zi_wei_branch(6, 13)], "hai");
        assert_eq!(BRANCHES[zi_wei_branch(5, 6)], "wei");
        assert_eq!(BRANCHES[zi_wei_branch(2, 1)], "chou");
        assert_eq!(BRANCHES[zi_wei_branch(2, 2)], "yin");
        assert_eq!(BRANCHES[zi_wei_branch(6, 1)], "you");
    }

    #[test]
    fn the_bureau_is_the_nayin_of_the_life_palace() {
        // 癸未 is willow wood: the wood bureau, 3.
        assert_eq!(
            bureau_of(9, 7),
            Bureau {
                element: "wood",
                number: 3
            }
        );
        // 甲子 is sea metal: 4.
        assert_eq!(bureau_of(0, 0).number, 4);
        // 丙子 is stream water: 2.
        assert_eq!(bureau_of(2, 0).number, 2);
    }
}
