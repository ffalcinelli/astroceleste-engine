//! Python bindings: `import astroceleste_engine`.
//!
//! Charts come back as plain dicts with the same JSON shape as the Rust `Serialize`
//! output. Values cross the boundary as JSON text, which keeps floats exact (both sides
//! print and parse shortest round-trip representations). Calculations release the GIL.

use std::path::PathBuf;

use astroceleste_engine::ephemeris::{Kernel, KernelSet, Spk};
use astroceleste_engine::{
    calculate_chart, calculate_derived_chart, calculate_election_chart, calculate_horary_chart,
    calculate_synastry, calculate_transit_chart, search_elections, ChartRequest, ElectionCriteria,
    EngineError as CoreError, UtcInstant,
};
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyString};
use serde::Serialize;
use serde_json::Value;

create_exception!(
    astroceleste_engine,
    EngineError,
    PyException,
    "A calculation failed (malformed kernel, unsupported data)."
);
create_exception!(
    astroceleste_engine,
    EphemerisRangeError,
    EngineError,
    "No loaded kernel covers the requested date."
);

fn to_py_err(err: CoreError) -> PyErr {
    match &err {
        CoreError::OutOfRange { .. } => EphemerisRangeError::new_err(err.to_string()),
        CoreError::InvalidInput(msg) => PyValueError::new_err(msg.clone()),
        _ => EngineError::new_err(err.to_string()),
    }
}

/// Python object → JSON value, through the standard `json` module.
fn to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    let json = obj.py().import("json")?;
    let text: String = json.call_method1("dumps", (obj,))?.extract()?;
    serde_json::from_str(&text).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Serializable → Python object (dicts, lists, floats, …).
fn to_py<T: Serialize>(py: Python<'_>, value: &T) -> PyResult<Py<PyAny>> {
    let text = serde_json::to_string(value).map_err(|e| EngineError::new_err(e.to_string()))?;
    let json = py.import("json")?;
    Ok(json.call_method1("loads", (text,))?.unbind())
}

/// A UTC instant from an ISO 8601 string or a `datetime` (naive means UTC).
fn to_instant(moment: &Bound<'_, PyAny>) -> PyResult<UtcInstant> {
    if let Ok(text) = moment.cast::<PyString>() {
        return UtcInstant::parse(text.to_str()?).map_err(|e| PyValueError::new_err(e.to_string()));
    }
    let py = moment.py();
    let dt = if moment.getattr("tzinfo")?.is_none() {
        moment.clone()
    } else {
        let utc = py.import("datetime")?.getattr("timezone")?.getattr("utc")?;
        moment.call_method1("astimezone", (utc,))?
    };
    let field = |name: &str| -> PyResult<i64> { dt.getattr(name)?.extract() };
    Ok(UtcInstant::from_civil(
        field("year")? as i32,
        field("month")? as u32,
        field("day")? as u32,
        field("hour")? as u32,
        field("minute")? as u32,
        field("second")? as u32,
        field("microsecond")? as u32,
    ))
}

fn optional_value(obj: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Value>> {
    match obj {
        Some(o) if !o.is_none() => Ok(Some(to_value(o)?)),
        _ => Ok(None),
    }
}

/// Election criteria from a dict (None for the defaults).
fn to_criteria(obj: Option<&Bound<'_, PyAny>>) -> PyResult<ElectionCriteria> {
    match optional_value(obj)? {
        Some(value) => ElectionCriteria::from_value(&value).map_err(to_py_err),
        None => Ok(ElectionCriteria::default()),
    }
}

/// Everything a chart request needs, owned, so it can cross into a GIL-free section.
struct Request {
    instant: UtcInstant,
    latitude: f64,
    longitude: f64,
    house_system: String,
    zodiac_type: String,
    ayanamsa: String,
    orb_settings: Option<Value>,
}

impl Request {
    #[allow(clippy::too_many_arguments)]
    fn new(
        moment: &Bound<'_, PyAny>,
        latitude: f64,
        longitude: f64,
        house_system: &str,
        zodiac_type: &str,
        ayanamsa: &str,
        orb_settings: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Request {
            instant: to_instant(moment)?,
            latitude,
            longitude,
            house_system: house_system.to_string(),
            zodiac_type: zodiac_type.to_string(),
            ayanamsa: ayanamsa.to_string(),
            orb_settings: optional_value(orb_settings)?,
        })
    }

    fn as_chart_request(&self) -> ChartRequest<'_> {
        ChartRequest {
            instant: self.instant,
            latitude: self.latitude,
            longitude: self.longitude,
            house_system: &self.house_system,
            zodiac_type: &self.zodiac_type,
            ayanamsa: &self.ayanamsa,
            orb_settings: self.orb_settings.as_ref(),
        }
    }
}

/// Chart calculator over a set of JPL kernels.
///
/// `Engine(["de440s.bsp", "de441_part-1.bsp"])` loads the kernels in preference order:
/// each date is computed with the first kernel that covers it.
#[pyclass(frozen, module = "astroceleste_engine")]
struct Engine {
    kernels: KernelSet,
}

#[pymethods]
impl Engine {
    #[new]
    fn new(py: Python<'_>, kernels: Vec<PathBuf>) -> PyResult<Self> {
        let set = py.detach(|| {
            let mut set = KernelSet::new();
            for path in &kernels {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                let spk = Spk::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
                let kernel =
                    Kernel::new(name, spk).map_err(|e| format!("{}: {e}", path.display()))?;
                set.push(kernel);
            }
            Ok::<_, String>(set)
        });
        Ok(Engine {
            kernels: set.map_err(EngineError::new_err)?,
        })
    }

    /// Loaded kernels as (name, first JD, last JD), in preference order.
    #[getter]
    fn kernels(&self) -> Vec<(String, f64, f64)> {
        self.kernels
            .kernels()
            .iter()
            .map(|k| (k.name.clone(), k.start_jd, k.end_jd))
            .collect()
    }

    /// Combined (first JD, last JD) span of the loaded kernels, or None.
    #[getter]
    fn coverage(&self) -> Option<(f64, f64)> {
        self.kernels.coverage()
    }

    /// Whether a loaded kernel covers this UT Julian date.
    fn supports(&self, jd: f64) -> bool {
        self.kernels.for_jd(jd).is_some()
    }

    /// The chart of a moment and place, as the API's chart dict.
    #[pyo3(signature = (moment, latitude, longitude, house_system="P", zodiac_type="tropical", ayanamsa="galcent_0sag", orb_settings=None))]
    #[allow(clippy::too_many_arguments)]
    fn chart(
        &self,
        py: Python<'_>,
        moment: &Bound<'_, PyAny>,
        latitude: f64,
        longitude: f64,
        house_system: &str,
        zodiac_type: &str,
        ayanamsa: &str,
        orb_settings: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let req = Request::new(
            moment,
            latitude,
            longitude,
            house_system,
            zodiac_type,
            ayanamsa,
            orb_settings,
        )?;
        let chart = py
            .detach(|| calculate_chart(&self.kernels, &req.as_chart_request()))
            .map_err(to_py_err)?;
        to_py(py, &chart)
    }

    /// A chart with its horary analysis under the `horary_data` key.
    #[pyo3(signature = (moment, latitude, longitude, house_system="P", zodiac_type="tropical", ayanamsa="galcent_0sag", orb_settings=None))]
    #[allow(clippy::too_many_arguments)]
    fn horary(
        &self,
        py: Python<'_>,
        moment: &Bound<'_, PyAny>,
        latitude: f64,
        longitude: f64,
        house_system: &str,
        zodiac_type: &str,
        ayanamsa: &str,
        orb_settings: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let req = Request::new(
            moment,
            latitude,
            longitude,
            house_system,
            zodiac_type,
            ayanamsa,
            orb_settings,
        )?;
        let chart = py
            .detach(|| calculate_horary_chart(&self.kernels, &req.as_chart_request()))
            .map_err(to_py_err)?;
        to_py(py, &chart)
    }

    /// A chart with its electional assessment under the `election_data` key.
    #[pyo3(signature = (moment, latitude, longitude, criteria=None, house_system="P", zodiac_type="tropical", ayanamsa="galcent_0sag", orb_settings=None))]
    #[allow(clippy::too_many_arguments)]
    fn election(
        &self,
        py: Python<'_>,
        moment: &Bound<'_, PyAny>,
        latitude: f64,
        longitude: f64,
        criteria: Option<&Bound<'_, PyAny>>,
        house_system: &str,
        zodiac_type: &str,
        ayanamsa: &str,
        orb_settings: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let criteria = to_criteria(criteria)?;
        let req = Request::new(
            moment,
            latitude,
            longitude,
            house_system,
            zodiac_type,
            ayanamsa,
            orb_settings,
        )?;
        let chart = py
            .detach(|| calculate_election_chart(&self.kernels, &req.as_chart_request(), &criteria))
            .map_err(to_py_err)?;
        to_py(py, &chart)
    }

    /// The best electional windows from `start` to `end` at a place.
    #[pyo3(signature = (start, end, latitude, longitude, criteria=None, house_system="P", zodiac_type="tropical", ayanamsa="galcent_0sag"))]
    #[allow(clippy::too_many_arguments)]
    fn elections(
        &self,
        py: Python<'_>,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        latitude: f64,
        longitude: f64,
        criteria: Option<&Bound<'_, PyAny>>,
        house_system: &str,
        zodiac_type: &str,
        ayanamsa: &str,
    ) -> PyResult<Py<PyAny>> {
        let criteria = to_criteria(criteria)?;
        let end = to_instant(end)?;
        let req = Request::new(
            start,
            latitude,
            longitude,
            house_system,
            zodiac_type,
            ayanamsa,
            None,
        )?;
        let result = py
            .detach(|| search_elections(&self.kernels, &req.as_chart_request(), end, &criteria))
            .map_err(to_py_err)?;
        to_py(py, &result)
    }

    /// The sky at `moment` and place, with its cross-aspects to `natal_planets`.
    #[pyo3(signature = (natal_planets, moment, latitude, longitude, house_system="P", zodiac_type="tropical", ayanamsa="galcent_0sag", orb_settings=None))]
    #[allow(clippy::too_many_arguments)]
    fn transit(
        &self,
        py: Python<'_>,
        natal_planets: &Bound<'_, PyAny>,
        moment: &Bound<'_, PyAny>,
        latitude: f64,
        longitude: f64,
        house_system: &str,
        zodiac_type: &str,
        ayanamsa: &str,
        orb_settings: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let natal = to_value(natal_planets)?;
        let req = Request::new(
            moment,
            latitude,
            longitude,
            house_system,
            zodiac_type,
            ayanamsa,
            orb_settings,
        )?;
        let chart = py
            .detach(|| calculate_transit_chart(&self.kernels, &natal, &req.as_chart_request()))
            .map_err(to_py_err)?;
        to_py(py, &chart)
    }
}

/// Cross-aspects from chart B's planets (overlay) to chart A's (base).
#[pyfunction]
#[pyo3(signature = (chart_a_planets, chart_b_planets, orb_settings=None))]
fn synastry(
    py: Python<'_>,
    chart_a_planets: &Bound<'_, PyAny>,
    chart_b_planets: &Bound<'_, PyAny>,
    orb_settings: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let a = to_value(chart_a_planets)?;
    let b = to_value(chart_b_planets)?;
    let orbs = optional_value(orb_settings)?;
    let result = calculate_synastry(&a, &b, orbs.as_ref()).map_err(to_py_err)?;
    to_py(py, &result)
}

/// A derived (turned) chart from a chart dict: radix house `root_house` becomes house 1.
#[pyfunction]
#[pyo3(signature = (base_chart, root_house, custom_name=None))]
fn derived_chart(
    py: Python<'_>,
    base_chart: &Bound<'_, PyAny>,
    root_house: i64,
    custom_name: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let base = to_value(base_chart)?;
    let result = calculate_derived_chart(&base, root_house, custom_name).map_err(to_py_err)?;
    to_py(py, &result)
}

/// Julian day of a moment, as the chart calculation uses it.
#[pyfunction]
fn julian_day(moment: &Bound<'_, PyAny>) -> PyResult<f64> {
    Ok(to_instant(moment)?.julian_day())
}

#[pymodule(name = "astroceleste_engine")]
fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<Engine>()?;
    m.add_function(wrap_pyfunction!(synastry, m)?)?;
    m.add_function(wrap_pyfunction!(derived_chart, m)?)?;
    m.add_function(wrap_pyfunction!(julian_day, m)?)?;
    m.add("EngineError", m.py().get_type::<EngineError>())?;
    m.add(
        "EphemerisRangeError",
        m.py().get_type::<EphemerisRangeError>(),
    )?;
    Ok(())
}
