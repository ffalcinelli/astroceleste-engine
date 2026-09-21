//! A set of JPL kernels in preference order, and barycentric states of bodies.

use crate::constants::{AU_KM, DAY_S, T0};
use crate::frames::Vec3;
use crate::time::Time;

use super::spk::{Segment, Spk, SpkError};

/// NAIF code of the Solar System Barycenter.
pub const SSB: i32 = 0;

/// One loaded kernel and the Julian-date span that *all* its segments cover (a chart
/// needs every body, so the usable span is their intersection).
pub struct Kernel {
    pub name: String,
    spk: Spk,
    pub start_jd: f64,
    pub end_jd: f64,
}

impl Kernel {
    pub fn new(name: impl Into<String>, spk: Spk) -> Result<Self, SpkError> {
        let segments = spk.segments();
        if segments.is_empty() {
            return Err(SpkError::Format("kernel has no segments".into()));
        }
        let start_jd = segments
            .iter()
            .map(|s| s.start_et / DAY_S + T0)
            .fold(f64::NEG_INFINITY, f64::max);
        let end_jd = segments
            .iter()
            .map(|s| s.end_et / DAY_S + T0)
            .fold(f64::INFINITY, f64::min);
        Ok(Kernel {
            name: name.into(),
            spk,
            start_jd,
            end_jd,
        })
    }

    pub fn covers(&self, jd: f64) -> bool {
        self.start_jd <= jd && jd <= self.end_jd
    }

    /// Whether the kernel has a segment for this body (Skyfield `code in ephemeris`).
    pub fn contains(&self, code: i32) -> bool {
        self.spk
            .segments()
            .iter()
            .any(|s| s.target == code || s.center == code)
    }

    fn segment_for(&self, target: i32) -> Option<&Segment> {
        self.spk.segments().iter().find(|s| s.target == target)
    }

    /// Position (au) and velocity (au/day) of `code` relative to the Solar System
    /// Barycenter, summing the chain of segments outward from the SSB like Skyfield's
    /// `VectorSum`.
    pub fn barycentric(&self, code: i32, t: &Time) -> Result<(Vec3, Vec3), SpkError> {
        let mut chain = Vec::new();
        let mut current = code;
        while current != SSB {
            let segment = self.segment_for(current).ok_or(SpkError::NoSegment {
                target: current,
                center: SSB,
            })?;
            chain.push(segment);
            current = segment.center;
            if chain.len() > 8 {
                return Err(SpkError::Format(format!("segment chain for {code} loops")));
            }
        }
        let mut position = [0.0; 3];
        let mut velocity = [0.0; 3];
        for segment in chain.iter().rev() {
            let (p, v) = self
                .spk
                .segment_state_split(segment, t.whole, t.tdb_fraction)?;
            for k in 0..3 {
                position[k] += p[k] / AU_KM;
                velocity[k] += v[k] * DAY_S / AU_KM;
            }
        }
        Ok((position, velocity))
    }
}

/// Kernels in preference order: a date is computed with the first kernel covering it.
#[derive(Default)]
pub struct KernelSet {
    kernels: Vec<Kernel>,
}

impl KernelSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, kernel: Kernel) {
        self.kernels.push(kernel);
    }

    pub fn kernels(&self) -> &[Kernel] {
        &self.kernels
    }

    pub fn is_empty(&self) -> bool {
        self.kernels.is_empty()
    }

    /// The preferred kernel covering this Julian date.
    pub fn for_jd(&self, jd: f64) -> Option<&Kernel> {
        self.kernels.iter().find(|k| k.covers(jd))
    }

    /// Combined span of every loaded kernel, as Julian dates.
    pub fn coverage(&self) -> Option<(f64, f64)> {
        if self.kernels.is_empty() {
            return None;
        }
        let start = self
            .kernels
            .iter()
            .map(|k| k.start_jd)
            .fold(f64::INFINITY, f64::min);
        let end = self
            .kernels
            .iter()
            .map(|k| k.end_jd)
            .fold(f64::NEG_INFINITY, f64::max);
        Some((start, end))
    }
}
