"""Astrological chart calculation on JPL ephemerides."""

from datetime import datetime
from os import PathLike
from typing import Any

__version__: str

Moment = datetime | str
"""An aware (or naive, taken as UTC) datetime, or an ISO 8601 string."""

class EngineError(Exception): ...
class EphemerisRangeError(EngineError): ...

class Engine:
    """Chart calculator over JPL kernels, given in preference order."""

    def __init__(self, kernels: list[str | PathLike[str]]) -> None: ...
    @property
    def kernels(self) -> list[tuple[str, float, float]]:
        """(name, first JD, last JD) of each loaded kernel."""
    @property
    def coverage(self) -> tuple[float, float] | None: ...
    def supports(self, jd: float) -> bool: ...
    def chart(
        self,
        moment: Moment,
        latitude: float,
        longitude: float,
        house_system: str = "P",
        zodiac_type: str = "tropical",
        ayanamsa: str = "galcent_0sag",
        orb_settings: dict[str, Any] | None = None,
    ) -> dict[str, Any]: ...
    def horary(
        self,
        moment: Moment,
        latitude: float,
        longitude: float,
        house_system: str = "P",
        zodiac_type: str = "tropical",
        ayanamsa: str = "galcent_0sag",
        orb_settings: dict[str, Any] | None = None,
    ) -> dict[str, Any]: ...
    def election(
        self,
        moment: Moment,
        latitude: float,
        longitude: float,
        criteria: dict[str, Any] | None = None,
        house_system: str = "P",
        zodiac_type: str = "tropical",
        ayanamsa: str = "galcent_0sag",
        orb_settings: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """A chart with its electional assessment under `election_data`."""
    def elections(
        self,
        start: Moment,
        end: Moment,
        latitude: float,
        longitude: float,
        criteria: dict[str, Any] | None = None,
        house_system: str = "P",
        zodiac_type: str = "tropical",
        ayanamsa: str = "galcent_0sag",
    ) -> dict[str, Any]:
        """The best electional windows from `start` to `end` (at most 92 days)."""
    def transit(
        self,
        natal_planets: list[dict[str, Any]],
        moment: Moment,
        latitude: float,
        longitude: float,
        house_system: str = "P",
        zodiac_type: str = "tropical",
        ayanamsa: str = "galcent_0sag",
        orb_settings: dict[str, Any] | None = None,
    ) -> dict[str, Any]: ...

def synastry(
    chart_a_planets: list[dict[str, Any]],
    chart_b_planets: list[dict[str, Any]],
    orb_settings: dict[str, Any] | None = None,
) -> dict[str, Any]: ...
def derived_chart(
    base_chart: dict[str, Any], root_house: int, custom_name: str | None = None
) -> dict[str, Any]: ...
def julian_day(moment: Moment) -> float: ...
