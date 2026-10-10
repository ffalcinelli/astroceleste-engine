// Smoke test of the browser build (`--target web`), the one published to npm, run with Node:
//   wasm-pack build --release --target web crates/astroceleste-engine-wasm
//   node crates/astroceleste-engine-wasm/tests/web_smoke.mjs
// golden.mjs covers the API in depth on the Node build; this checks that the published
// package loads and computes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, "../pkg");
const { initSync, Engine, julianDay, degreeQualities } = await import(join(pkg, "astroceleste_engine_wasm.js"));
initSync({ module: readFileSync(join(pkg, "astroceleste_engine_wasm_bg.wasm")) });

const engine = new Engine();
engine.addKernel("de440s_2000.bsp", readFileSync(join(here, "../../../tests/data/de440s_2000.bsp")));
const chart = engine.chart({ utc: "2000-06-01T12:00:00Z", latitude: 41.9, longitude: 12.5 });
assert.equal(chart.planets[0].name, "Sun");
assert.equal(chart.planets[0].sign, "Gemini");
assert.equal(chart.houses.length, 12);
assert.equal(julianDay("2000-06-01T12:00:00Z"), 2451697);
assert.equal(degreeQualities(5.5).sign, "Aries");
assert.throws(() => engine.chart({ utc: "2010-01-01T00:00:00Z", latitude: 0, longitude: 0 }), (e) => e.code === "ephemeris_out_of_range");
console.log("web package: ok");
