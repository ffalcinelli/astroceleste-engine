// Live demo: astroceleste-engine compiled to WebAssembly, computing charts in the browser.
// pkg/ and ephemeris/ are added by scripts/build-site.sh.
import init, { Engine, julianDay } from "./pkg/astroceleste_engine_wasm.js";

const KERNEL_URL = "ephemeris/de440s-1950-2050.bsp";
const KERNEL_NAME = "de440s-1950-2050.bsp";
const COVERAGE_TEXT = "1950–2050"; // keep in sync with scripts/build-site.sh

const SIGNS = [
  ["Aries", "♈", "fire"], ["Taurus", "♉", "earth"], ["Gemini", "♊", "air"],
  ["Cancer", "♋", "water"], ["Leo", "♌", "fire"], ["Virgo", "♍", "earth"],
  ["Libra", "♎", "air"], ["Scorpio", "♏", "water"], ["Sagittarius", "♐", "fire"],
  ["Capricorn", "♑", "earth"], ["Aquarius", "♒", "air"], ["Pisces", "♓", "water"],
];
const ASPECT_CLASS = {
  Conjunction: "neutral", Sextile: "harmonious", Trine: "harmonious",
  Square: "hard", Opposition: "hard", "Semi-Sextile": "harmonious", Quincunx: "hard",
};
const PRESETS = {
  now: () => {
    const iso = new Date().toISOString();
    return { date: iso.slice(0, 10), time: iso.slice(11, 19), latitude: 51.4769, longitude: 0 };
  },
  rome: () => ({ date: "1987-05-17", time: "14:30:00", latitude: 41.9, longitude: 12.5 }),
  moon: () => ({ date: "1969-07-20", time: "20:17:40", latitude: 29.5597, longitude: -95.0908 }),
  y2k: () => ({ date: "2000-01-01", time: "12:00:00", latitude: 51.4769, longitude: 0 }),
};

const $ = (id) => document.getElementById(id);
const form = $("chart-form");
const statusEl = $("status");
const submit = form.querySelector("button[type=submit]");
// Force text (not emoji) presentation of zodiac and planet glyphs.
const text = (glyph) => (/^[A-Z]+$/.test(glyph) ? glyph : `${glyph}\uFE0E`);
const escapeHtml = (s) =>
  String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
const dm = (p) => `${p.degree}°${String(p.minute).padStart(2, "0")}′`;

let enginePromise;
let lastJson = "";

function setStatus(html, isError = false) {
  statusEl.innerHTML = html;
  statusEl.classList.toggle("error", isError);
}

async function fetchWithProgress(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`could not download the ephemeris (HTTP ${res.status})`);
  const total = Number(res.headers.get("content-length")) || 0;
  if (!res.body) return new Uint8Array(await res.arrayBuffer());
  const reader = res.body.getReader();
  const chunks = [];
  let received = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    received += value.length;
    const mb = (received / 1e6).toFixed(1);
    const pct = total ? Math.min(100, (received / total) * 100) : 0;
    setStatus(
      `Downloading the JPL ephemeris… ${mb} MB` +
        (total ? ` of ${(total / 1e6).toFixed(1)} MB` : "") +
        `<span class="progress"><i style="width:${pct.toFixed(1)}%"></i></span>`,
    );
  }
  const bytes = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

function loadEngine() {
  enginePromise ??= (async () => {
    setStatus("Loading the engine…");
    await init();
    const bytes = await fetchWithProgress(KERNEL_URL);
    const engine = new Engine();
    engine.addKernel(KERNEL_NAME, bytes);
    return engine;
  })().catch((err) => {
    enginePromise = undefined; // allow a retry
    throw err;
  });
  return enginePromise;
}

function readForm() {
  const data = new FormData(form);
  const time = data.get("time") || "00:00";
  return {
    utc: `${data.get("date")}T${time.length === 5 ? `${time}:00` : time}Z`,
    latitude: Number(data.get("latitude")),
    longitude: Number(data.get("longitude")),
    house_system: data.get("house_system"),
    zodiac_type: data.get("zodiac_type"),
    ayanamsa: form.elements.ayanamsa.value,
  };
}

function fillForm(values) {
  for (const [key, value] of Object.entries(values)) {
    if (form.elements[key] !== undefined) form.elements[key].value = value;
  }
  syncAyanamsa();
}

function syncAyanamsa() {
  form.elements.ayanamsa.disabled = form.elements.zodiac_type.value !== "sidereal";
}

async function compute() {
  if (!form.reportValidity()) return;
  const request = readForm();
  submit.disabled = true;
  try {
    const engine = await loadEngine();
    if (!engine.supports(julianDay(request.utc))) {
      setStatus(
        `This demo ships an ephemeris excerpt for ${COVERAGE_TEXT} only. The engine itself ` +
          `covers 1849–2150 with DE440s, and 13200 BC–17191 AD with DE441.`,
        true,
      );
      return;
    }
    const start = performance.now();
    const chart = engine.chart(request);
    const ms = performance.now() - start;
    render(chart, request);
    const took = ms < 1 ? "under 1 ms" : ms < 10 ? `${ms.toFixed(1)} ms` : `${Math.round(ms)} ms`;
    setStatus(`Computed in ${took}, on your device.`);
    const params = new URLSearchParams({
      utc: request.utc, lat: request.latitude, lon: request.longitude,
      houses: request.house_system, zodiac: request.zodiac_type,
    });
    if (request.zodiac_type === "sidereal") params.set("ayanamsa", request.ayanamsa);
    history.replaceState(null, "", `?${params}#demo`);
  } catch (err) {
    const code = err && err.code ? ` (${err.code})` : "";
    setStatus(`${escapeHtml(err && err.message ? err.message : err)}${code}`, true);
  } finally {
    submit.disabled = false;
  }
}

// ---------------------------------------------------------------------------- rendering

function render(chart, request) {
  $("result").hidden = false;
  renderWheel(chart);
  renderSummary(chart);
  renderTables(chart);
  lastJson = JSON.stringify(chart, null, 2);
  $("json").textContent = lastJson;
  $("wheel").setAttribute(
    "aria-label",
    `Chart wheel for ${request.utc} at latitude ${request.latitude}, longitude ${request.longitude}`,
  );
}

function renderSummary(chart) {
  const find = (name) => chart.planets.find((p) => p.name === name);
  const rows = [];
  const sun = find("Sun");
  const moon = find("Moon");
  const asc = find("Ascendant");
  if (sun) rows.push(["Sun", `${dm(sun)} ${sun.sign}`]);
  if (moon) {
    const phase = chart.lunar_status && chart.lunar_status.phase_name;
    rows.push([
      "Moon",
      `${dm(moon)} ${moon.sign}` +
        (phase ? `<br><small>${phase}, ${chart.lunar_status.illumination_percentage}% lit</small>` : ""),
    ]);
  }
  if (asc) rows.push(["Ascendant", `${dm(asc)} ${asc.sign}`]);
  if (chart.temperament && chart.temperament.primary_temperament) {
    rows.push(["Temperament", `${chart.temperament.primary_temperament} – ${chart.temperament.secondary_temperament}`]);
  }
  rows.push([
    "Zodiac",
    chart.zodiac_type === "sidereal"
      ? `Sidereal, ${chart.ayanamsa_name}<br><small>ayanamsa ${chart.ayanamsa_formatted}</small>`
      : "Tropical",
  ]);
  $("summary").innerHTML = rows
    .map(([k, v]) => `<div><dt>${k}</dt><dd>${v}</dd></div>`)
    .join("");
}

function table(el, head, rows) {
  el.innerHTML =
    `<thead><tr>${head.map((h) => `<th>${h}</th>`).join("")}</tr></thead>` +
    `<tbody>${rows.map((r) => `<tr>${r.join("")}</tr>`).join("")}</tbody>`;
}

function renderTables(chart) {
  const glyphOf = new Map(chart.planets.map((p) => [p.name, p.symbol]));
  table($("planets"), ["", "Point", "Position", "House", "Speed °/day"], chart.planets.map((p) => [
    `<td class="glyph">${escapeHtml(text(p.symbol))}</td>`,
    `<td>${escapeHtml(p.name)}${p.is_retrograde ? ' <span class="retro" title="retrograde">℞</span>' : ""}</td>`,
    `<td>${dm(p)} <span class="glyph">${text(p.sign_symbol)}</span> ${p.sign}</td>`,
    `<td class="num">${p.house}</td>`,
    `<td class="num">${p.speed ? p.speed.toFixed(3) : "–"}</td>`,
  ]));
  table($("houses"), ["House", "Cusp"], chart.houses.map((h) => [
    `<td>${h.house_number}</td>`,
    `<td>${dm(h)} <span class="glyph">${text(h.sign_symbol)}</span> ${h.sign}</td>`,
  ]));
  const label = (name) =>
    glyphOf.has(name) ? `<span class="glyph">${escapeHtml(text(glyphOf.get(name)))}</span> ${escapeHtml(name)}` : `★ ${escapeHtml(name)}`;
  table($("aspects"), ["", "Aspect", "", "Orb"], chart.aspects.map((a) => [
    `<td>${label(a.body1)}</td>`,
    `<td class="${ASPECT_CLASS[a.aspect_type] || ""}"><span class="glyph">${escapeHtml(text(a.symbol))}</span> ${escapeHtml(a.aspect_type)}</td>`,
    `<td>${label(a.body2)}</td>`,
    `<td class="num">${a.orb.toFixed(2)}°</td>`,
  ]));
}

const SVG = "http://www.w3.org/2000/svg";
const C = 200;
const R = { outer: 195, zodiac: 165, tick: 158, glyph: 141, houseNum: 106, inner: 95 };

function renderWheel(chart) {
  const svg = $("wheel");
  svg.replaceChildren();
  const byName = new Map(chart.planets.map((p) => [p.name, p]));
  const asc = (byName.get("Ascendant") || chart.houses[0]).ecliptic_longitude;
  // Ascendant on the left, longitudes increasing counterclockwise.
  const xy = (lon, r) => {
    const a = ((180 + lon - asc) * Math.PI) / 180;
    return [C + r * Math.cos(a), C - r * Math.sin(a)];
  };
  const add = (tag, attrs, content) => {
    const el = document.createElementNS(SVG, tag);
    for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
    if (content !== undefined) el.textContent = content;
    svg.append(el);
    return el;
  };
  const line = (lon1, r1, lon2, r2, cls) => {
    const [x1, y1] = xy(lon1, r1);
    const [x2, y2] = xy(lon2, r2);
    return add("line", { x1, y1, x2, y2, class: cls });
  };

  add("circle", { cx: C, cy: C, r: R.outer, class: "w-zodiac" });
  add("circle", { cx: C, cy: C, r: R.zodiac, class: "w-ring" });
  add("circle", { cx: C, cy: C, r: R.inner, class: "w-ring" });
  SIGNS.forEach(([name, glyph, element], i) => {
    line(i * 30, R.zodiac, i * 30, R.outer, "w-sign-line");
    const [x, y] = xy(i * 30 + 15, (R.zodiac + R.outer) / 2);
    add("text", { x, y, class: `w-sign ${element}` }, text(glyph)).append(
      Object.assign(document.createElementNS(SVG, "title"), { textContent: name }),
    );
  });

  chart.houses.forEach((h, i) => {
    const next = chart.houses[(i + 1) % 12].ecliptic_longitude;
    const axis = h.house_number === 1 || h.house_number === 4 || h.house_number === 7 || h.house_number === 10;
    line(h.ecliptic_longitude, R.inner, h.ecliptic_longitude, R.zodiac, axis ? "w-axis" : "w-cusp");
    const span = (next - h.ecliptic_longitude + 360) % 360;
    const [x, y] = xy(h.ecliptic_longitude + span / 2, R.houseNum);
    add("text", { x, y, class: "w-house-num" }, h.house_number);
  });

  const points = chart.planets.filter((p) => typeof p.ecliptic_longitude === "number");
  for (const a of chart.aspects) {
    const p1 = byName.get(a.body1);
    const p2 = byName.get(a.body2);
    const cls = ASPECT_CLASS[a.aspect_type];
    if (!p1 || !p2 || cls === "neutral" || !cls) continue;
    const minor = a.is_major === false ? " minor" : "";
    line(p1.ecliptic_longitude, R.inner, p2.ecliptic_longitude, R.inner, `w-aspect ${cls}${minor}`);
  }

  const shown = spread(points.map((p) => p.ecliptic_longitude), 8);
  points.forEach((p, i) => {
    line(p.ecliptic_longitude, R.zodiac, p.ecliptic_longitude, R.tick, "w-tick");
    line(p.ecliptic_longitude, R.tick, shown[i], R.glyph + 9, "w-leader");
    const [x, y] = xy(shown[i], R.glyph);
    const angle = /^[A-Z]+$/.test(p.symbol);
    const el = add("text", { x, y, class: `w-planet${angle ? " angle" : ""}` }, text(p.symbol));
    el.append(Object.assign(document.createElementNS(SVG, "title"), {
      textContent: `${p.name} ${dm(p)} ${p.sign}${p.is_retrograde ? " (retrograde)" : ""}`,
    }));
  });
}

/** Display longitudes at least `minSep` degrees apart, as close as possible to the real ones. */
function spread(lons, minSep) {
  const norm = (a) => ((a % 360) + 360) % 360;
  const pos = lons.slice();
  if (pos.length < 2) return pos;
  for (let iter = 0; iter < 200; iter++) {
    let moved = false;
    const order = [...pos.keys()].sort((a, b) => pos[a] - pos[b]);
    for (let k = 0; k < order.length; k++) {
      const a = order[k];
      const b = order[(k + 1) % order.length];
      const gap = norm(pos[b] - pos[a]);
      if (gap < minSep - 1e-6) {
        const push = (minSep - gap) / 2;
        pos[a] = norm(pos[a] - push);
        pos[b] = norm(pos[b] + push);
        moved = true;
      }
    }
    if (!moved) break;
  }
  return pos;
}

// ---------------------------------------------------------------------------- wiring

form.addEventListener("submit", (e) => {
  e.preventDefault();
  compute();
});
form.elements.zodiac_type.addEventListener("change", syncAyanamsa);
for (const button of form.querySelectorAll("[data-preset]")) {
  button.addEventListener("click", () => {
    fillForm(PRESETS[button.dataset.preset]());
    compute();
  });
}
$("copy-json").addEventListener("click", async (e) => {
  try {
    await navigator.clipboard.writeText(lastJson);
    e.target.textContent = "Copied";
  } catch {
    e.target.textContent = "Copy failed";
  }
  setTimeout(() => (e.target.textContent = "Copy"), 1500);
});

// A shared link (?utc=…&lat=…) computes its chart on load.
const params = new URLSearchParams(location.search);
const utc = params.get("utc");
if (utc && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2})?Z?$/.test(utc)) {
  fillForm({
    date: utc.slice(0, 10),
    time: utc.slice(11, 19).replace(/Z$/, ""),
    latitude: params.get("lat") ?? form.elements.latitude.value,
    longitude: params.get("lon") ?? form.elements.longitude.value,
    house_system: params.get("houses") ?? "P",
    zodiac_type: params.get("zodiac") ?? "tropical",
    ayanamsa: params.get("ayanamsa") ?? "galcent_0sag",
  });
  compute();
}
syncAyanamsa();
