# Chinese astrology

The engine computes the Chinese calendar of a moment. From it, it casts the Four Pillars (Ba Zi, 八字) and the Purple Star chart (Zi Wei Dou Shu, 紫微斗數). Results carry stable codes only:
- pinyin for stems, branches and solar terms;
- English for elements, animals, Ten Gods, NaYin and life stages.

The wording is left to the application.

The work is split by what needs the ephemeris:

| Step | Needs a kernel | Entry point |
|---|---|---|
| The calendar: solar terms, lunar months, equation of time | yes | `chinese_calendar`, or `chinese_calendar: true` on a chart request (the chart's `chinese_calendar` key) |
| The Four Pillars | no | `bazi` (Python `bazi`, JavaScript `bazi`) |
| The Purple Star chart | no | `zi_wei` (Python `zi_wei`, JavaScript `ziWei`) |

An app can therefore store the calendar with a chart, and re-cast both charts offline whenever the native's sex or a convention changes.

## The calendar

```python
chart = engine.chart("1940-11-27T15:12:00Z", 37.77, -122.42, chinese_calendar=True)
chart["chinese_calendar"]
# {"equation_of_time": 12.257…,
#  "solar_terms": [{"name": "lichun", "longitude": 315, "jie": True, "utc": "1940-02-04T23:07:32+00:00"}, …],
#  "lunar_months": [{"year": 1940, "month": 10, "leap": False, "start": "1940-10-31", "days": 29},
#                   {"year": 1940, "month": 11, "leap": False, "start": "1940-11-29", "days": 30}]}
```

- **`solar_terms`**: every solar term (節氣) from the last 立春 at or before the moment to the first jie term after it. A term is the moment the Sun's apparent longitude reaches a multiple of 15°.
  - The jie terms (`jie: true`, at 15° + 30°·k) open the solar months.
  - The others are the major terms (中氣).
- **`lunar_months`**: the months around the date of the moment, a few days either side. This is enough for any local clock.
  - `year` is the Gregorian year of the lunar year's New Year.
  - `leap` marks the intercalary month (閏月), which repeats the number before it.
- **`equation_of_time`**: apparent minus mean solar time, in minutes. It turns civil time into true solar time.

The calendar follows the rules of the modern calendar:
- **Terms and new moons:** true solar terms and true new moons, computed from the JPL ephemeris to well under a second.
- **Dates:** a month starts on the day of its new moon. Days are reckoned in China Standard Time (UTC+8) from 1929, and on the Beijing meridian (116°25′ E) before.
- **Month 11:** the month that holds the winter solstice.
- **Leap months:** when 13 months run from one month 11 to the next, the first of them without a major term is the leap month. This rule handles the 2033 case: an earlier month of 2033 has no major term, but the leap month is the eleventh.

These rules are applied to every date. Dates before 1645, when China still used mean solar terms, follow them proleptically, so the calendar may differ from the historical almanac by a month there.

It adds a few milliseconds to a chart.
The calendar needs about 14 months of ephemeris before the moment and 2 after. Near the edge of the loaded kernels it is left out (`None`), and the chart is still computed.

The tests check:
- Chinese New Year from 1900 to 2100;
- the leap months of 1984–2033;
- the equinoxes and solstices of 2000 against USNO times;
- the equation of time.

## The Four Pillars

```python
pillars = ace.bazi(chart["chinese_calendar"], "1940-11-27T15:12:00Z", -122.42, -480, sex="male")
[(pillars[p]["stem"], pillars[p]["branch"]) for p in ("year", "month", "day", "hour")]
# [("geng", "chen"), ("ding", "hai"), ("jia", "xu"), ("wu", "chen")]
```

```js
const pillars = bazi(chart.chinese_calendar, "1940-11-27T15:12:00Z", -122.42, -480, { sex: "male" });
```

The arguments are the calendar, the moment of birth (UTC), the longitude of the birthplace (degrees east), and the offset of civil time from UTC at birth (minutes, daylight saving included). The options are:

| Option | Meaning | Default |
|---|---|---|
| `solar_time` | cast the day and hour on true solar time at the birthplace (`true`) or on civil time (`false`) | `true` |
| `zi_hour` | `next_day`: the Zi hour (23:00–01:00) belongs to the next day; `split`: the day changes at midnight, and the late Zi hour keeps its day (its stem still comes from the next day's) | `next_day` |
| `sex` | `male` or `female`: sets the direction of the luck pillars, which are left out without it | none |
| `luck_pillars` | how many ten-year luck pillars to list | 10 |
| `year` | a year whose pillar (流年) to give as `annual`, seen from the Day Master | none |

### How the pillars are cast

- **Year:** changes at 立春 (about 4 February), not at the lunar New Year. 1984 is 甲子.
- **Month:** changes at each jie term. 寅 is the month from 立春. Its stem follows from the year's stem (五虎遁: 甲 and 己 years start from 丙寅).
- **Day:** the sexagenary count of days, unbroken since antiquity. 1949-10-01 is 甲子.
- **Hour:** two-hour branches from 子 (23:00–01:00). Its stem follows from the day's stem (五鼠遁: 甲 and 己 days start from 甲子).

The year and month compare instants, so the clock the birth was recorded in does not change them. The day and hour use the local clock:
- true solar time: UTC + longitude/15 hours + the equation of time;
- or civil time: UTC + the offset.

### The result

| Key | Content |
|---|---|
| `year`, `month`, `day`, `hour` | each pillar: `stem`, `branch`, `cycle` (0–59), `element` and `polarity` of the stem, `branch_element`, `animal`, `ten_god` (none for the Day Master), `hidden_stems`, `nayin`, `life_stage` |
| `day_master`, `day_master_element` | the stem of the day: the self |
| `elements` | weight of each element: each stem counts 1, and each branch 1 shared among its hidden stems |
| `time_basis`, `zi_hour`, `civil_time`, `solar_time` | the conventions used and both local times of birth |
| `month_term`, `next_term` | the jie term that opened the birth month and the next one |
| `lunar_date` | the birth date in the lunisolar calendar, with the animal of the lunar year (the popular "Chinese sign", which changes at the New Year) |
| `luck` | `direction`, `start_age` and the luck pillars (`start_age`, `start`, `end`, `pillar`) |
| `annual` | the pillar of the requested `year` (it changes at 立春), with its Ten God and hidden stems |

The tables are these:

- **Hidden stems (藏干)**: 子 癸; 丑 己癸辛; 寅 甲丙戊; 卯 乙; 辰 戊乙癸; 巳 丙庚戊; 午 丁己; 未 己丁乙; 申 庚壬戊; 酉 辛; 戌 戊辛丁; 亥 壬甲.
  - The main qi comes first.
  - The weights are 1, 0.7/0.3 or 0.6/0.3/0.1, by how many stems the branch holds.
- **Ten Gods (十神)**: the relation of a stem to the Day Master, by element and polarity:
  - same element: `friend` (same polarity), `rob_wealth` (opposite);
  - produced by it: `eating_god`, `hurting_officer`;
  - controlled by it: `indirect_wealth`, `direct_wealth`;
  - controlling it: `seven_killings`, `direct_officer`;
  - producing it: `indirect_resource`, `direct_resource`.
- **NaYin (納音)**: the element of each pair of the cycle, from `sea_metal` (甲子乙丑) to `ocean_water` (壬戌癸亥).
- **Life stages (十二長生)**: the Day Master's stage in each branch: `birth`, `bath`, `cap_and_belt`, `coming_of_age`, `prosperity`, `decline`, `sickness`, `death`, `tomb`, `extinction`, `conception`, `nurture`.
  - Yang stems are born at 甲亥, 丙戊寅, 庚巳 and 壬申, and go forward.
  - Yin stems are born at 乙午, 丁己酉, 辛子 and 癸卯, and go backward.
- **Luck pillars (大運)**:
  - **Direction:** forward from the month pillar for a yang year and a man, or a yin year and a woman; backward otherwise.
  - **Start:** after the days from the birth to the next jie term (forward) or from the previous one (backward), three days to a year.
  - **Length:** each lasts ten tropical years.

## Zi Wei Dou Shu

```python
chart = ace.zi_wei(calendar, "1940-11-27T15:12:00Z", -122.42, -480, sex="male")
chart["life_palace"], chart["bureau"]    # ("wei", {"element": "wood", "number": 3})
```

```js
const chart = ziWei(calendar, "1940-11-27T15:12:00Z", -122.42, -480, { sex: "male" });
```

The arguments are those of `bazi`. The options are `solar_time`, `zi_hour` and `sex`, as for the pillars, plus one more:

| Option | Meaning | Default |
|---|---|---|
| `leap_month` | `split`: a birth after the 15th of a leap month counts in the next month; `same`: every day of a leap month counts in the month it repeats | `split` |
| `year` | a lunar year to cast the horoscope for (see below) | none |
| `date` | a date, `YYYY-MM-DD`, to cast the horoscope for down to its month and day; its lunar year replaces `year` | none |

The chart follows the classic rules of the *Zi Wei Dou Shu Quanshu*.

- **Date and hour:** the year is the lunar year, which changes at the New Year (not at 立春). The month and day are the lunar ones. The hour is the birth hour's branch, on the same clock and with the same Zi-hour convention as the pillars.
- **Palaces:**
  - The life palace (命宮) is counted from 寅 forward to the month, then back to the hour.
  - The body palace (身宮) is counted forward to the hour.
  - The twelve palaces run backward through the branches from the life palace: `life`, `siblings`, `spouse`, `children`, `wealth`, `health`, `travel`, `friends`, `career`, `property`, `fortune`, `parents`.
  - Their stems follow from the year stem (五虎遁).
- **Bureau (五行局):** the NaYin element of the life palace's stem and branch. Water is 2, wood 3, metal 4, earth 5, fire 6.
- **The fourteen major stars:**
  - Zi Wei is placed from the bureau and the lunar day.
  - Its group follows it backward: 天機 −1, 太陽 −3, 武曲 −4, 天同 −5, 廉貞 −8.
  - Tian Fu mirrors Zi Wei across the 寅–申 axis. Its group follows it forward: 太陰 +1, 貪狼 +2, 巨門 +3, 天相 +4, 天梁 +5, 七殺 +6, 破軍 +10.
- **Auxiliary stars:**
  - by the month: 左輔 (from 辰, forward) and 右弼 (from 戌, backward);
  - by the hour: 文昌 (from 戌, backward) and 文曲 (from 辰, forward);
  - by the year stem: 天魁 and 天鉞 (甲戊庚 丑未, 乙己 子申, 丙丁 亥酉, 辛 午寅, 壬癸 卯巳) and 祿存;
  - by the year branch: 天馬.
- **Malefic stars:**
  - 擎羊 and 陀羅, either side of 祿存;
  - 火星 and 鈴星, from the year branch's starting point, forward by the hour;
  - 地空 and 地劫, from 亥 by the hour (backward and forward).
- **Brightness** (`miao` 廟, `wang` 旺, `de` 得, `li` 利, `ping` 平, `bu` 不, `xian` 陷): given for the major stars, 文昌, 文曲, 火星, 鈴星, 擎羊 and 陀羅. It follows the common table, as published by the [iztro](https://github.com/SylarLong/iztro) project (MIT, see NOTICE).
- **The Four Transformations (四化)** are those of the year stem, in the order 祿, 權, 科, 忌. Where schools differ:
  - 庚 gives 太陽, 武曲, 太陰, 天同;
  - 壬 gives 天梁, 紫微, 左輔, 武曲.
- **Life and body masters (命主, 身主):** by the life palace's branch and by the year branch.
- **Decade limits (大限):** given the native's sex.
  - They start in the life palace at the bureau's number of years (nominal age, 虛歲) and last ten years each.
  - They run forward through the branches for a yang year and a man or a yin year and a woman, backward otherwise.
- **Small limits (小限):** each palace lists the nominal ages up to 120 whose limit falls there. They start from 辰, 戌, 未 or 丑 (for 寅午戌, 申子辰, 巳酉丑 and 亥卯未 years), forward for a man and backward for a woman.

- **Minor stars (雜曜):** the 38 of the common school, in each palace's `minor_stars`:
  - by the year branch: 紅鸞 and 天喜, 咸池, 華蓋, 孤辰, 寡宿, 蜚廉, 破碎, 龍池, 鳳閣, 天哭, 天虛, 天德, 月德, 天空, 年解;
  - by the year stem: 天官, 天福 (`tian_fu_blessing`), 天廚, 截路, 空亡; by both: 旬空;
  - by the month: 天姚, 天刑, 解神, 陰煞, 天月 (`tian_yue_moon`), 天巫;
  - by the day, from 左輔, 右弼, 文昌 and 文曲: 三台, 八座, 恩光, 天貴;
  - by the hour: 台輔, 封誥;
  - from the life and body palaces: 天才, 天壽; in the friends and health palaces: 天傷, 天使.
- **Cycles of twelve gods** (each palace's `gods`):
  - 長生 (`life_stage`, the same codes as Ba Zi's), from 申, 亥, 巳, 申 or 寅 by the bureau;
  - 博士 (`boshi`), from 祿存;
  - both forward for a yang year and a man or a yin year and a woman, backward otherwise, so they need the sex;
  - 歲前 (`suiqian`), from the year's branch; 將前 (`jiangqian`), from the general star of the year's trine (子, 酉, 午 or 卯).
- **Flying transformations (飛化):** each palace's `flying` lists the four transformations of its own stem and the palaces of the stars they reach. One that lands in the same palace is a self-transformation (自化).

The result carries:
- `lunar_date`;
- `month`: the month the chart is cast with;
- `hour_branch`;
- `life_palace` and `body_palace` (branches);
- `bureau`;
- `life_master` and `body_master`;
- `palaces`: from the life palace, each with `name`, `branch`, `stem`, `is_body`, `stars` (`star`, `kind`, `brightness`, `transformation`), `decade` and `small_limit_ages`;
- `transformations` (`kind`, `star`, `palace`);
- `decade_direction`;
- `horoscope`, when a `year` is given.

### The horoscope of a year

With `year`, the chart carries `horoscope`:

- **`nominal_age`:** the year minus the lunar year of birth, plus one (虛歲, counted by calendar years).
- **`decade`:** the decade limit holding that age. Before the first decade starts, it is the childhood limit (童限, `childhood: true`): the life, wealth, health, spouse, fortune and career palaces for ages 1 to 6.
- **`small_limit`:** the branch of the small limit's palace.
- **`yearly`:** the year (流年), whose life palace is the year's branch.
- Each period (`decade`, `yearly`) gives:
  - its `branch` and `stem` (the decade palace's stem, or the year's);
  - `palaces`: its palace names by branch, life first;
  - `transformations`: the four of its stem, with the natal palace of each star;
  - `stars`: its moving stars 魁, 鉞, 昌, 曲, 祿, 羊, 陀, 馬, 鸞 and 喜. The moving 昌 and 曲 follow the stem (甲 巳/酉, 乙 午/申, 丙戊 申/午, 丁己 酉/巳, 庚 亥/卯, 辛 子/寅, 壬 寅/子, 癸 卯/亥).
- **`suiqian` and `jiangqian`:** the year's 歲前 and 將前 gods.
- **With a `date`:**
  - `date` and its `lunar_date`;
  - `monthly` (流月): month 1 is counted from the year's palace back to the birth month and forward to the birth hour (斗君), one palace a month. A date after the 15th of a leap month counts in the next month. Its stem and branch are the lunar month's (五虎遁 from the year's stem).
  - `daily` (流日): from the month's palace, one palace a day; its stem and branch are the day's.

  Dates are read in the lunar calendar from an embedded table of the lunar years 1900 to 2100, generated from the engine's own calendar and checked against it. So a date needs no kernel.

The decade, childhood and small limits need the native's sex.

The tests check:
- a published chart against the values iztro gives for it;
- the geometry of 300 charts: every star once (28 main and 38 minor), Tian Fu mirroring Zi Wei, Qi Sha opposite Tian Fu, the lambs either side of Lu Cun, each cycle of twelve gods in twelve palaces, and ages 1–120 each in exactly one small limit;
- the minor stars and the cycles of gods of a palace, and the horoscope of a year and of childhood;
- 120 charts and 80 horoscopes against [iztro](https://github.com/SylarLong/iztro), the reference implementation of the common school (`tests/ziwei_iztro.rs`). They match it in every palace, star, brightness, transformation, minor star, god, limit and period. The fixtures come from `scripts/make_ziwei_fixtures.cjs` and `scripts/make_ziwei_horoscope_fixtures.cjs`. The one known difference: iztro gives placeholder values for a date before the birth, where the engine gives no decade or small limit.
