//! Minimal reader for JPL SPK kernels (NAIF DAF container, segment types 2 and 3).
//!
//! Positions are returned in km and velocities in km/s, in the kernel's frame (ICRF/J2000
//! for the JPL planetary ephemerides), relative to the segment's center body.
//! Time is TDB seconds past J2000 ("ET"), see [`jd_tdb_to_et`].
//!
//! Format reference: NAIF "DAF Required Reading" and "SPK Required Reading".

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

const RECORD_BYTES: usize = 1024;
const WORD_BYTES: usize = 8;
const J2000_JD: f64 = 2_451_545.0;
const SECONDS_PER_DAY: f64 = 86_400.0;

/// TDB Julian day → TDB seconds past J2000.
pub fn jd_tdb_to_et(jd_tdb: f64) -> f64 {
    (jd_tdb - J2000_JD) * SECONDS_PER_DAY
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpkError {
    Io(String),
    Format(String),
    UnsupportedType(i32),
    /// No segment for this center → target pair covers the requested time.
    OutOfRange {
        target: i32,
        center: i32,
        et: f64,
    },
    /// The kernel has no segment at all for this center → target pair.
    NoSegment {
        target: i32,
        center: i32,
    },
}

impl fmt::Display for SpkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpkError::Io(msg) => write!(f, "SPK I/O error: {msg}"),
            SpkError::Format(msg) => write!(f, "malformed SPK file: {msg}"),
            SpkError::UnsupportedType(t) => write!(f, "unsupported SPK segment type {t}"),
            SpkError::OutOfRange { target, center, et } => write!(
                f,
                "no SPK segment for {center} -> {target} covers ET {et:.1} s"
            ),
            SpkError::NoSegment { target, center } => {
                write!(f, "kernel has no segment for {center} -> {target}")
            }
        }
    }
}

impl std::error::Error for SpkError {}

impl From<std::io::Error> for SpkError {
    fn from(err: std::io::Error) -> Self {
        SpkError::Io(err.to_string())
    }
}

/// Where the kernel bytes live: fully in memory (WASM, mobile, small kernels) or read on
/// demand from disk (multi-GB kernels such as de441 on the server).
enum Storage {
    Memory(Vec<u8>),
    File(Mutex<File>),
}

impl Storage {
    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> Result<(), SpkError> {
        match self {
            Storage::Memory(data) => {
                let end = offset
                    .checked_add(buf.len())
                    .filter(|end| *end <= data.len())
                    .ok_or_else(|| SpkError::Format("read past end of file".into()))?;
                buf.copy_from_slice(&data[offset..end]);
                Ok(())
            }
            Storage::File(file) => {
                let mut file = file
                    .lock()
                    .map_err(|_| SpkError::Io("poisoned lock".into()))?;
                file.seek(SeekFrom::Start(offset as u64))?;
                file.read_exact(buf)?;
                Ok(())
            }
        }
    }
}

/// One SPK segment: a center → target trajectory over a time span.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub center: i32,
    pub target: i32,
    pub frame: i32,
    pub data_type: i32,
    pub start_et: f64,
    pub end_et: f64,
    /// 1-based DAF word addresses of the segment data.
    start_word: usize,
    // Chebyshev directory (types 2 and 3), read from the segment's last four words.
    init: f64,
    interval: f64,
    record_size: usize,
    record_count: usize,
}

impl Segment {
    pub fn covers(&self, et: f64) -> bool {
        et >= self.start_et && et <= self.end_et
    }

    fn components(&self) -> usize {
        if self.data_type == 3 {
            6
        } else {
            3
        }
    }
}

pub struct Spk {
    storage: Storage,
    little_endian: bool,
    segments: Vec<Segment>,
}

impl fmt::Debug for Spk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spk")
            .field("segments", &self.segments)
            .finish()
    }
}

/// Position (km) and velocity (km/s).
pub type State = ([f64; 3], [f64; 3]);

impl Spk {
    /// Load a kernel held in memory (e.g. fetched by a browser or bundled in an app).
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, SpkError> {
        Self::load(Storage::Memory(data))
    }

    /// Open a kernel on disk; segment data is read on demand, so size is not a concern.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SpkError> {
        Self::load(Storage::File(Mutex::new(File::open(path)?)))
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    fn load(storage: Storage) -> Result<Self, SpkError> {
        let mut header = [0u8; RECORD_BYTES];
        storage.read_bytes(0, &mut header)?;
        if &header[0..7] != b"DAF/SPK" {
            return Err(SpkError::Format("not a DAF/SPK file".into()));
        }
        let little_endian = match &header[88..96] {
            b"LTL-IEEE" => true,
            b"BIG-IEEE" => false,
            // Pre-1995 files lack LOCFMT: infer from ND, which is always 2 for SPK.
            _ => i32::from_le_bytes(header[8..12].try_into().unwrap()) == 2,
        };
        let mut spk = Spk {
            storage,
            little_endian,
            segments: Vec::new(),
        };
        let nd = spk.int_at(&header, 8) as usize;
        let ni = spk.int_at(&header, 12) as usize;
        if nd != 2 || ni != 6 {
            return Err(SpkError::Format(format!("unexpected ND={nd} NI={ni}")));
        }
        let summary_words = nd + ni.div_ceil(2);

        let mut record_number = spk.int_at(&header, 76) as usize;
        let mut visited = 0;
        while record_number != 0 {
            visited += 1;
            if visited > 100_000 {
                return Err(SpkError::Format("summary record chain does not end".into()));
            }
            let mut record = [0u8; RECORD_BYTES];
            spk.storage
                .read_bytes((record_number - 1) * RECORD_BYTES, &mut record)?;
            let next = spk.double_at(&record, 0) as usize;
            let count = spk.double_at(&record, 16) as usize;
            for i in 0..count {
                let base = 24 + i * summary_words * WORD_BYTES;
                if base + summary_words * WORD_BYTES > RECORD_BYTES {
                    return Err(SpkError::Format("summary overflows its record".into()));
                }
                let segment = spk.read_segment(&record, base)?;
                spk.segments.push(segment);
            }
            record_number = next;
        }
        Ok(spk)
    }

    fn read_segment(&self, record: &[u8], base: usize) -> Result<Segment, SpkError> {
        let start_et = self.double_at(record, base);
        let end_et = self.double_at(record, base + 8);
        let ints = base + 16;
        let target = self.int_at(record, ints);
        let center = self.int_at(record, ints + 4);
        let frame = self.int_at(record, ints + 8);
        let data_type = self.int_at(record, ints + 12);
        let start_word = self.int_at(record, ints + 16) as usize;
        let end_word = self.int_at(record, ints + 20) as usize;

        let mut segment = Segment {
            center,
            target,
            frame,
            data_type,
            start_et,
            end_et,
            start_word,
            init: 0.0,
            interval: 0.0,
            record_size: 0,
            record_count: 0,
        };
        if data_type == 2 || data_type == 3 {
            if end_word < start_word.max(4) {
                return Err(SpkError::Format(format!(
                    "bad word range for {center} -> {target}"
                )));
            }
            let dir = self.read_words(end_word - 3, 4)?;
            segment.init = dir[0];
            segment.interval = dir[1];
            segment.record_size = dir[2] as usize;
            segment.record_count = dir[3] as usize;
            let coefficients = segment.record_size.saturating_sub(2);
            if segment.interval <= 0.0
                || segment.record_count == 0
                || coefficients == 0
                || coefficients % segment.components() != 0
            {
                return Err(SpkError::Format(format!(
                    "bad Chebyshev directory for {center} -> {target}"
                )));
            }
        }
        Ok(segment)
    }

    /// State of `target` relative to `center` from the single segment that covers `et`.
    pub fn state(&self, target: i32, center: i32, et: f64) -> Result<State, SpkError> {
        let mut found_pair = false;
        for segment in &self.segments {
            if segment.target != target || segment.center != center {
                continue;
            }
            found_pair = true;
            if segment.covers(et) {
                return self.segment_state(segment, et);
            }
        }
        if found_pair {
            Err(SpkError::OutOfRange { target, center, et })
        } else {
            Err(SpkError::NoSegment { target, center })
        }
    }

    /// Evaluate one segment at `et` (which must lie inside it).
    pub fn segment_state(&self, segment: &Segment, et: f64) -> Result<State, SpkError> {
        if !segment.covers(et) {
            return Err(SpkError::OutOfRange {
                target: segment.target,
                center: segment.center,
                et,
            });
        }
        if segment.data_type != 2 && segment.data_type != 3 {
            return Err(SpkError::UnsupportedType(segment.data_type));
        }
        let index = (((et - segment.init) / segment.interval).floor().max(0.0) as usize)
            .min(segment.record_count - 1);
        let words = self.read_words(
            segment.start_word + index * segment.record_size,
            segment.record_size,
        )?;
        let (mid, radius) = (words[0], words[1]);
        let n = (segment.record_size - 2) / segment.components();
        let coeffs = &words[2..];
        let s = (et - mid) / radius;

        let mut position = [0.0; 3];
        let mut velocity = [0.0; 3];
        for axis in 0..3 {
            let c = &coeffs[axis * n..(axis + 1) * n];
            let (value, derivative) = chebyshev(c, s);
            position[axis] = value;
            velocity[axis] = derivative / radius;
        }
        if segment.data_type == 3 {
            // Type 3 stores velocity as its own series.
            for (axis, v) in velocity.iter_mut().enumerate() {
                let c = &coeffs[(axis + 3) * n..(axis + 4) * n];
                *v = chebyshev(c, s).0;
            }
        }
        Ok((position, velocity))
    }

    fn read_words(&self, first_word: usize, count: usize) -> Result<Vec<f64>, SpkError> {
        if first_word == 0 {
            return Err(SpkError::Format("word address 0".into()));
        }
        let mut buf = vec![0u8; count * WORD_BYTES];
        self.storage
            .read_bytes((first_word - 1) * WORD_BYTES, &mut buf)?;
        Ok(buf
            .chunks_exact(WORD_BYTES)
            .map(|b| self.to_f64(b.try_into().unwrap()))
            .collect())
    }

    fn double_at(&self, bytes: &[u8], offset: usize) -> f64 {
        self.to_f64(bytes[offset..offset + 8].try_into().unwrap())
    }

    fn int_at(&self, bytes: &[u8], offset: usize) -> i32 {
        let raw: [u8; 4] = bytes[offset..offset + 4].try_into().unwrap();
        if self.little_endian {
            i32::from_le_bytes(raw)
        } else {
            i32::from_be_bytes(raw)
        }
    }

    fn to_f64(&self, raw: [u8; 8]) -> f64 {
        if self.little_endian {
            f64::from_le_bytes(raw)
        } else {
            f64::from_be_bytes(raw)
        }
    }
}

/// Chebyshev series value and derivative with respect to `s` at `s` in [-1, 1].
fn chebyshev(coeffs: &[f64], s: f64) -> (f64, f64) {
    let (mut t_prev, mut t) = (1.0, s);
    let (mut d_prev, mut d) = (0.0, 1.0);
    let mut value = coeffs[0];
    let mut derivative = 0.0;
    if coeffs.len() > 1 {
        value += coeffs[1] * s;
        derivative += coeffs[1];
    }
    for &c in &coeffs[2.min(coeffs.len())..] {
        let t_next = 2.0 * s * t - t_prev;
        let d_next = 2.0 * t + 2.0 * s * d - d_prev;
        value += c * t_next;
        derivative += c * d_next;
        (t_prev, t) = (t, t_next);
        (d_prev, d) = (d, d_next);
    }
    (value, derivative)
}

#[cfg(test)]
mod tests {
    use super::chebyshev;

    #[test]
    fn chebyshev_matches_closed_form() {
        // 1 + 2 T1 + 3 T2 + 4 T3 with T2 = 2s²-1, T3 = 4s³-3s.
        let s: f64 = 0.37;
        let expected =
            1.0 + 2.0 * s + 3.0 * (2.0 * s * s - 1.0) + 4.0 * (4.0 * s.powi(3) - 3.0 * s);
        let expected_d = 2.0 + 12.0 * s + 4.0 * (12.0 * s * s - 3.0);
        let (v, d) = chebyshev(&[1.0, 2.0, 3.0, 4.0], s);
        assert!((v - expected).abs() < 1e-12);
        assert!((d - expected_d).abs() < 1e-12);
    }
}
