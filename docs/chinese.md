# Chinese astrology

The engine computes the Chinese calendar of a moment, and casts the Four Pillars (Ba Zi, 八字) from it. Results carry stable codes only:
- pinyin for stems, branches and solar terms;
- English for elements, animals, Ten Gods, NaYin and life stages.

The wording is left to the application.

The work is split by what needs the ephemeris:

| Step | Needs a kernel | Entry point |
|---|---|---|
| The calendar: solar terms, lunar months, equation of time | yes | `chinese_calendar`, or `chinese_calendar: true` on a chart request (the chart's `chinese_calendar` key) |
| The Four Pillars | no | `bazi` (Python `bazi`, JavaScript `bazi`) |

An app can therefore store the calendar with a chart, and re-cast the pillars offline whenever the native's sex or a convention changes.

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
