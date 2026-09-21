use std::fmt;

use crate::ephemeris::SpkError;

#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    /// No loaded kernel covers the requested instant (`ephemeris_out_of_range`).
    OutOfRange {
        jd: f64,
        coverage: Option<(f64, f64)>,
    },
    Ephemeris(SpkError),
}

impl EngineError {
    /// Stable machine-readable code, shared with the HTTP API.
    pub fn code(&self) -> &'static str {
        match self {
            EngineError::OutOfRange { .. } => "ephemeris_out_of_range",
            EngineError::Ephemeris(_) => "ephemeris_error",
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::OutOfRange {
                jd,
                coverage: Some((start, end)),
            } => write!(
                f,
                "JD {jd:.1} is outside the loaded ephemeris coverage (JD {start:.1} to {end:.1})"
            ),
            EngineError::OutOfRange { jd, coverage: None } => {
                write!(f, "JD {jd:.1} cannot be computed: no ephemeris loaded")
            }
            EngineError::Ephemeris(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<SpkError> for EngineError {
    fn from(err: SpkError) -> Self {
        EngineError::Ephemeris(err)
    }
}
