import { getVersion } from "@tauri-apps/api/app";
import { api, type Landing, type Layout, type LayoutWindow, type Monitor, type Rect, type RestoreSummary, type Store, type Theme } from "./api";
import { lang, setLang, systemLang, t, type Lang } from "./i18n";
import { icon, type IconName } from "./icons";
import { menu, select, tooltips } from "./sj-ui.js";
import appIcon from "../src-tauri/icons/icon.svg";
import "./sj-ui.css";
import "./style.css";

// CCS/1 M:ui | SJ Utility UI pattern B: side nav = Save current layout + one item per layout | Settings, About
// WHY:B: layouts are master-detail (list beside map + windows), plus Settings and About = 4+ areas
// FLOW:state -> render() -> nav list + head/body innerHTML; clicks via [data-act] delegation; selects mounted into [data-select]
// RULE:sj-ui controls only (no native select/checkbox/title=/alert/confirm); InfoBar (.bars) instead of toasts
// RULE:every string from i18n.ts; data-act/data-i/.win/.sub/.shortcut/.nav-keys/.screen.missing/.infobar[data-restored] = E2E hooks

type Page = "layout" | "settings" | "about";

const SITE = "https://szymonjankiewicz.com";
const REPO = "https://github.com/prejas3/PaneKeeper";
const AUTHOR = "Szymon Jankiewicz"; // i18n-ok: a name
const APP = "PaneKeeper"; // i18n-ok: the app name

let store: Store = {
  layouts: [],
  settings: { autoRestore: false, paused: false, autostart: false, launchWaitSeconds: 8, theme: (stored("theme") as Theme) ?? "system", lang: null },
};
let monitors: Monitor[] = [];
let page: Page = "layout";
let selected: string | null = null;
let landings: (Landing | null)[] = [];
let conflicts = new Set<string>();
let recording = false;
let version = "";
const icons = new Map<string, string | null>();

let nav: HTMLElement;
let navList: HTMLElement;
let content: HTMLElement;
let headEl: HTMLElement;
let bars: HTMLElement;
let body: HTMLElement;
/** Select controls built by the page render, mounted into their [data-select] slot. */
let selects: Record<string, () => HTMLElement> = {};

// ---------- markup helpers (sj-ui.css components) ----------

type Attrs = Record<string, string | number | boolean | undefined>;
const attrs = (a: Attrs = {}) =>
  Object.entries(a).filter(([, v]) => v !== undefined && v !== false).map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${esc(String(v))}"`)).join("");

function btn(label: string, act: string, o: { cls?: string; ic?: IconName; tip?: string; a?: Attrs } = {}) {
  const tip = o.tip ? { "data-tip": o.tip, ...(label ? {} : { "aria-label": o.tip }) } : {};
  return `<button type="button" class="${o.cls ?? "btn"}" data-act="${act}"${attrs({ ...tip, ...o.a })}>${o.ic ? icon(o.ic) : ""}${label ? `<span>${esc(label)}</span>` : ""}</button>`;
}
const iconBtn = (ic: IconName, tip: string, act: string, a: Attrs = {}, cls = "") => btn("", act, { cls: `btn subtle square ${cls}`.trim(), ic, tip, a });

/** Toggle; `state` shows On/Off before the track (settings cards only). */
const toggle = (on: boolean, label: string, act: string, a: Attrs = {}, state = false) =>
  `<button type="button" class="toggle" role="switch" aria-checked="${on}" aria-label="${esc(label)}" data-act="${act}"${attrs(a)}>${state ? `<span>${on ? t.on : t.off}</span>` : ""}<i></i></button>`;

const check = (on: boolean, text: string, label: string, act: string, a: Attrs = {}) =>
  `<button type="button" class="check" role="checkbox" aria-checked="${on}" aria-label="${esc(label)}" data-act="${act}"${attrs(a)}><i>${icon("check")}</i><span>${esc(text)}</span></button>`;

const card = (ic: IconName | null, title: string, text: string, controls: string, cls = "") =>
  `<div class="card ${cls}">${ic ? icon(ic) : ""}<div class="card-text"><h3>${esc(title)}</h3>${text ? `<p>${text}</p>` : ""}</div><div class="card-ctl">${controls}</div></div>`;

const group = (title: string, items: string, aside = "") =>
  `<section class="group"${attrs({ "aria-label": title || undefined })}>${title ? `<h2 class="group-title">${esc(title)}${aside ? ` <span class="t-caption t-2">${esc(aside)}</span>` : ""}</h2>` : ""}${items}</section>`;

function slot(id: string, make: () => HTMLElement) {
  selects[id] = make;
  return `<span data-select="${id}"></span>`;
}

// ---------- shell ----------

function build() {
  document.documentElement.lang = lang;
  const item = (p: Page, ic: IconName) => navItem(p, "go", ic, t[p as "settings" | "about"], { "data-v": p });
  document.body.innerHTML = `
    <nav class="nav" aria-label="${esc(t.nav)}">
      ${iconBtn("menu", t.nav, "nav-menu", { "aria-expanded": "false" }, "menu-btn")}
      <button type="button" class="nav-item save" data-act="capture" aria-label="${esc(t.saveLayout)}">${icon("add", 20)}<span>${esc(t.saveLayout)}</span></button>
      <div class="nav-list" role="list" aria-label="${esc(t.layouts)}"></div>
      ${item("settings", "settings")}${item("about", "about")}
    </nav>
    <main class="content"><div class="page"><header class="page-head"></header><div class="bars"></div><div class="body"></div></div></main>`;
  nav = document.querySelector(".nav")!;
  navList = nav.querySelector(".nav-list")!;
  content = document.querySelector(".content")!;
  headEl = content.querySelector(".page-head")!;
  bars = content.querySelector(".bars")!;
  body = content.querySelector(".body")!;
  render();
}

function navItem(key: string, act: string, ic: IconName, label: string, a: Attrs, extra = "") {
  return `<button type="button" class="nav-item" data-act="${act}"${attrs({ "aria-label": label, ...a })} data-key="${esc(key)}">${icon(ic).replace('class="icon"', 'class="icon off"')}${icon(`${ic}On` as IconName).replace('class="icon"', 'class="icon on"')}<span class="nav-name">${esc(label)}</span>${extra}</button>`;
}

const compact = matchMedia("(width < 640px)");
compact.addEventListener("change", navTips);
/** Below 640 px the nav shows icons only: its labels become tooltips. */
function navTips() {
  nav?.querySelectorAll<HTMLElement>(".nav-item").forEach((b) => {
    if (compact.matches) b.dataset.tip = b.getAttribute("aria-label") ?? "";
    else delete b.dataset.tip;
  });
}

function render() {
  if (!body) return;
  const focus = focusKey(document.activeElement);
  const current = layout();
  navList.innerHTML = store.layouts
    .map((l) => navItem(l.id, "select", "layout", l.name, { "data-id": l.id, role: "listitem" },
      l.shortcut ? `<span class="nav-keys${conflicts.has(l.id) ? " taken" : ""}">${esc(l.shortcut)}</span>` : ""))
    .join("");
  const here = page === "layout" ? (current ? `[data-id="${CSS.escape(current.id)}"]` : "") : `[data-v="${page}"]`;
  nav.querySelectorAll(".nav-item").forEach((b) => b.removeAttribute("aria-current"));
  if (here) nav.querySelector(here)?.setAttribute("aria-current", "page");
  selects = {};
  const [title, lede, actions, html] = page === "layout" ? (current ? layoutPage(current) : emptyPage()) : pages[page]();
  headEl.innerHTML = `<div>${title}${lede ? `<p class="t-2 lede">${esc(lede)}</p>` : ""}</div>${actions ? `<div class="head-actions">${actions}</div>` : ""}`;
  body.innerHTML = html;
  for (const el of content.querySelectorAll<HTMLElement>("[data-select]")) el.replaceWith(selects[el.dataset.select!]());
  navTips();
  pausedBar();
  if (focus) document.querySelector<HTMLElement>(focus)?.focus();
}

/** A selector that finds the same control after a re-render, so keyboard focus stays put. */
function focusKey(el: Element | null) {
  if (!(el instanceof HTMLElement) || !document.body.contains(el) || el.closest(".infobar")) return "";
  const keys = ["data-act", "data-i", "data-id", "data-v", "data-field", "aria-label"].filter((k) => el.hasAttribute(k));
  return keys.length ? keys.map((k) => `[${k}="${CSS.escape(el.getAttribute(k)!)}"]`).join("") : "";
}

function setPage(next: Page, id: string | null = selected) {
  if (next !== page || id !== selected) bars.querySelectorAll(".infobar:not(#paused)").forEach((b) => b.remove());
  page = next;
  recording = false;
  nav.classList.remove("open");
  nav.querySelector('[data-act="nav-menu"]')?.setAttribute("aria-expanded", "false");
  if (next === "layout" && id !== selected) {
    selected = id;
    void refreshPreview();
  } else render();
  content.scrollTop = 0;
}

const title = (text: string) => `<h1 class="t-title">${esc(text)}</h1>`;

// ---------- pages: [title html, lede, head actions, body] ----------

function emptyPage(): [string, string, string, string] {
  return [title(t.layouts), t.emptyText, btn(t.saveLayout, "capture", { cls: "btn accent", ic: "add" }), ""];
}

function layoutPage(l: Layout): [string, string, string, string] {
  const included = l.windows.filter((w) => w.included).length;
  const conflict = conflicts.has(l.id);
  const name = `<label class="title-field"><input class="t-title" data-field="rename" value="${esc(l.name)}" aria-label="${esc(t.layoutName)}" spellcheck="false" maxlength="60" autocomplete="off"></label>`;
  const actions =
    btn(t.update, "update", { ic: "sync", tip: t.updateTip }) +
    btn(t.restore, "restore", { cls: "btn accent", ic: "play", a: { disabled: !included } }) +
    iconBtn("more", t.more, "more", { "aria-haspopup": "menu" });
  const keys = recording
    ? `<span>${esc(t.shortcutPress)}</span>`
    : l.shortcut ? `<span class="keys">${l.shortcut.split("+").map((k) => `<kbd>${esc(k)}</kbd>`).join("")}</span>` : `<span class="t-2">${esc(t.shortcutNone)}</span>`;
  const shortcut = `<button type="button" class="btn shortcut${recording ? " rec" : ""}${conflict ? " taken" : ""}" data-act="record"
      aria-label="${esc(t.shortcutLabel(l.shortcut ?? t.none))}">${keys}${conflict && !recording ? icon("warn") : ""}</button>`;
  const shortcutText = recording ? esc(t.shortcutHelp) : conflict ? `<span class="status err">${esc(t.shortcutTaken)}</span>` : esc(t.shortcutText);
  const bindText = l.bound ? (store.settings.autoRestore ? t.bindOn : t.bindNeedsAuto) : t.bindText;
  const rows = l.windows.length
    ? `<ul class="list wins">${l.windows.map((w, i) => windowRow(w, i)).join("")}</ul>`
    : `<p class="t-2 pad">${esc(t.noWindows)}</p>`;
  return [name, t.included(included, l.windows.length), actions,
    group("",
      card("keyboard", t.shortcut, shortcutText, shortcut) +
      card("displaySync", t.bind, esc(bindText), toggle(l.bound, t.bind, "bind", {}, true))) +
    `<section class="surface map-card" aria-label="${esc(t.mapLabel)}">${map(l)}</section>` +
    group(t.windowsTitle, `<div class="surface">${rows}</div>`)];
}

function windowRow(entry: LayoutWindow, i: number) {
  const w = entry.window;
  const app = appName(w.exe);
  const target = landings[i] ? monitors.find((m) => m.id === landings[i]!.monitorId) : null;
  const moved = target && target.id !== w.monitor.id;
  const where = `<span class="where"${attrs({ "data-tip": moved ? t.movedTip(w.monitor.name, target!.name) : undefined, tabindex: moved ? 0 : undefined })}>${moved ? icon("warn") : icon("monitor")}<span>${esc(monitorLabel(moved ? target! : w.monitor))} · ${esc(t.states[w.showState])}</span></span>`;
  return `<li class="list-row win${entry.included ? "" : " off"}" data-w="${i}">
    ${appIconHtml(w.exe)}
    <span class="win-text"><span class="win-name">${esc(app)}</span><span class="sub t-caption t-2">${esc(w.title)}</span>${where}</span>
    <span class="win-ctl">
      ${check(entry.included, t.include, t.includeApp(app), "include", { "data-i": i })}
      ${check(!!entry.launch, t.launch, t.launchApp(app), "launch", { "data-i": i, "data-tip": t.launchTip, disabled: !entry.included })}
      ${iconBtn("dismiss", t.removeWindow(app), "remove-window", { "data-i": i })}
    </span>
  </li>`;
}

// Monitors to scale in their real arrangement; saved monitors missing now are dashed.
function map(l: Layout) {
  const saved = new Map(l.windows.map((w) => [w.window.monitor.id, w.window.monitor]));
  const missing = [...saved.values()].filter((m) => !monitors.some((c) => c.id === m.id));
  const all = [...monitors, ...missing];
  if (!all.length) return `<p class="t-2 pad">${esc(t.noMonitors)}</p>`;
  const box = all.reduce(
    (b, m) => ({ left: Math.min(b.left, m.bounds.left), top: Math.min(b.top, m.bounds.top), right: Math.max(b.right, m.bounds.right), bottom: Math.max(b.bottom, m.bounds.bottom) }),
    { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity },
  );
  const W = box.right - box.left;
  const H = box.bottom - box.top;
  const pad = Math.max(W, H) * 0.02;
  // ~12px on screen: the map is drawn about this wide (card width, capped by its 300px height).
  const drawn = Math.min((body?.clientWidth ?? 800) - 30, (300 * (W + pad * 2)) / (H + pad * 2));
  const font = (12 * (W + pad * 2)) / Math.max(200, drawn);
  const primary = monitors.find((m) => m.primary) ?? monitors[0];
  const screen = (m: Monitor, kind: string) => {
    const r = m.bounds;
    return `<rect class="screen ${kind}" x="${r.left}" y="${r.top}" width="${r.right - r.left}" height="${r.bottom - r.top}" rx="${font * 0.35}" vector-effect="non-scaling-stroke"/>`;
  };
  // Labels go on top of the windows, with a halo so they read over anything.
  const label = (m: Monitor, kind: string) => {
    const r = m.bounds;
    const scale = Math.round((m.dpi / 96) * 100);
    const lines = kind === "missing"
      ? [m.name, primary ? t.notConnectedUses(primary.name) : t.notConnected]
      : [`${monitorNumber(m)}  ${m.name}`, `${r.right - r.left} × ${r.bottom - r.top} · ${scale}%`];
    const max = (r.right - r.left - font * 1.2) / (font * 0.5);
    return `<g class="label ${kind}" stroke-width="${font * 0.32}">${lines
      .map((text, n) => `<text x="${r.left + font * 0.6}" y="${r.bottom - font * (0.6 + (lines.length - 1 - n) * 1.25)}" font-size="${font * (n ? 0.78 : 0.9)}" class="${n ? "label-sub" : "label-name"}">${esc(clip(text, max * (n ? 0.9 / 0.78 : 1)))}</text>`)
      .join("")}</g>`;
  };
  const windows = l.windows
    .map((entry, i) => ({ entry, i, land: landings[i] }))
    .filter(({ entry, land }) => entry.included && land)
    .sort((a, b) => b.entry.window.zOrder - a.entry.window.zOrder)
    .map(({ entry, i, land }) => {
      const target = monitors.find((m) => m.id === land!.monitorId);
      const full: Rect = entry.window.showState === "maximized" && target ? target.workArea : land!.rect;
      const inset = Math.min(font * 0.18, (full.right - full.left) / 8, (full.bottom - full.top) / 8);
      const r: Rect = { left: full.left + inset, top: full.top + inset, right: full.right - inset, bottom: full.bottom - inset };
      const w = r.right - r.left;
      const h = r.bottom - r.top;
      const s = Math.min(font * 1.5, h * 0.4, w * 0.3);
      const gap = s * 0.4;
      const url = icons.get(entry.window.exe);
      return `<g class="map-win ${entry.window.showState}" data-w="${i}">
        <rect x="${r.left}" y="${r.top}" width="${w}" height="${h}" rx="${font * 0.2}" vector-effect="non-scaling-stroke"/>
        <image data-icon="${esc(entry.window.exe)}" x="${r.left + gap}" y="${r.top + gap}" width="${s}" height="${s}"${url ? ` href="${url}"` : ""}/>
        <text x="${r.left + gap * 2 + s}" y="${r.top + gap + s * 0.72}" font-size="${font * 0.85}">${esc(clip(appName(entry.window.exe), (w - s - gap * 3) / (font * 0.5)))}</text>
      </g>`;
    })
    .join("");
  // i18n-ok-start: SVG markup
  return `<svg class="map" viewBox="${box.left - pad} ${box.top - pad} ${W + pad * 2} ${H + pad * 2}" role="img" aria-label="${esc(t.mapLabel)}">
    ${monitors.map((m) => screen(m, "")).join("")}${missing.map((m) => screen(m, "missing")).join("")}${windows}
    ${monitors.map((m) => label(m, "")).join("")}${missing.map((m) => label(m, "missing")).join("")}
  </svg>`;
  // i18n-ok-end
}

const pages: Record<"settings" | "about", () => [string, string, string, string]> = {
  settings: () => {
    const s = store.settings;
    const langSel = slot("lang", () => select({
      options: [{ value: "pl", label: "Polski" }, { value: "en", label: "English" }], // i18n-ok: language names
      value: lang, label: t.language, onChange: (v: string) => void changeLang(v as Lang),
    }));
    const themeSel = slot("theme", () => select({
      options: (["system", "light", "dark"] as Theme[]).map((v) => ({ value: v, label: t.themes[v] })),
      value: s.theme, label: t.theme,
      onChange: (v: string) => { s.theme = v as Theme; remember("theme", v); applyTheme(); void saveSettings(); },
    }));
    const wait = `<label class="field number"><input data-field="wait" value="${s.launchWaitSeconds}" inputmode="numeric" maxlength="2" aria-label="${esc(t.launchWaitLabel)}" autocomplete="off"><span class="suffix">${esc(t.seconds)}</span>
      <span class="spin"><button type="button" tabindex="-1" class="up" data-act="wait" data-v="1" aria-label="${esc(t.secMore)}">${icon("chevron")}</button><button type="button" tabindex="-1" data-act="wait" data-v="-1" aria-label="${esc(t.secLess)}">${icon("chevron")}</button></span></label>`;
    return [title(t.settings), "", "", [
      group(t.general,
        card("power", t.startWithWindows, esc(t.startText), toggle(s.autostart, t.startWithWindows, "set-autostart", {}, true)) +
        card("language", t.language, esc(t.languageText), langSel) +
        card("theme", t.theme, esc(t.themeText), themeSel)),
      group(APP,
        card("displaySync", t.autoRestore, esc(t.autoRestoreText), toggle(s.autoRestore, t.autoRestore, "set-auto", {}, true)) +
        card("pause", t.pause, esc(t.pauseText), toggle(s.paused, t.pause, "set-paused", {}, true)) +
        card("timer", t.launchWait, esc(t.launchWaitText), wait)),
      group(t.advanced,
        card("folderOpen", t.dataFolder, esc(t.dataFolderText), btn(t.openFolder, "open", { a: { "data-k": "data" } }))),
    ].join("")];
  },
  about: () => {
    const link = (ic: IconName, label: string, url: string) =>
      `<button type="button" class="card" data-act="open" data-k="${esc(url)}">${icon(ic)}<span class="card-text"><span>${esc(label)}</span><p>${esc(url.replace("https://", ""))}</p></span>${icon("open")}</button>`;
    return ["", "", "", `
      <div class="about-head"><img src="${appIcon}" width="48" height="48" alt="">
        <div><h1 class="t-subtitle">${APP}</h1><p class="t-caption t-2">${esc(version ? t.version(version) : "")}</p></div></div>
      <p class="t-2">${esc(t.aboutText)} ${esc(t.safety)}</p>
      <p><button type="button" class="link" data-act="open" data-k="${SITE}">${AUTHOR}</button></p>
      ${group("", link("globe", t.projectPage, `${SITE}/p/panekeeper`) + link("code", t.source, REPO) + link("bug", t.issues, `${REPO}/issues`) + link("document", t.licence, `${REPO}/blob/main/LICENSE`))}`];
  },
};

// ---------- app icons ----------

function appIconHtml(exe: string) {
  void loadIcon(exe);
  const url = icons.get(exe);
  return url
    ? `<img class="win-icon" data-icon="${esc(exe)}" src="${url}" alt="">`
    : `<span class="win-icon mono" data-icon="${esc(exe)}" aria-hidden="true">${esc(appName(exe).slice(0, 1).toUpperCase())}</span>`;
}

/** Fills icons in place once loaded: a re-render would reset a name being typed. */
async function loadIcon(exe: string) {
  if (icons.has(exe)) return;
  icons.set(exe, null);
  const got = await api.icon(exe).catch(() => null);
  if (!got) return;
  const canvas = Object.assign(document.createElement("canvas"), { width: got.size, height: got.size });
  canvas.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(got.rgba), got.size, got.size), 0, 0);
  const url = canvas.toDataURL();
  icons.set(exe, url);
  for (const el of document.querySelectorAll<HTMLElement | SVGElement>(`[data-icon="${CSS.escape(exe)}"]`)) {
    if (el instanceof SVGImageElement) el.setAttribute("href", url);
    else if (el instanceof HTMLImageElement) el.src = url;
    else el.replaceWith(Object.assign(document.createElement("img"), { className: "win-icon", src: url, alt: "" }));
  }
}

// ---------- text ----------

const layout = () => store.layouts.find((l) => l.id === selected) ?? null;
const appName = (exe: string) => {
  const file = exe.split(/[\\/]/).pop() ?? exe;
  const stem = file.replace(/\.exe$/i, "");
  return stem.charAt(0).toUpperCase() + stem.slice(1);
};
// Monitors are numbered left to right, then top to bottom, on the map and in the list alike.
const monitorNumber = (m: Monitor) =>
  [...monitors].sort((a, b) => a.bounds.left - b.bounds.left || a.bounds.top - b.bounds.top).findIndex((c) => c.id === m.id) + 1;
const monitorLabel = (m: Monitor) => (monitors.some((c) => c.id === m.id) ? `${monitorNumber(m)} · ${m.name}` : m.name);
const clip = (text: string, max: number) => (max < 2 ? "" : text.length > max ? `${text.slice(0, Math.max(1, Math.floor(max) - 1))}…` : text);

function esc(s: string) {
  return s.replace(/[&<>\x22\x27]/g, (c) => `&#${c.charCodeAt(0)};`); // \x22\x27 = quotes, kept out of the i18n scanner
}

function stored(key: string) {
  try { return localStorage.getItem(key); } catch { return null; }
}
function remember(key: string, value: string) {
  try { localStorage.setItem(key, value); } catch { /* private mode */ }
}

// ---------- InfoBar ----------

type Kind = "ok" | "warn" | "err" | "info";
/** InfoBar (spec 6), the stand-in for toasts. `id` replaces that bar; `sticky` has no ×. */
function say(kind: Kind, heading: string, text = "", o: { id?: string; sticky?: boolean; action?: [string, () => void]; data?: Attrs } = {}) {
  if (!bars) return;
  if (o.id) bars.querySelector(`#${o.id}`)?.remove();
  const bar = document.createElement("div");
  bar.className = `infobar ${kind}`;
  bar.setAttribute("role", kind === "err" ? "alert" : "status");
  if (o.id) bar.id = o.id;
  for (const [k, v] of Object.entries(o.data ?? {})) bar.setAttribute(k, String(v));
  bar.innerHTML = `${icon(kind)}<p class="infobar-text"><b>${esc(heading)}</b><span>${esc(text)}</span></p>`;
  if (o.action) {
    const [label, run] = o.action;
    const b = Object.assign(document.createElement("button"), { type: "button", className: "btn", textContent: label });
    b.addEventListener("click", () => { if (!o.sticky) bar.remove(); run(); });
    bar.append(b);
  }
  if (!o.sticky) {
    bar.insertAdjacentHTML("beforeend", iconBtn("dismiss", t.dismiss, "noop"));
    bar.lastElementChild!.addEventListener("click", () => bar.remove());
  }
  bars.append(bar);
  // Keep the stack short: the oldest closable bars go first.
  const closable = [...bars.children].filter((el) => el.id !== "paused");
  closable.slice(0, Math.max(0, closable.length - 3)).forEach((el) => el.remove());
}

const fail = (heading: string) => (error: unknown) => say("err", heading, String(error));

function pausedBar() {
  if (!store.settings.paused || page !== "layout") bars.querySelector("#paused")?.remove();
  else if (!bars.querySelector("#paused")) say("warn", t.pausedTitle, t.pausedText, { id: "paused", sticky: true, action: [t.resume, () => void setPaused(false)] });
}

function restored(s: RestoreSummary) {
  const clean = !s.notRunning && !s.blocked.length;
  const parts = [t.restoredN(s.restored), t.notRunningN(s.notRunning), t.blockedN(s.blocked.length)];
  const only = s.blocked.length === 1 ? ` (${s.blocked[0].title}: ${s.blocked[0].error})` : "";
  say(clean ? "ok" : "warn", clean ? t.restoreDone : t.restorePartly, parts.join(" · ") + only, {
    id: "restore",
    data: { "data-restored": s.restored, "data-not-running": s.notRunning, "data-blocked": s.blocked.length },
  });
}

// ---------- data ----------

async function load(id?: string) {
  try {
    [store, monitors] = await Promise.all([api.store(), api.monitors()]);
    conflicts = new Set((await api.conflicts()).map(([layoutId]) => layoutId));
  } catch (error) {
    fail(t.loadFailed)(error);
  }
  if (id) selected = id;
  if (!store.layouts.some((l) => l.id === selected)) selected = store.layouts[0]?.id ?? null;
  if (store.settings.theme !== stored("theme")) { remember("theme", store.settings.theme); applyTheme(); }
  await refreshPreview();
}

async function refreshPreview() {
  const current = layout();
  landings = current ? await api.preview(current).catch(() => []) : [];
  render();
}

async function saveLayout(l: Layout, preview = false) {
  await api.save(l).catch(fail(t.saveFailed));
  if (preview) await refreshPreview();
  else render();
}

async function saveSettings() {
  try {
    await api.saveSettings(store.settings);
  } catch (error) {
    fail(t.saveFailed)(error);
    await load();
    return;
  }
  render();
}

const setPaused = (on: boolean) => { store.settings.paused = on; return saveSettings(); };

function applyTheme() {
  const theme = store.settings.theme;
  if (theme === "light" || theme === "dark") document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme; // the native frame + Mica follow in the backend (save_settings)
}

async function changeLang(next: Lang) {
  setLang(next);
  store.settings.lang = next;
  remember("lang", next);
  build();
  await api.saveSettings(store.settings).catch(fail(t.saveFailed));
  content.querySelector<HTMLElement>(`[aria-label="${CSS.escape(t.language)}"]`)?.focus();
}

async function capture() {
  try {
    const created = await api.capture(t.newLayoutName(store.layouts.length + 1));
    page = "layout";
    await load(created.id);
    say("ok", t.saved(created.windows.length), t.nameIt);
    const input = content.querySelector<HTMLInputElement>('[data-field="rename"]');
    input?.focus();
    input?.select();
  } catch (error) {
    fail(t.actionFailed)(error);
  }
}

const clampWait = (n: number) => Math.min(60, Math.max(1, Math.round(n) || 1));
const setWait = (n: number) => { store.settings.launchWaitSeconds = clampWait(n); return saveSettings(); };

// ---------- actions ----------

const actions: Record<string, (el: HTMLElement) => unknown> = {
  go: (el) => setPage(el.dataset.v as Page),
  select: (el) => setPage("layout", el.dataset.id!),
  "nav-menu": (el) => el.setAttribute("aria-expanded", String(nav.classList.toggle("open"))),
  capture: () => capture(),
  restore: () => { const l = layout(); if (l) return api.restore(l.id).catch(fail(t.actionFailed)); },
  update: async () => {
    const l = layout();
    if (!l) return;
    const updated = await api.update(l.id);
    await load(updated.id);
    say("ok", t.updated(updated.windows.length));
  },
  more: (el) => {
    const l = layout();
    if (!l) return;
    menu(el, [
      { label: t.duplicate, icon: icon("copy"), run: () => void api.duplicate(l.id).then((copy) => load(copy.id)).catch(fail(t.actionFailed)) },
      { label: t.delete, icon: icon("delete"), run: () => void removeLayout(l) },
    ], t.more);
  },
  record: () => {
    recording = !recording;
    render();
    if (recording) content.querySelector<HTMLElement>(".shortcut")?.focus();
  },
  bind: async () => {
    const l = layout();
    if (!l) return;
    Object.assign(l, await api.bind(l.id, !l.bound));
    render();
  },
  include: (el) => {
    const l = layout()!;
    const w = l.windows[+el.dataset.i!];
    w.included = !w.included;
    return saveLayout(l, true);
  },
  launch: (el) => {
    const l = layout()!;
    const w = l.windows[+el.dataset.i!];
    w.launch = w.launch ? null : { args: [], workingDir: null, waitSeconds: store.settings.launchWaitSeconds };
    return saveLayout(l);
  },
  "remove-window": (el) => {
    const l = layout()!;
    l.windows.splice(+el.dataset.i!, 1);
    return saveLayout(l, true);
  },
  "set-autostart": () => { store.settings.autostart = !store.settings.autostart; return saveSettings(); },
  "set-auto": () => { store.settings.autoRestore = !store.settings.autoRestore; return saveSettings(); },
  "set-paused": () => setPaused(!store.settings.paused),
  wait: (el) => setWait(store.settings.launchWaitSeconds + Number(el.dataset.v)),
  open: (el) => api.openExternal(el.dataset.k!).catch(fail(t.openFailed)),
};

/** Delete with Undo (spec 9: confirm only what can't be undone): the layout object is saved back as it was. */
async function removeLayout(l: Layout) {
  await api.remove(l.id);
  selected = null;
  await load();
  say("info", t.deleted(l.name), "", { action: [t.undo, () => void api.save(l).then(() => load(l.id)).catch(fail(t.actionFailed))] });
}

// ---------- events ----------

document.addEventListener("click", async (event) => {
  const el = (event.target as HTMLElement).closest<HTMLElement>("[data-act]");
  if (!el || !document.body.contains(el) || (el as HTMLButtonElement).disabled || el.closest(".infobar")) return;
  try {
    await actions[el.dataset.act!]?.(el);
  } catch (error) {
    fail(t.actionFailed)(error);
  }
});

document.addEventListener("change", (event) => {
  const input = event.target as HTMLInputElement;
  const l = layout();
  if (input.dataset.field === "rename" && l) {
    const name = input.value.trim();
    if (name && name !== l.name) {
      l.name = name;
      void saveLayout(l);
    } else input.value = l.name;
  }
  if (input.dataset.field === "wait") void setWait(Number(input.value));
});

document.addEventListener("keydown", (event) => {
  const el = event.target as HTMLElement;
  if (event.key === "Escape" && nav.classList.contains("open")) { nav.classList.remove("open"); return; }
  if (el.dataset.field === "rename" && (event.key === "Enter" || event.key === "Escape")) {
    if (event.key === "Escape") (el as HTMLInputElement).value = layout()?.name ?? "";
    (el as HTMLInputElement).blur();
    return;
  }
  if (el.dataset.field === "wait" && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
    event.preventDefault();
    void setWait(store.settings.launchWaitSeconds + (event.key === "ArrowUp" ? 1 : -1));
    return;
  }
  if (el.dataset.field === "wait" && event.key === "Enter") { el.blur(); return; }
  if (recording) recordKey(event);
});

// Shortcut field: modifiers + A–Z, 0–9 or F1–F24, the keys the Rust parser (events.rs) accepts.
function recordKey(event: KeyboardEvent) {
  const l = layout();
  if (!l) return;
  event.preventDefault();
  if (event.key === "Escape") {
    recording = false;
    render();
    return;
  }
  if (event.key === "Backspace" || event.key === "Delete") {
    recording = false;
    l.shortcut = null;
    conflicts.delete(l.id);
    void saveLayout(l).then(focusShortcut);
    return;
  }
  const key = /^Key([A-Z])$/.exec(event.code)?.[1] ?? /^(?:Digit|Numpad)(\d)$/.exec(event.code)?.[1] ?? /^(F(?:[1-9]|1\d|2[0-4]))$/.exec(event.code)?.[1];
  const mods = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Win"].filter(Boolean) as string[]; // i18n-ok: key names
  if (!key || (!mods.length && !key.startsWith("F")) || (mods.length === 1 && mods[0] === "Shift")) return;
  recording = false;
  l.shortcut = [...mods, key].join("+");
  const clash = store.layouts.find((o) => o.id !== l.id && o.shortcut === l.shortcut);
  if (clash) {
    say("info", t.shortcutMoved(clash.name, l.shortcut), t.shortcutMovedText(l.name));
    clash.shortcut = null;
    void api.save(clash);
  }
  conflicts.delete(l.id);
  void saveLayout(l).then(focusShortcut);
}
const focusShortcut = () => content.querySelector<HTMLElement>(".shortcut")?.focus();

document.addEventListener("focusout", (event) => {
  if (recording && (event.target as HTMLElement).matches?.(".shortcut")) {
    recording = false;
    render();
  }
});

// Hover links a window on the map with its row.
document.addEventListener("pointerover", (event) => {
  const el = (event.target as Element).closest?.<HTMLElement | SVGElement>("[data-w]");
  for (const hot of document.querySelectorAll(".hot")) hot.classList.remove("hot");
  if (el) for (const twin of document.querySelectorAll(`[data-w="${el.dataset.w}"]`)) twin.classList.add("hot");
});

// ---------- boot ----------

async function boot() {
  setLang((stored("lang") as Lang | null) ?? systemLang());
  tooltips();
  applyTheme();
  void (navigator as any).userAgentData?.getHighEntropyValues(["platformVersion"]).then((ua: { platformVersion: string }) => {
    document.documentElement.classList.toggle("mica", parseInt(ua.platformVersion) >= 13); // Windows 11
  });
  build();
  // The choice lives in layouts.json so the tray reads it too; localStorage only avoids a flash.
  const backend = await api.uiLang().catch(() => null);
  if (backend && backend !== lang) { setLang(backend); remember("lang", backend); build(); }
  void getVersion().then((v) => { version = v; if (page === "about") render(); }).catch(() => {});
  void api.on("restore-result", (s) => restored(s as RestoreSummary));
  void api.on("shortcuts-registered", async () => {
    conflicts = new Set((await api.conflicts()).map(([id]) => id));
    render();
  });
  void api.on("store-changed", () => void load());
  void api.on("save-requested", () => void capture());
  await load();
}

void boot();
