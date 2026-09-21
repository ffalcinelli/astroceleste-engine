//! Cross-checks the SPK reader against jplephem (see scripts/make_spk_fixtures.py).

use std::path::PathBuf;

use astroceleste_engine::ephemeris::{jd_tdb_to_et, Spk, SpkError};
use serde_json::Value;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference() -> Value {
    let text = std::fs::read_to_string(workspace_root().join("tests/fixtures/spk_reference.json"))
        .expect("run scripts/make_spk_fixtures.py");
    serde_json::from_str(&text).unwrap()
}

fn check_rows(spk: &Spk, rows: &[Value]) {
    assert!(!rows.is_empty());
    for row in rows {
        let target = row["target"].as_i64().unwrap() as i32;
        let center = row["center"].as_i64().unwrap() as i32;
        let et = jd_tdb_to_et(row["jd_tdb"].as_f64().unwrap());
        let (pos, vel) = spk.state(target, center, et).unwrap();
        for axis in 0..3 {
            let want_p = row["position_km"][axis].as_f64().unwrap();
            let want_v = row["velocity_km_per_day"][axis].as_f64().unwrap();
            let got_v = vel[axis] * 86_400.0;
            // Both sides sum the same series; only float summation order differs.
            assert!(
                (pos[axis] - want_p).abs() <= 1e-6 + want_p.abs() * 1e-13,
                "{center}->{target} axis {axis}: position {} vs {want_p}",
                pos[axis]
            );
            assert!(
                (got_v - want_v).abs() <= 1e-6 + want_v.abs() * 1e-12,
                "{center}->{target} axis {axis}: velocity {got_v} vs {want_v}"
            );
        }
    }
}

#[test]
fn excerpt_matches_jplephem() {
    let bytes = std::fs::read(workspace_root().join("tests/data/de440s_2000.bsp")).unwrap();
    let spk = Spk::from_bytes(bytes).unwrap();
    assert_eq!(spk.segments().len(), 14);
    check_rows(&spk, reference()["excerpt"].as_array().unwrap());
}

#[test]
fn file_and_memory_storage_agree() {
    let path = workspace_root().join("tests/data/de440s_2000.bsp");
    let from_file = Spk::open(&path).unwrap();
    let from_memory = Spk::from_bytes(std::fs::read(&path).unwrap()).unwrap();
    let et = jd_tdb_to_et(2_451_700.25);
    assert_eq!(from_file.state(301, 3, et), from_memory.state(301, 3, et));
}

#[test]
fn outside_coverage_is_an_error() {
    let spk = Spk::open(workspace_root().join("tests/data/de440s_2000.bsp")).unwrap();
    let et = jd_tdb_to_et(2_460_000.5);
    assert!(matches!(
        spk.state(4, 0, et),
        Err(SpkError::OutOfRange { .. })
    ));
    assert!(matches!(
        spk.state(499, 4, 0.0),
        Err(SpkError::NoSegment { .. })
    ));
}

/// Needs the full kernel: `scripts/fetch-kernels.sh` (skipped otherwise).
#[test]
fn full_kernel_matches_jplephem() {
    let path = workspace_root().join("kernels/de440s.bsp");
    if !path.exists() {
        eprintln!("skipping: {} not found", path.display());
        return;
    }
    let spk = Spk::open(path).unwrap();
    check_rows(&spk, reference()["full"].as_array().unwrap());
}
