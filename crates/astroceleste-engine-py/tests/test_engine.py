"""Python bindings: API behaviour, and the golden charts through the Python layer."""

import json
import math
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

import astroceleste_engine as ace

ROOT = Path(__file__).resolve().parents[3]
EXCERPT = ROOT / "tests" / "data" / "de440s_2000.bsp"
FULL = ROOT / "kernels" / "de440s.bsp"
FIXTURES = ROOT / "tests" / "fixtures"
PRIVATE_KEYS = {"degree_symbol", "degree_symbols"}

needs_full_kernel = pytest.mark.skipif(not FULL.exists(), reason="scripts/fetch-kernels.sh")


def strip_private(value):
    if isinstance(value, dict):
        return {k: strip_private(v) for k, v in value.items() if k not in PRIVATE_KEYS}
    if isinstance(value, list):
        return [strip_private(v) for v in value]
    return value


def assert_same(actual, expected, path="$", tol=1e-9):
    if isinstance(expected, dict):
        assert list(actual) == list(expected), f"{path}: keys"
        for key in expected:
            assert_same(actual[key], expected[key], f"{path}.{key}", tol)
    elif isinstance(expected, list):
        assert len(actual) == len(expected), f"{path}: length"
        for i, (a, e) in enumerate(zip(actual, expected, strict=True)):
            assert_same(a, e, f"{path}[{i}]", tol)
    elif isinstance(expected, float):
        assert isinstance(actual, float), f"{path}: {actual!r} is not a float"
        assert math.isclose(actual, expected, rel_tol=tol, abs_tol=tol), f"{path}: {actual} != {expected}"
    else:
        assert type(actual) is type(expected) and actual == expected, f"{path}: {actual!r} != {expected!r}"


@pytest.fixture(scope="module")
def excerpt():
    return ace.Engine([EXCERPT])


def test_engine_reports_its_kernels(excerpt):
    ((name, start, end),) = excerpt.kernels
    assert name == "de440s_2000.bsp"
    assert start < 2451545.0 + 30 < end
    assert excerpt.coverage == (start, end)
    assert excerpt.supports(2451600.0) and not excerpt.supports(2460000.0)


def test_datetime_string_and_offset_are_the_same_instant(excerpt):
    utc = datetime(2000, 6, 1, 12, 0, tzinfo=timezone.utc)
    rome = utc.astimezone(timezone(timedelta(hours=2)))
    a = excerpt.chart(utc, 41.9, 12.5)
    assert a == excerpt.chart(rome, 41.9, 12.5)
    assert a == excerpt.chart("2000-06-01T12:00:00Z", 41.9, 12.5)
    assert a == excerpt.chart(utc.replace(tzinfo=None), 41.9, 12.5)
    assert ace.julian_day(utc) == 2451697.0


def test_chart_shape(excerpt):
    chart = excerpt.chart("2000-06-01T12:00:00Z", 41.9, 12.5, zodiac_type="sidereal", ayanamsa="lahiri")
    assert chart["zodiac_type"] == "sidereal" and chart["ayanamsa"] == "lahiri"
    names = [p["name"] for p in chart["planets"]]
    assert names[:2] == ["Sun", "Moon"] and names[-2:] == ["Ascendant", "Midheaven"]
    assert len(chart["houses"]) == 12
    json.dumps(chart)  # plain JSON types only


def test_out_of_range_raises(excerpt):
    with pytest.raises(ace.EphemerisRangeError):
        excerpt.chart("2010-01-01T00:00:00Z", 41.9, 12.5)
    assert issubclass(ace.EphemerisRangeError, ace.EngineError)


def test_bad_input_raises_value_error(excerpt):
    with pytest.raises(ValueError):
        excerpt.chart("not a date", 41.9, 12.5)
    with pytest.raises(ValueError):
        excerpt.chart("2000-06-01T12:00:00Z", 41.9, 12.5, orb_settings={"fixed_star_orb": "wide"})
    with pytest.raises(ace.EngineError, match="no_such.bsp"):
        ace.Engine(["no_such.bsp"])


def load(name):
    return json.loads((FIXTURES / name).read_text())


@needs_full_kernel
def test_golden_natal_and_horary():
    engine = ace.Engine([FULL])
    for name, compute in (("natal.json", engine.chart), ("horary.json", engine.horary)):
        for case in load(name):
            i = case["input"]
            actual = compute(i["utc"], i["lat"], i["lon"], i["house_system"], i["zodiac_type"], i["ayanamsa"])
            assert_same(strip_private(actual), case["output"], i["id"])


@needs_full_kernel
def test_golden_transits_synastry_derived():
    engine = ace.Engine([FULL])
    natal = {c["input"]["id"]: c["output"] for c in load("natal.json")}
    for case in load("transits.json"):
        t = case["input"]["transit"]
        actual = engine.transit(
            natal[case["input"]["natal"]]["planets"],
            t["utc"], t["lat"], t["lon"], t["house_system"], t["zodiac_type"], t["ayanamsa"],
        )
        assert_same(actual, case["output"], t["id"])
    for case in load("synastry.json"):
        a, b = natal[case["input"]["a"]], natal[case["input"]["b"]]
        assert_same(ace.synastry(a["planets"], b["planets"]), case["output"])
    for case in load("derived.json"):
        base = natal[case["input"]["base"]]
        actual = ace.derived_chart(base, case["input"]["root_house"])
        assert_same(strip_private(actual), case["output"], case["input"]["base"], tol=0.0)


def test_election_criteria_are_validated(excerpt):
    with pytest.raises(ValueError, match="unknown variant"):
        excerpt.election("2000-06-01T12:00:00Z", 41.9, 12.5, {"purpose": "war"})
    with pytest.raises(ValueError, match="at most"):
        excerpt.elections("2000-01-01T00:00:00Z", "2000-06-01T00:00:00Z", 41.9, 12.5)


def test_election_and_search(excerpt):
    chart = excerpt.election("2000-06-01T12:00:00Z", 41.9, 12.5, {"purpose": "contract"})
    data = chart["election_data"]
    assert chart["planets"] and 0 <= data["score"] <= 100
    assert data["purpose"] == "contract"
    assert {"code", "weight", "planet", "target", "aspect", "house", "sign"} <= set(data["factors"][0])

    result = excerpt.elections(
        datetime(2000, 6, 1, tzinfo=timezone.utc), "2000-06-08T00:00:00Z", 41.9, 12.5, {"step_minutes": 30}
    )
    assert result["criteria"]["step_minutes"] == 30
    assert result["evaluated"] > 0
    for window in result["windows"]:
        assert window["start"] <= window["best"] <= window["end"]
        best = excerpt.election(window["best"], 41.9, 12.5)["election_data"]
        assert best["score"] == window["score"]
    json.dumps(result)
