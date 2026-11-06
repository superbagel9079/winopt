// winopt's page: reads the machine through the Rust side (`scan`) and shows it in one
// window, the way an old control panel did: the sections on the left, the one chosen on
// the right. Mouse first, with keys as shortcuts.
// Outside the app (a plain browser), it shows mock.json, a scan saved from a real machine.

"use strict";

const $ = (id) => document.getElementById(id);

/** An element with its attributes and children (strings become text, never HTML). */
function el(tag, attrs, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs || {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key === "class") node.className = value;
    else node.setAttribute(key, value);
  }
  for (const child of children.flat()) {
    if (child === undefined || child === null || child === false || child === "") continue;
    node.append(child instanceof Node ? child : String(child));
  }
  return node;
}

// The start screen's pace. It does not follow Windows' "animation effects" switch: lines
// printed one after another are a reading pace, and with no delay nothing could be read.
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const SPLASH = { spin: 120, minReading: 1200, line: 220, hold: 1800 };
const SPINNER = ["|", "/", "-", "\\"];

// --- formatting ----------------------------------------------------------------------

const GIB = 1024 ** 3;

/** Memory in binary units, as Windows shows it: "32 GB", "8 GB", "1.5 GB". */
function memorySize(bytes) {
  const gb = bytes / GIB;
  // Drivers report a little under the size on the box (7.996 GB for an 8 GB card).
  const whole = Math.round(gb);
  return `${Math.abs(gb - whole) < 0.05 ? whole : gb.toFixed(1)} GB`;
}

/** Disks in decimal units, as they are sold: "2 TB", "512 GB". */
function diskSize(bytes) {
  if (bytes >= 1e12) return `${+(bytes / 1e12).toFixed(1)} TB`;
  return `${Math.round(bytes / 1e9)} GB`;
}

/** Names without the trademark marks, which a narrow line cannot spare. */
const plain = (name) => (name || "").replace(/\((R|TM)\)/gi, "").replace(/\s+/g, " ").trim();

const join = (...parts) => parts.filter(Boolean).join(" · ");

/** A Windows 2000 progress bar: a sunken box of separate blocks, filled up to `share`
 *  (0 to 1), with its percentage and an optional word after it. */
function bar(share, label = "") {
  const clamped = Math.max(0, Math.min(1, share));
  const filled = Math.round(clamped * BAR_BLOCKS);
  const blocks = Array.from({ length: BAR_BLOCKS }, (_, i) => el("i", { class: i < filled ? "on" : null }));
  return el("div", { class: "bar-row" },
    el("div", { class: "bar", role: "img", "aria-label": `${Math.round(clamped * 100)} %` }, blocks),
    el("span", { class: "bar-value" }, join(`${Math.round(clamped * 100)} %`, label)));
}

const BAR_BLOCKS = 30;

const FORM = { laptop: "Laptop", desktop: "Desktop", unknown: "Unknown" };
const VENDOR = { nvidia: "NVIDIA", amd: "AMD", intel: "Intel", other: "" };
const MARK = { warning: "[!]", advice: "[>]", note: "[i]" };

function machineName(m) {
  return (m.family && !m.model.includes(m.family) ? `${m.family} ${m.model}` : m.model) || "Unknown model";
}

/** Identical memory modules counted once: "8 × 4 GB". */
function moduleGroups(modules) {
  const groups = new Map();
  for (const m of modules) {
    const key = [m.capacityBytes, m.kind, m.ratedMhz, m.configuredMhz, m.manufacturer, m.part].join("|");
    const group = groups.get(key) || { ...m, count: 0 };
    group.count += 1;
    groups.set(key, group);
  }
  return [...groups.values()];
}

const installedMemory = (inv) => inv.memory.modules.reduce((sum, m) => sum + m.capacityBytes, 0) || inv.memory.totalBytes;

// --- window contents -----------------------------------------------------------------

function fact(key, value) {
  if (value === undefined || value === null || value === "" || value === false) return null;
  return el("div", { class: "fact" }, el("span", { class: "k" }, key), el("span", { class: "dots" }), el("span", { class: "v" }, value));
}

const heading = (text) => el("h3", {}, text);

function overview({ inventory: inv, findings }) {
  const m = inv.machine;
  const count = (level) => findings.filter((f) => f.level === level).length;
  const discrete = inv.gpus.find((g) => g.discrete) || inv.gpus[0];
  return [
    heading("This machine"),
    fact("Maker", m.manufacturer),
    fact("Model", machineName(m)),
    fact("Type", FORM[m.form]),
    fact("Board", m.board),
    fact("BIOS", m.biosVersion && join(m.biosVersion, m.biosDate)),
    fact("Windows", join(inv.os.caption.replace(/^Microsoft /, ""), inv.os.build && `build ${inv.os.build}`)),
    heading("At a glance"),
    fact("Processor", plain(inv.cpu.name)),
    fact("Graphics", discrete && plain(discrete.name)),
    fact("Memory", memorySize(installedMemory(inv))),
    fact("Storage", inv.disks.length && inv.disks.map((d) => diskSize(d.sizeBytes)).join(" + ")),
    fact("Power plan", inv.power.planName),
    heading("Findings"),
    fact("Slowing it down", String(count("warning"))),
    fact("Worth changing", String(count("advice"))),
    fact("For information", String(count("note"))),
  ];
}

function findingsSection({ findings }) {
  if (!findings.length) return [el("p", {}, "Nothing to report on this machine.")];
  return [
    ...findings.map((f) => el("div", { class: `finding ${f.level}` },
      el("span", { class: "mark" }, MARK[f.level]),
      el("div", {},
        el("div", {}, el("span", { class: "what" }, f.title), el("span", { class: "area" }, `  (${f.area})`)),
        el("div", { class: "why" }, f.detail)))),
    el("p", { class: "legend" }, "[!] slows the machine down   [>] worth changing   [i] for information"),
  ];
}

function processor({ inventory: { cpu } }) {
  return [
    fact("Model", plain(cpu.name)),
    fact("Maker", cpu.manufacturer),
    fact("Cores", cpu.cores && String(cpu.cores)),
    fact("Threads", cpu.threads && String(cpu.threads)),
    fact("Base clock", cpu.baseMhz && `${(cpu.baseMhz / 1000).toFixed(2)} GHz`),
  ];
}

function graphics({ inventory: { gpus } }) {
  if (!gpus.length) return [el("p", {}, "No graphics card found.")];
  return gpus.flatMap((g, i) => [
    heading(`GPU ${i}: ${plain(g.name)}`),
    fact("Type", join(g.discrete ? "Discrete" : "Integrated", VENDOR[g.vendor])),
    fact("Video memory", g.vramBytes && memorySize(g.vramBytes)),
    fact("Driver", g.driver),
    fact("Display", g.width && `${g.width}x${g.height}${g.refreshHz ? ` @ ${g.refreshHz} Hz` : ""}`),
  ]);
}

function memory({ inventory: { memory: mem } }) {
  // Windows' total leaves out what the firmware and the integrated graphics keep: the
  // modules give what is installed.
  const installed = mem.modules.reduce((sum, m) => sum + m.capacityBytes, 0);
  const out = [
    fact("Installed", installed && memorySize(installed)),
    fact("Usable", mem.totalBytes && memorySize(mem.totalBytes)),
    installed && mem.totalBytes && bar(mem.totalBytes / installed, "usable"),
  ];
  moduleGroups(mem.modules).forEach((g, i) => {
    const speed = g.ratedMhz > g.configuredMhz && g.configuredMhz
      ? `${g.configuredMhz} MT/s (rated ${g.ratedMhz})`
      : g.configuredMhz && `${g.configuredMhz} MT/s`;
    out.push(
      heading(`Modules ${i + 1}`),
      fact("Count", `${g.count} x ${memorySize(g.capacityBytes)}`),
      fact("Type", join(g.kind, g.soldered && "soldered")),
      fact("Speed", speed),
      fact("Maker", g.manufacturer),
      fact("Part", g.part));
  });
  return out;
}

function storage({ inventory: { disks } }) {
  if (!disks.length) return [el("p", {}, "No disk found.")];
  return disks.flatMap((d, i) => [
    heading(`Disk ${i}: ${d.name || "unnamed"}${d.system ? "  [Windows]" : ""}`),
    fact("Bus", d.bus),
    fact("Media", d.media),
    fact("Size", diskSize(d.sizeBytes)),
  ]);
}

function power({ inventory: { power: p, machine } }) {
  const b = p.battery;
  return [
    fact("Plan", p.planName),
    fact("GUID", p.planGuid),
    heading("Battery"),
    b ? fact("Source", b.onAc ? "Plugged in" : "On battery") : fact("Battery", machine.form === "laptop" ? "not read" : "none"),
    b && b.chargePercent != null && fact("Charge", `${b.chargePercent} %`),
    b && b.chargePercent != null && bar(b.chargePercent / 100),
  ];
}

function tools({ inventory: inv }) {
  const t = inv.tools;
  const out = [heading("Package manager"), fact("Chocolatey", t.chocolatey ? "installed" : "not installed")];
  if (/asus/i.test(inv.machine.manufacturer)) {
    out.push(heading("ASUS"), fact("G-Helper", t.ghelperRunning ? "running" : "not running"));
    if (!t.asusServices.length) out.push(fact("Services", "none"));
    for (const [name, mode] of t.asusServices) out.push(fact(name, mode.toLowerCase()));
  }
  return out;
}

/** The logo: the name at a command prompt, the cursor blinking after it. */
function logo(className) {
  return el("span", { class: `logo ${className}`, "aria-label": "winopt" },
    el("span", { class: "prompt" }, "C:\\>"), " winopt", el("span", { class: "block-cursor", "aria-hidden": "true" }));
}

// --- tuning: power plan ------------------------------------------------------------------

const inApp = Boolean(window.__TAURI__);

/** When the changes were applied, as a short date and time. */
function appliedWhen(seconds) {
  return new Date(seconds * 1000).toLocaleString("en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

function powerTuning({ power: p }) {
  const out = [el("p", {}, p.reason)];
  if (p.error) out.push(el("p", { class: "error" }, p.error));
  out.push(heading("Plan"), fact("In use", p.planNow), p.planAfter && fact("After Apply", p.planAfter));
  if (p.changes.length) {
    out.push(heading("What Apply changes"),
      ...p.changes.map((c) => fact(`${c.setting} (${c.when})`, `${c.before} → ${c.after}`)));
  } else if (p.actionable && !p.planAfter) {
    out.push(el("p", { class: "dim" }, "Everything is already set the way winopt would set it."));
  }
  if (p.appliedAt) {
    out.push(el("p", { class: "dim" }, `winopt applied its changes on ${appliedWhen(p.appliedAt)}. Undo puts back the values it found.`));
  }
  if (p.actionable || p.appliedAt) {
    const canApply = inApp && (p.changes.length > 0 || Boolean(p.planAfter)) && !state.busy;
    const canUndo = inApp && Boolean(p.appliedAt) && !state.busy;
    out.push(el("div", { class: "actions" },
      el("button", { type: "button", class: "btn", "data-power": "apply", disabled: !canApply || null }, "Apply"),
      el("button", { type: "button", class: "btn", "data-power": "undo", disabled: !canUndo || null }, "Undo"),
      !inApp && el("span", { class: "dim" }, "Apply and Undo work in the app only.")));
  }
  if (state.powerError) out.push(el("p", { class: "error" }, state.powerError));
  return out;
}

/** Runs Apply or Undo, then reads the machine again so every section shows the result. */
async function runPower(action) {
  if (state.busy || !inApp) return;
  state.busy = true;
  state.powerError = null;
  renderContent();
  $("status-right").textContent = action === "apply" ? "applying…" : "undoing…";
  try {
    state.report.power = await window.__TAURI__.core.invoke(action === "apply" ? "power_apply" : "power_undo");
  } catch (e) {
    state.powerError = `${action === "apply" ? "Apply" : "Undo"} failed: ${e?.message || e}`;
  } finally {
    state.busy = false;
  }
  await refresh();
  renderContent();
}

function about() {
  return [
    el("div", { class: "about" }, logo("logo-big")),
    el("p", { class: "dim" }, "Version 0.1.0"),
    el("p", {}, "winopt reads what this PC is, then suggests the settings that suit it."),
    el("p", { class: "dim" }, "It changes nothing until you click Apply, and Undo puts back what it found."),
  ];
}

// --- sections and themes -------------------------------------------------------------

// Grouped as in an old settings panel; `count` puts a number at the right of the row.
const SECTIONS = [
  { id: "overview", group: "Machine", label: "This PC", file: "THISPC", render: overview, subtitle: (r) => machineName(r.inventory.machine) },
  { id: "findings", group: "Machine", label: "Findings", file: "FINDINGS", render: findingsSection,
    subtitle: (r) => `${r.findings.length} for this machine`, count: (r) => r.findings.length },
  { id: "processor", group: "Hardware", label: "Processor", file: "CPU", render: processor, subtitle: (r) => plain(r.inventory.cpu.name) },
  { id: "graphics", group: "Hardware", label: "Graphics", file: "GPU", render: graphics,
    subtitle: (r) => r.inventory.gpus.map((g) => plain(g.name)).join(" + "), count: (r) => r.inventory.gpus.length },
  { id: "memory", group: "Hardware", label: "Memory", file: "RAM", render: memory, subtitle: (r) => memorySize(installedMemory(r.inventory)) },
  { id: "storage", group: "Hardware", label: "Storage", file: "DISKS", render: storage,
    subtitle: (r) => r.inventory.disks.map((d) => diskSize(d.sizeBytes)).join(" + "), count: (r) => r.inventory.disks.length },
  { id: "power", group: "System", label: "Power", file: "POWER", render: power, subtitle: (r) => r.inventory.power.planName },
  { id: "tools", group: "System", label: "Tools", file: "TOOLS", render: tools, subtitle: () => "Installed and running" },
  { id: "tune-power", group: "Tuning", label: "Power plan", file: "POWERPLN", render: powerTuning,
    subtitle: (r) => r.power.title, count: (r) => r.power.changes.length || null },
  { id: "about", group: "", label: "About", file: "ABOUT", render: about, subtitle: () => "Version 0.1.0" },
];

const THEMES = [
  { id: "platinum", name: "Platinum" },
  { id: "graphite", name: "Graphite" },
  { id: "bondi", name: "Bondi Blue" },
  { id: "lime", name: "Lime" },
  { id: "tangerine", name: "Tangerine" },
  { id: "strawberry", name: "Strawberry" },
  { id: "grape", name: "Grape" },
  { id: "gold", name: "Gold" },
  { id: "charcoal", name: "Charcoal" },
];


/** The menu bar's pull-downs: what each offers, and the body attribute it sets. */
const MENUS = { theme: THEMES };

const state = { report: null, selected: 0, booting: false, refreshing: false, busy: false, powerError: null };

/** How often the machine is read again while the window is shown: the source of power,
 *  the plan, the tools running can change; one reading costs WMI about a second. */
const REFRESH_MS = 60_000;

/** Preferences kept in the browser; the page works the same when storage is refused. */
const prefs = {
  get(key, fallback) { try { return localStorage.getItem(key) ?? fallback; } catch { return fallback; } },
  set(key, value) { try { localStorage.setItem(key, value); } catch { /* not kept, still applied */ } },
};

function current(menu) {
  const items = MENUS[menu];
  const id = prefs.get(menu, items[0].id);
  return items.find((item) => item.id === id) || items[0];
}

function applyLook() {
  for (const menu of Object.keys(MENUS)) {
    document.body.dataset[menu] = current(menu).id;
    renderMenu(menu);
  }
}

function choose(menu, id) {
  prefs.set(menu, id);
  applyLook();
}

function next(menu) {
  const items = MENUS[menu];
  choose(menu, items[(items.indexOf(current(menu)) + 1) % items.length].id);
}

function renderMenu(menu) {
  const chosen = current(menu).id;
  $(`${menu}-menu`).replaceChildren(...MENUS[menu].map((item) => el("li", { role: "none" },
    el("button", { type: "button", role: "menuitemradio", "aria-checked": String(item.id === chosen), "data-pick": `${menu}:${item.id}` },
      el("span", { class: "check" }, item.id === chosen ? "✓" : ""), item.name))));
}

/** Opens one pull-down (closing the others), or closes them all with `null`. */
function openMenu(menu) {
  for (const name of Object.keys(MENUS)) {
    const show = name === menu && $(`${name}-menu`).hidden;
    $(`${name}-menu`).hidden = !show;
    document.querySelector(`[data-menu="${name}"]`).setAttribute("aria-expanded", String(show));
  }
}

/** The frame's buttons act on the program's own window; in a plain browser they do nothing. */
function windowAction(action) {
  const win = window.__TAURI__?.window.getCurrentWindow();
  if (!win) return;
  if (action === "minimize") win.minimize();
  else if (action === "maximize") win.toggleMaximize();
  else if (action === "close") win.close();
}

// --- the window ----------------------------------------------------------------------

function renderSidebar() {
  const rows = [];
  SECTIONS.forEach((s, i) => {
    if (s.group && s.group !== SECTIONS[i - 1]?.group) rows.push(el("div", { class: "group" }, s.group.toUpperCase()));
    // A count of null (nothing to show, as with no power change pending) leaves the row bare.
    const count = s.count?.(state.report);
    rows.push(el("button", {
      type: "button",
      class: `item${i === state.selected ? " selected" : ""}${s.id === "about" ? " last" : ""}`,
      "data-index": String(i),
      "aria-current": i === state.selected ? "page" : null,
    }, el("span", {}, s.label), count != null && el("span", { class: "count" }, String(count))));
  });
  $("sidebar").replaceChildren(...rows);
}

function renderContent() {
  const section = SECTIONS[state.selected];
  $("content-head").replaceChildren(
    section.group && el("div", { class: "group" }, section.group.toUpperCase()),
    el("h2", {}, section.label),
    el("div", { class: "dim" }, section.subtitle(state.report) || ""));
  $("content-body").replaceChildren(...section.render(state.report).filter(Boolean));
  $("content").scrollTop = 0;
  $("status-path").textContent = `C:\\WINOPT\\${section.file}.TXT`;
}

function select(index) {
  if (!state.report) return;
  state.selected = (index + SECTIONS.length) % SECTIONS.length;
  renderSidebar();
  renderContent();
}

function showMachine() {
  const name = machineName(state.report.inventory.machine);
  const time = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  $("app-title").textContent = name;
  $("status-right").textContent = `read at ${time}`;
  $("splash").hidden = true;
  $("sidebar").hidden = false;
  $("content").hidden = false;
  select(state.selected);
}

// --- start ---------------------------------------------------------------------------

function scan() {
  if (window.__TAURI__) return window.__TAURI__.core.invoke("scan");
  return fetch("mock.json", { cache: "no-store" }).then((r) => {
    if (!r.ok) throw new Error(`mock.json: HTTP ${r.status}`);
    return r.json();
  });
}

/** The lines a BIOS would print about this machine. */
function bootLines({ inventory: inv }) {
  const line = (key, value) => `${key.padEnd(12, ".")} ${value}`;
  return [
    line("Machine", machineName(inv.machine)),
    line("Processor", plain(inv.cpu.name)),
    ...inv.gpus.map((g) => line("Graphics", plain(g.name))),
    line("Memory", memorySize(installedMemory(inv))),
    ...inv.disks.map((d, i) => line(`Disk ${i}`, join(d.name, diskSize(d.sizeBytes)))),
    ...inv.warnings.map((w) => line("Not read", w)),
  ];
}

async function start() {
  if (state.booting) return;
  state.booting = true;
  $("sidebar").hidden = true;
  $("content").hidden = true;
  $("splash").hidden = false;
  $("status-right").textContent = "reading…";
  $("status-path").textContent = "C:\\WINOPT";
  const log = $("splash-log");
  const cursor = el("span", { class: "cursor" }, "_");
  const print = (text) => cursor.before(text + "\n");
  log.replaceChildren(cursor);
  cursor.before("Reading the machine... ");
  const spinner = el("span", {}, SPINNER[0]);
  cursor.before(spinner);
  // The reading often takes under a second: the spinner turns a little longer, so the
  // line is seen before its answer.
  const reading = scan();
  let done = false;
  Promise.allSettled([reading, sleep(SPLASH.minReading)]).then(() => { done = true; });
  for (let turn = 1; !done; turn++) {
    await sleep(SPLASH.spin);
    spinner.textContent = SPINNER[turn % SPINNER.length];
  }
  spinner.remove();
  try {
    state.report = await reading;
    print("OK\n");
    for (const text of bootLines(state.report)) {
      await sleep(SPLASH.line);
      print(text);
    }
    await sleep(SPLASH.hold);
    showMachine();
  } catch (e) {
    print("failed\n");
    print(`The machine could not be read: ${e?.message || e}`);
    print("winopt will try again in a minute.");
    $("status-right").textContent = "not read";
  } finally {
    state.booting = false;
  }
}

/** Reads the machine again without the start screen, and updates what is shown in place.
 *  A failed reading keeps the last one on screen and says so in the status bar. */
async function refresh() {
  if (state.booting || state.refreshing || document.hidden) return;
  if (!state.report) { start(); return; }
  state.refreshing = true;
  try {
    state.report = await scan();
    const scroll = $("content").scrollTop;
    showMachine();
    $("content").scrollTop = scroll;
  } catch (e) {
    $("status-right").textContent = `not updated: ${e?.message || e}`;
  } finally {
    state.refreshing = false;
  }
}

// --- input ---------------------------------------------------------------------------

$("menubar").addEventListener("click", (e) => {
  const pick = e.target.closest("button[data-pick]")?.dataset.pick;
  if (pick) {
    const [menu, id] = pick.split(":");
    choose(menu, id);
    openMenu(null);
    return;
  }
  const menu = e.target.closest("button[data-menu]")?.dataset.menu;
  if (menu) { openMenu(menu); return; }
  openMenu(null);
  if (e.target.closest('[data-action="about"]')) select(SECTIONS.findIndex((s) => s.id === "about"));
});

document.addEventListener("pointerdown", (e) => {
  if (!e.target.closest(".pulldown")) openMenu(null);
});

document.querySelector(".titlebar").addEventListener("click", (e) => {
  const action = e.target.closest("[data-win]")?.dataset.win;
  if (action) windowAction(action);
});

$("content").addEventListener("click", (e) => {
  const action = e.target.closest("button[data-power]")?.dataset.power;
  if (action) runPower(action);
});

$("sidebar").addEventListener("click", (e) => {
  const item = e.target.closest(".item");
  if (item) select(Number(item.dataset.index));
});

document.addEventListener("keydown", (e) => {
  if (e.key === "F2") { e.preventDefault(); next("theme"); return; }
  if (e.key === "Escape") { openMenu(null); return; }
  if (e.ctrlKey || e.altKey || e.metaKey) return;
  if (e.key === "ArrowUp") { e.preventDefault(); select(state.selected - 1); }
  else if (e.key === "ArrowDown") { e.preventDefault(); select(state.selected + 1); }
});

function tickClock() {
  const now = new Date();
  const date = now.toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short", year: "numeric" });
  const time = now.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
  $("clock").textContent = `${date}  ${time}`;
}

$("splash-logo").replaceWith(logo("logo-splash"));
applyLook();
tickClock();
setInterval(tickClock, 15000);
// Read again every minute, and as soon as winopt comes back to the front.
setInterval(refresh, REFRESH_MS);
document.addEventListener("visibilitychange", () => { if (!document.hidden) refresh(); });
window.addEventListener("focus", refresh);
start();
