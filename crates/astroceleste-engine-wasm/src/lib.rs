//! WebAssembly bindings (npm package `astroceleste-engine`).
//!
//! ```js
//! import init, { Engine, synastry, derivedChart } from "astroceleste-engine";
//! await init();
//! const engine = new Engine();
//! engine.addKernel("de440s.bsp", new Uint8Array(await (await fetch(url)).arrayBuffer()));
//! const chart = engine.chart({ utc: "1987-05-17T14:30:00Z", latitude: 41.9, longitude: 12.5 });
//! ```
//!
//! Requests and results are plain objects in the API's JSON shape (snake_case keys).
//! Failures throw an `Error` whose `code` is the API error code, e.g.
//! `"ephemeris_out_of_range"`.

use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
use astroceleste_engine::{
    calculate_chart, calculate_derived_chart, calculate_election_chart, calculate_horary_chart,
    calculate_synastry, calculate_transit_chart, degree_qualities as core_degree_qualities,
    degree_quality_table as core_degree_quality_table, search_elections, ChartRequest,
    ElectionCriteria, EngineError, UtcInstant,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::prelude::*;

fn js_error(code: &str, message: &str) -> JsValue {
    let err = js_sys::Error::new(message);
    // Setting a property on a fresh Error object cannot fail.
    let _ = js_sys::Reflect::set(&err, &"code".into(), &code.into());
    err.into()
}

fn engine_error(err: EngineError) -> JsValue {
    js_error(err.code(), &err.to_string())
}

fn invalid(message: impl std::fmt::Display) -> JsValue {
    js_error("invalid_input", &message.to_string())
}

fn from_js<T: for<'de> Deserialize<'de>>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(invalid)
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    // Plain objects (not Maps) and numbers as numbers, like JSON.parse would give.
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    value
        .serialize(&serializer)
        .map_err(|e| js_error("internal_error", &e.to_string()))
}

fn default_house_system() -> String {
    "P".into()
}
fn default_zodiac() -> String {
    "tropical".into()
}
fn default_ayanamsa() -> String {
    "galcent_0sag".into()
}

/// `{ utc, latitude, longitude, house_system?, zodiac_type?, ayanamsa?, orb_settings? }`
#[derive(Deserialize)]
struct Request {
    utc: String,
    latitude: f64,
    longitude: f64,
    #[serde(default = "default_house_system")]
    house_system: String,
    #[serde(default = "default_zodiac")]
    zodiac_type: String,
    #[serde(default = "default_ayanamsa")]
    ayanamsa: String,
    #[serde(default)]
    orb_settings: Option<Value>,
}

impl Request {
    fn parse(value: JsValue) -> Result<(Self, UtcInstant), JsValue> {
        let req: Request = from_js(value)?;
        let instant = UtcInstant::parse(&req.utc).map_err(invalid)?;
        Ok((req, instant))
    }

    fn as_chart_request(&self, instant: UtcInstant) -> ChartRequest<'_> {
        ChartRequest {
            instant,
            latitude: self.latitude,
            longitude: self.longitude,
            house_system: &self.house_system,
            zodiac_type: &self.zodiac_type,
            ayanamsa: &self.ayanamsa,
            orb_settings: self.orb_settings.as_ref(),
        }
    }
}

/// Chart calculator over JPL kernels loaded from memory.
#[wasm_bindgen]
pub struct Engine {
    kernels: KernelSet,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Engine {
        Engine {
            kernels: KernelSet::new(),
        }
    }

    /// Add a JPL SPK kernel (e.g. de440s.bsp). Kernels are used in the order added: a
    /// date is computed with the first one covering it.
    #[wasm_bindgen(js_name = addKernel)]
    pub fn add_kernel(&mut self, name: &str, bytes: Vec<u8>) -> Result<(), JsValue> {
        let spk = Spk::from_bytes(bytes).map_err(|e| js_error("invalid_kernel", &e.to_string()))?;
        let kernel =
            Kernel::new(name, spk).map_err(|e| js_error("invalid_kernel", &e.to_string()))?;
        self.kernels.push(kernel);
        Ok(())
    }

    /// `[firstJD, lastJD]` covered by the loaded kernels, or `undefined`.
    #[wasm_bindgen(getter)]
    pub fn coverage(&self) -> Option<Vec<f64>> {
        self.kernels.coverage().map(|(a, b)| vec![a, b])
    }

    /// Whether a loaded kernel covers this UT Julian date.
    pub fn supports(&self, jd: f64) -> bool {
        self.kernels.for_jd(jd).is_some()
    }

    /// The chart of a moment and place.
    pub fn chart(&self, request: JsValue) -> Result<JsValue, JsValue> {
        let (req, instant) = Request::parse(request)?;
        let chart =
            calculate_chart(&self.kernels, &req.as_chart_request(instant)).map_err(engine_error)?;
        to_js(&chart)
    }

    /// A chart with its horary analysis under `horary_data`.
    pub fn horary(&self, request: JsValue) -> Result<JsValue, JsValue> {
        let (req, instant) = Request::parse(request)?;
        let chart = calculate_horary_chart(&self.kernels, &req.as_chart_request(instant))
            .map_err(engine_error)?;
        to_js(&chart)
    }

    /// A chart with its electional assessment under `election_data`. `criteria` may be
    /// `undefined` for the defaults.
    pub fn election(&self, request: JsValue, criteria: JsValue) -> Result<JsValue, JsValue> {
        let (req, instant) = Request::parse(request)?;
        let criteria = to_criteria(criteria)?;
        let chart =
            calculate_election_chart(&self.kernels, &req.as_chart_request(instant), &criteria)
                .map_err(engine_error)?;
        to_js(&chart)
    }

    /// The best electional windows from `request.utc` to `end` (ISO 8601 UTC, at most 92
    /// days later) at the request's place.
    pub fn elections(
        &self,
        request: JsValue,
        end: &str,
        criteria: JsValue,
    ) -> Result<JsValue, JsValue> {
        let (req, instant) = Request::parse(request)?;
        let end = UtcInstant::parse(end).map_err(invalid)?;
        let criteria = to_criteria(criteria)?;
        let result = search_elections(
            &self.kernels,
            &req.as_chart_request(instant),
            end,
            &criteria,
        )
        .map_err(engine_error)?;
        to_js(&result)
    }

    /// The sky of `request`, with its cross-aspects to `natalPlanets`.
    pub fn transit(&self, natal_planets: JsValue, request: JsValue) -> Result<JsValue, JsValue> {
        let natal: Value = from_js(natal_planets)?;
        let (req, instant) = Request::parse(request)?;
        let chart = calculate_transit_chart(&self.kernels, &natal, &req.as_chart_request(instant))
            .map_err(engine_error)?;
        to_js(&chart)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

fn optional(value: JsValue) -> Result<Option<Value>, JsValue> {
    if value.is_undefined() || value.is_null() {
        Ok(None)
    } else {
        from_js(value).map(Some)
    }
}

fn to_criteria(value: JsValue) -> Result<ElectionCriteria, JsValue> {
    match optional(value)? {
        Some(v) => ElectionCriteria::from_value(&v).map_err(engine_error),
        None => Ok(ElectionCriteria::default()),
    }
}

/// Cross-aspects from chart B's planets (overlay) to chart A's (base).
#[wasm_bindgen]
pub fn synastry(
    chart_a_planets: JsValue,
    chart_b_planets: JsValue,
    orb_settings: JsValue,
) -> Result<JsValue, JsValue> {
    let a: Value = from_js(chart_a_planets)?;
    let b: Value = from_js(chart_b_planets)?;
    let orbs = optional(orb_settings)?;
    to_js(&calculate_synastry(&a, &b, orbs.as_ref()).map_err(engine_error)?)
}

/// A derived (turned) chart: radix house `rootHouse` becomes house 1.
#[wasm_bindgen(js_name = derivedChart)]
pub fn derived_chart(
    base_chart: JsValue,
    root_house: i32,
    custom_name: Option<String>,
) -> Result<JsValue, JsValue> {
    let base: Value = from_js(base_chart)?;
    let chart = calculate_derived_chart(&base, root_house as i64, custom_name.as_deref())
        .map_err(engine_error)?;
    to_js(&chart)
}

/// Lilly's qualities of the degree an ecliptic longitude falls in.
#[wasm_bindgen(js_name = degreeQualities)]
pub fn degree_qualities(longitude: f64) -> Result<JsValue, JsValue> {
    to_js(&core_degree_qualities(longitude))
}

/// Lilly's table of the degree qualities, one object per sign from Aries.
#[wasm_bindgen(js_name = degreeQualityTable)]
pub fn degree_quality_table() -> Result<JsValue, JsValue> {
    to_js(core_degree_quality_table())
}

/// Julian day of an ISO 8601 UTC date-time, as the chart calculation uses it.
#[wasm_bindgen(js_name = julianDay)]
pub fn julian_day(utc: &str) -> Result<f64, JsValue> {
    Ok(UtcInstant::parse(utc).map_err(invalid)?.julian_day())
}

/// A smaller kernel covering TDB Julian dates `startJd`..`endJd`, with positions
/// identical to `kernel`'s inside that range (e.g. to ship 1900-2100 of de440s).
#[wasm_bindgen(js_name = excerptKernel)]
pub fn excerpt_kernel(kernel: Vec<u8>, start_jd: f64, end_jd: f64) -> Result<Vec<u8>, JsValue> {
    let spk = Spk::from_bytes(kernel).map_err(|e| js_error("invalid_kernel", &e.to_string()))?;
    spk.excerpt(start_jd, end_jd)
        .map_err(|e| js_error("invalid_kernel", &e.to_string()))
}
