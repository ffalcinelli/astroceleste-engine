# astroceleste-engine (WebAssembly)

Astrological chart calculation on NASA JPL ephemerides, compiled to WebAssembly from the
Rust [astroceleste-engine](https://github.com/ffalcinelli/astroceleste-engine). Charts are
computed entirely in the browser (or Node), identical to the Astroceleste server.

```js
import init, { Engine, synastry, derivedChart } from "astroceleste-engine";

await init();
const engine = new Engine();
const kernel = await (await fetch("/ephemeris/de440s.bsp")).arrayBuffer();
engine.addKernel("de440s.bsp", new Uint8Array(kernel));

const chart = engine.chart({
  utc: "1987-05-17T14:30:00Z",
  latitude: 41.9,
  longitude: 12.5,
  house_system: "P",        // optional: P, W, E, O
  zodiac_type: "sidereal",  // optional: tropical (default) or sidereal
  ayanamsa: "lahiri",       // optional
});
const horary = engine.horary(request);                 // chart + horary_data
const transit = engine.transit(chart.planets, request);
synastry(chartA.planets, chartB.planets);
derivedChart(chart, 5);
```

To ship a smaller kernel, cut the range you need (positions are unchanged inside it):

```js
import { excerptKernel } from "astroceleste-engine";
const small = excerptKernel(fullKernelBytes, 2415020.5, 2488069.5); // 1900-2100, ~21 MB
```

Errors are thrown as `Error` objects with a `code` (`ephemeris_out_of_range`,
`invalid_input`, `invalid_kernel`).

JPL kernels: https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/ (`de440s.bsp` covers 1849-2150,
32 MB). Licensed under MIT OR Apache-2.0.
