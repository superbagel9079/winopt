// winopt's page: reads the machine through the Rust side (`scan`) and shows it.
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

const join = (...parts) => parts.filter(Boolean).join(" · ");

const FORM = { laptop: "laptop", desktop: "desktop", unknown: "kind unknown" };
const VENDOR = { nvidia: "NVIDIA", amd: "AMD", intel: "Intel", other: "" };
const MARK = { warning: "!", advice: "→", note: "·" };

// --- sections ------------------------------------------------------------------------

function fact(key, value) {
  if (value === undefined || value === null || value === "") return null;
  return el("li", {}, el("span", { class: "k" }, key), el("span", { class: "v" }, value));
}

/** A row that opens a group of facts (one graphics card, one disk). */
function head(value) {
  return el("li", { class: "head" }, el("span", { class: "v" }, value));
}

function card(title, rows, wide) {
  return el("section", { class: wide ? "card wide" : "card" }, el("h2", {}, title), el("ul", { class: "facts" }, rows));
}

function machineBlock(inv) {
  const m = inv.machine;
  const name = m.family && !m.model.includes(m.family) ? `${m.family} ${m.model}` : m.model;
  const discrete = inv.gpus.find((g) => g.discrete);
  const tags = [
    el("span", { class: "tag strong" }, FORM[m.form]),
    m.manufacturer && el("span", { class: "tag" }, /asus/i.test(m.manufacturer) ? "ASUS" : m.manufacturer.split(" ")[0]),
    discrete && el("span", { class: "tag" }, VENDOR[discrete.vendor] || discrete.name),
    inv.gpus.some((g) => g.discrete) && inv.gpus.some((g) => !g.discrete) && el("span", { class: "tag" }, "hybrid graphics"),
    inv.power.battery && el("span", { class: "tag" }, inv.power.battery.onAc ? "on AC" : "on battery"),
  ];
  const bios = m.biosVersion && `BIOS ${m.biosVersion}${m.biosDate ? ` (${m.biosDate})` : ""}`;
  return el("div", { class: "machine" },
    el("div", { class: "maker" }, m.manufacturer || "Unknown maker"),
    el("h1", {}, name || "Unknown model"),
    el("div", { class: "tags" }, tags),
    el("div", { class: "os" }, join(inv.os.caption, inv.os.build && `build ${inv.os.build}`, m.board && `board ${m.board}`, bios)));
}

function cpuCard(cpu) {
  return card("Processor", [
    fact("Model", cpu.name),
    fact("Cores", cpu.cores && `${cpu.cores} cores, ${cpu.threads} threads`),
    fact("Base clock", cpu.baseMhz && `${(cpu.baseMhz / 1000).toFixed(2)} GHz`),
  ]);
}

function gpuCard(gpus) {
  if (!gpus.length) return card("Graphics", [fact("Cards", "none found")]);
  const rows = gpus.flatMap((g) => [
    head(g.name),
    fact("Type", join(g.discrete ? "discrete" : "integrated", VENDOR[g.vendor])),
    fact("Memory", g.vramBytes && memorySize(g.vramBytes)),
    fact("Driver", g.driver),
    fact("Display", g.width && `${g.width}×${g.height}${g.refreshHz ? ` at ${g.refreshHz} Hz` : ""}`),
  ]);
  return card("Graphics", rows);
}

/** Identical modules counted once: "8 × 4 GB". */
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

function memoryCard(memory) {
  // Windows' total leaves out what the firmware and the integrated graphics keep: the
  // modules give what is installed.
  const installed = memory.modules.reduce((sum, m) => sum + m.capacityBytes, 0);
  const usable = memory.totalBytes && memorySize(memory.totalBytes);
  const rows = [fact("Total", installed ? join(memorySize(installed), usable && `${usable} usable`) : usable)];
  for (const g of moduleGroups(memory.modules)) {
    const speed = g.configuredMhz && g.ratedMhz > g.configuredMhz
      ? `${g.configuredMhz} MT/s (rated ${g.ratedMhz})`
      : g.configuredMhz && `${g.configuredMhz} MT/s`;
    rows.push(
      fact("Modules", join(`${g.count} × ${memorySize(g.capacityBytes)}`, g.kind, g.soldered && "soldered")),
      fact("Speed", speed),
      fact("Made by", join(g.manufacturer, g.part)));
  }
  return card("Memory", rows);
}

function storageCard(disks) {
  const rows = disks.flatMap((d) => [
    head(d.name || "Unnamed disk"),
    fact("Kind", join(d.bus, d.media, diskSize(d.sizeBytes))),
    d.system && fact("Holds", "Windows"),
  ]);
  return card("Storage", rows.length ? rows : [fact("Disks", "none found")]);
}

function powerCard(power, form) {
  const b = power.battery;
  return card("Power", [
    fact("Plan", power.planName),
    fact("Source", b ? join(b.onAc ? "plugged in" : "on battery", b.chargePercent != null && `${b.chargePercent} %`) : form === "laptop" ? "battery not read" : "no battery"),
  ]);
}

function toolsCard(tools, isAsus) {
  const rows = [fact("Chocolatey", tools.chocolatey ? "installed" : "not installed")];
  if (isAsus) {
    rows.push(fact("G-Helper", tools.ghelperRunning ? "running" : "not running"));
    rows.push(fact("ASUS services", tools.asusServices.length
      ? tools.asusServices.map(([name, mode]) => `${name} (${mode.toLowerCase()})`).join(", ")
      : "none"));
  }
  return card("Tools", rows);
}

function findingsCard(findings) {
  const items = findings.map((f) => el("li", { class: f.level },
    el("span", { class: "mark", title: f.level }, MARK[f.level]),
    el("span", { class: "area" }, f.area),
    el("div", { class: "body" }, el("div", { class: "title" }, f.title), el("div", { class: "detail" }, f.detail))));
  return el("section", { class: "card wide" },
    el("h2", {}, "What tuning this machine takes into account"),
    items.length ? el("ul", { class: "findings" }, items) : el("p", { class: "dim" }, "Nothing particular."),
    el("p", { class: "legend" }, "! holds the machine back  → worth changing  · taken into account. Nothing is changed yet: this is a reading."));
}

function render({ inventory: inv, findings }) {
  const isAsus = /asus/i.test(inv.machine.manufacturer);
  const content = $("content");
  const parts = [
    machineBlock(inv),
    el("div", { class: "cards" },
      findingsCard(findings),
      cpuCard(inv.cpu),
      gpuCard(inv.gpus),
      memoryCard(inv.memory),
      storageCard(inv.disks),
      powerCard(inv.power, inv.machine.form),
      toolsCard(inv.tools, isAsus)),
    inv.warnings.length ? el("section", { class: "warnings" },
      el("h2", {}, "Not read"),
      el("ul", {}, inv.warnings.map((w) => el("li", {}, w)))) : null,
  ];
  content.replaceChildren(...parts.filter(Boolean));
  $("status").textContent = join(inv.machine.family || inv.machine.model, FORM[inv.machine.form]);
}

// --- scanning ------------------------------------------------------------------------

function scan() {
  if (window.__TAURI__) return window.__TAURI__.core.invoke("scan");
  return fetch("mock.json").then((r) => {
    if (!r.ok) throw new Error(`mock.json: HTTP ${r.status}`);
    return r.json();
  });
}

async function refresh() {
  const button = $("btn-rescan");
  button.disabled = true;
  $("status").textContent = "reading the machine…";
  if (!$("content").firstChild) {
    $("content").append(el("div", { class: "empty" }, el("span", { class: "live-dot" }, "●"), "Reading the machine…"));
  }
  try {
    render(await scan());
  } catch (e) {
    $("status").textContent = "";
    $("content").replaceChildren(el("p", { class: "error" }, `The machine could not be read: ${e?.message || e}`));
  } finally {
    button.disabled = false;
  }
}

$("btn-rescan").addEventListener("click", refresh);
refresh();
