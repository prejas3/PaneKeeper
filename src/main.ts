import { api, type Landing, type Layout, type LayoutWindow, type Monitor, type Rect, type Store } from "./api";
import "./style.css";

// CCS/1 M:ui | FLOW:state -> render() -> innerHTML; clicks via [data-act] delegation
// RULE:custom controls only (switch, segmented, stepper, menu popover, shortcut field, tooltip, confirm dialog, overlay scrollbar)

type View = "layout" | "settings";

let store: Store = { layouts: [], settings: { autoRestore: false, paused: false, autostart: false, launchWaitSeconds: 8, theme: "system" } };
let monitors: Monitor[] = [];
let view: View = "layout";
let selected: string | null = null;
let landings: (Landing | null)[] = [];
let conflicts = new Set<string>();
let recording = false;
const icons = new Map<string, string | null>();

const app = document.querySelector<HTMLElement>("#app")!;

const svg = (d: string) =>
  `<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
const ic = {
  plus: svg('<path d="M12 5v14M5 12h14"/>'),
  play: svg('<path d="M8 5.5v13l10.5-6.5Z"/>'),
  refresh: svg('<path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4h-4"/>'),
  more: svg('<path d="M6 12h.01M12 12h.01M18 12h.01" stroke-width="3"/>'),
  settings: svg('<circle cx="12" cy="12" r="3"/><path d="M12 3.5v2M12 18.5v2M3.5 12h2M18.5 12h2M6 6l1.4 1.4M16.6 16.6 18 18M6 18l1.4-1.4M16.6 7.4 18 6"/>'),
  trash: svg('<path d="M4 7h16M9 7V4.5h6V7M6.5 7l1 13h9l1-13"/>'),
  copy: svg('<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5.5A1.5 1.5 0 0 0 14.5 4h-9A1.5 1.5 0 0 0 4 5.5v9A1.5 1.5 0 0 0 5.5 16H8"/>'),
  x: svg('<path d="M6 6l12 12M18 6 6 18"/>'),
  minus: svg('<path d="M6 12h12"/>'),
  warn: svg('<path d="M12 4 2.5 20h19Z"/><path d="M12 10v4M12 17h.01"/>'),
  monitor: svg('<rect x="3" y="4.5" width="18" height="12" rx="1.5"/><path d="M9 20h6M12 16.5V20"/>'),
  layout: svg('<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M10 4.5v15M10 11h10.5"/>'),
};

// ---------- render ----------

function render() {
  const top = app.querySelector(".scroll")?.scrollTop ?? 0;
  const current = layout();
  app.innerHTML = `
    <nav class="side" aria-label="Layouts">
      <div class="brand">${logo()}<span>WindowsKeeper</span></div>
      <button class="primary save" data-act="capture">${ic.plus}<span>Save current layout</span></button>
      ${store.layouts.length ? `<div class="side-label">Layouts</div>` : ""}
      <ul class="layouts">${store.layouts
        .map(
          (l) => `<li><button class="nav" data-act="select" data-id="${esc(l.id)}" ${view === "layout" && l.id === selected ? 'aria-current="page"' : ""}>
            <span class="nav-name">${esc(l.name)}</span>${l.shortcut ? `<span class="nav-keys ${conflicts.has(l.id) ? "taken" : ""}">${esc(l.shortcut.replaceAll("+", " + "))}</span>` : ""}</button></li>`,
        )
        .join("")}</ul>
      <div class="side-foot">
        ${store.settings.paused ? `<button class="paused" data-act="unpause" data-tip="Shortcuts and auto-restore are paused. Click to resume.">Paused</button>` : ""}
        <button class="nav" data-act="settings" ${view === "settings" ? 'aria-current="page"' : ""}>${ic.settings}<span>Settings</span></button>
      </div>
    </nav>
    <main class="main"><div class="scroll"><div class="page">${
      view === "settings" ? settingsPage() : current ? layoutPage(current) : emptyPage()
    }</div></div><div class="thumb" aria-hidden="true"></div></main>
    <div class="tip" role="tooltip" hidden></div>
    <dialog class="confirm" aria-labelledby="confirm-title"></dialog>`;
  for (const el of app.querySelectorAll<HTMLElement>("[data-anchor]")) {
    el.style.setProperty(el.matches("[popover]") ? "position-anchor" : "anchor-name", `--${el.dataset.anchor}`);
  }
  for (const img of app.querySelectorAll<SVGImageElement | HTMLImageElement>("[data-icon]")) {
    const url = icons.get(img.dataset.icon!);
    if (url) img.setAttribute(img instanceof HTMLImageElement ? "src" : "href", url);
  }
  if (view === "layout") app.querySelector(".scroll")!.scrollTop = top;
  scrollbar();
}

function emptyPage() {
  const step = (n: number, title: string, text: string) =>
    `<li class="step"><span class="step-n">${n}</span><span class="grow"><span class="name">${title}</span><span class="sub">${text}</span></span></li>`;
  return `<section class="empty">
    <h1>Bring your windows back in one keystroke</h1>
    <p class="lede">WindowsKeeper remembers where every app window sits, on every monitor, and puts it all back when you ask.</p>
    <ol class="steps">
      ${step(1, "Arrange your windows", "Browser, editor, chat: put each where you like it, on any monitor.")}
      ${step(2, "Save current layout", "WindowsKeeper notes each window's place, size and monitor.")}
      ${step(3, "Press the shortcut to bring it back", "Ctrl+Alt+1 for your first layout, from any app, even after a dock or a restart.")}
    </ol>
    <button class="primary big" data-act="capture">${ic.plus}<span>Save current layout</span></button>
  </section>`;
}

function layoutPage(l: Layout) {
  const included = l.windows.filter((w) => w.included).length;
  const more = menu("more", ic.more, "icon-btn", "More actions", [
    ["duplicate", ic.copy, "Duplicate"],
    ["delete", ic.trash, "Delete…"],
  ]);
  const conflict = conflicts.has(l.id);
  return `
    <header class="head">
      <input class="title" data-act="rename" value="${esc(l.name)}" aria-label="Layout name" spellcheck="false" maxlength="60" />
      <div class="head-tools">
        <button class="ghost" data-act="update" data-tip="Replace the saved windows with how they sit right now. Include and launch choices are kept.">${ic.refresh}<span>Update from current</span></button>
        <button class="primary" data-act="restore" ${included ? "" : "disabled"}>${ic.play}<span>Restore now</span></button>
        ${more}
      </div>
    </header>
    <div class="facts">
      <div class="fact">
        <span class="fact-label">Shortcut</span>
        <button class="shortcut ${recording ? "rec" : ""} ${conflict ? "taken" : ""}" data-act="record" aria-label="Shortcut: ${esc(l.shortcut ?? "none")}. Press to change."
          ${conflict ? 'data-tip="Another app already uses this shortcut. Pick a different one."' : ""}>
          ${recording ? `<span class="rec-text">Press keys… <span class="sub">Esc cancels · Backspace clears</span></span>` : l.shortcut ? keys(l.shortcut, conflict) : `<span class="sub">None. Click to set</span>`}
          ${conflict && !recording ? ic.warn : ""}
        </button>
      </div>
      <div class="fact">
        <span class="fact-label">Bind to this monitor setup</span>
        <span class="fact-row">${toggle(l.bound, "bind", "", "Bind to this monitor setup")}
          <span class="sub">${l.bound ? (store.settings.autoRestore ? "Restores by itself when these monitors connect" : "Turn on auto-restore in Settings to use it") : "Off"}</span></span>
      </div>
    </div>
    <section class="card map-card" aria-label="Monitor map">${map(l)}</section>
    <section class="card windows" aria-label="Windows">
      <div class="list-head"><span class="name">Windows</span><span class="sub">${included} of ${l.windows.length} included</span></div>
      ${l.windows.length ? `<ul>${l.windows.map((w, i) => windowRow(w, i)).join("")}</ul>` : `<p class="sub pad">No windows saved. Use “Update from current”.</p>`}
    </section>`;
}

function windowRow(entry: LayoutWindow, i: number) {
  const w = entry.window;
  const state = { normal: "Normal", maximized: "Maximised", minimized: "Minimised" }[w.showState];
  const target = landings[i] ? monitors.find((m) => m.id === landings[i]!.monitorId) : null;
  const moved = target && target.id !== w.monitor.id;
  return `<li class="win ${entry.included ? "" : "off"}" data-w="${i}">
    ${appIcon(w.exe, "win-icon")}
    <span class="grow"><span class="name">${esc(appName(w.exe))}</span><span class="sub">${esc(w.title)}</span></span>
    <span class="win-meta">
      <span class="badge" ${moved ? `data-tip="${esc(w.monitor.name)} isn't connected. This window goes to ${esc(target!.name)}."` : ""}>${ic.monitor}<span>${esc(monitorLabel(moved ? target! : w.monitor))}</span>${moved ? ic.warn : ""}</span>
      <span class="badge">${state}</span>
    </span>
    <span class="win-toggles">
      <span class="pair">${toggle(entry.included, "include", String(i), `Include ${appName(w.exe)}`)}<span class="sub">Include</span></span>
      <span class="pair" data-tip="Start the app if it isn't running, then place its window">${toggle(!!entry.launch, "launch", String(i), `Launch ${appName(w.exe)} if not running`, !entry.included)}<span class="sub">Launch</span></span>
    </span>
    <button class="icon-btn" data-act="remove-window" data-i="${i}" aria-label="Remove ${esc(appName(w.exe))} from this layout">${ic.x}</button>
  </li>`;
}

// Monitors to scale in their real arrangement; saved monitors missing now are dashed.
function map(l: Layout) {
  const saved = new Map(l.windows.map((w) => [w.window.monitor.id, w.window.monitor]));
  const missing = [...saved.values()].filter((m) => !monitors.some((c) => c.id === m.id));
  const all = [...monitors, ...missing];
  if (!all.length) return `<p class="sub pad">No monitors found.</p>`;
  const box = all.reduce(
    (b, m) => ({ left: Math.min(b.left, m.bounds.left), top: Math.min(b.top, m.bounds.top), right: Math.max(b.right, m.bounds.right), bottom: Math.max(b.bottom, m.bounds.bottom) }),
    { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity },
  );
  const W = box.right - box.left;
  const H = box.bottom - box.top;
  const pad = Math.max(W, H) * 0.02;
  // ~12px on screen: the map is drawn about this wide (card width, capped by its 300px height).
  const drawn = Math.min((app.querySelector(".page")?.clientWidth ?? 800) - 30, (300 * (W + pad * 2)) / (H + pad * 2));
  const font = (12 * (W + pad * 2)) / Math.max(200, drawn);
  const primary = monitors.find((m) => m.primary) ?? monitors[0];
  const screen = (m: Monitor, kind: string) => {
    const r = m.bounds;
    return `<rect class="screen ${kind}" x="${r.left}" y="${r.top}" width="${r.right - r.left}" height="${r.bottom - r.top}" rx="${font * 0.6}" vector-effect="non-scaling-stroke"/>`;
  };
  // Labels go on top of the windows, with a halo so they read over anything.
  const label = (m: Monitor, kind: string) => {
    const r = m.bounds;
    const scale = Math.round((m.dpi / 96) * 100);
    const lines = kind === "missing"
      ? [m.name, primary ? `Not connected · uses ${primary.name}` : "Not connected"]
      : [`${monitorNumber(m)}  ${m.name}`, `${r.right - r.left} × ${r.bottom - r.top} · ${scale}%`];
    const max = (r.right - r.left - font * 1.2) / (font * 0.5);
    return `<g class="label ${kind}" stroke-width="${font * 0.32}">${lines
      .map((t, n) => `<text x="${r.left + font * 0.6}" y="${r.bottom - font * (0.6 + (lines.length - 1 - n) * 1.25)}" font-size="${font * (n ? 0.78 : 0.9)}" class="${n ? "label-sub" : "label-name"}">${esc(clip(t, max * (n ? 0.9 / 0.78 : 1)))}</text>`)
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
      return `<g class="map-win ${entry.window.showState}" data-w="${i}">
        <rect x="${r.left}" y="${r.top}" width="${w}" height="${h}" rx="${font * 0.35}" vector-effect="non-scaling-stroke"/>
        <image data-icon="${esc(entry.window.exe)}" x="${r.left + gap}" y="${r.top + gap}" width="${s}" height="${s}"/>
        <text x="${r.left + gap * 2 + s}" y="${r.top + gap + s * 0.72}" font-size="${font * 0.85}">${esc(clip(appName(entry.window.exe), (w - s - gap * 3) / (font * 0.5)))}</text>
      </g>`;
    })
    .join("");
  return `<svg class="map" viewBox="${box.left - pad} ${box.top - pad} ${W + pad * 2} ${H + pad * 2}" role="img" aria-label="Monitor map showing where each window lands">
    ${monitors.map((m) => screen(m, "")).join("")}${missing.map((m) => screen(m, "missing")).join("")}${windows}
    ${monitors.map((m) => label(m, "")).join("")}${missing.map((m) => label(m, "missing")).join("")}
  </svg>`;
}

function settingsPage() {
  const s = store.settings;
  const row = (title: string, sub: string, control: string) =>
    `<li class="set"><span class="grow"><span class="name">${title}</span><span class="sub wrap">${sub}</span></span>${control}</li>`;
  const theme = (["system", "light", "dark"] as const)
    .map((t) => `<button role="radio" aria-checked="${(s.theme || "system") === t}" data-act="theme" data-v="${t}">${t[0].toUpperCase() + t.slice(1)}</button>`)
    .join("");
  return `
    <header class="head"><h1>Settings</h1></header>
    <ul class="card settings">
      ${row("Start with Windows", "WindowsKeeper starts in the tray when you sign in.", toggle(s.autostart, "set-autostart", "", "Start with Windows"))}
      ${row("Auto-restore on display change", "When monitors connect or disconnect, restore the layout bound to that monitor setup.", toggle(s.autoRestore, "set-auto", "", "Auto-restore on display change"))}
      ${row("Pause", "Shortcuts and auto-restore do nothing while paused. Restore now still works.", toggle(s.paused, "set-paused", "", "Pause"))}
      ${row("Launch wait", "How long a launched app gets to open its window before WindowsKeeper moves on.", stepper(s.launchWaitSeconds))}
      ${row("Theme", "Follow Windows, or pick one.", `<span class="seg" role="radiogroup" aria-label="Theme">${theme}</span>`)}
    </ul>
    <p class="sub about">WindowsKeeper 0.1.0 · MIT licence · only moves and resizes windows, never closes them.</p>`;
}

// ---------- controls ----------

const toggle = (on: boolean, act: string, i: string, label: string, disabled = false) =>
  `<button class="switch" role="switch" aria-checked="${on}" data-act="${act}" data-i="${i}" aria-label="${esc(label)}" ${disabled ? "disabled" : ""}></button>`;

const stepper = (value: number) => `<span class="stepper" role="group" aria-label="Launch wait in seconds">
    <button class="icon-btn" data-act="wait" data-v="-1" aria-label="Less" ${value <= 1 ? "disabled" : ""}>${ic.minus}</button>
    <input class="num" data-act="wait-input" value="${value}" inputmode="numeric" aria-label="Seconds" maxlength="2" /><span class="sub">s</span>
    <button class="icon-btn" data-act="wait" data-v="1" aria-label="More" ${value >= 60 ? "disabled" : ""}>${ic.plus}</button>
  </span>`;

const keys = (shortcut: string, taken: boolean) =>
  `<span class="keys ${taken ? "taken" : ""}">${shortcut.split("+").map((k) => `<kbd>${esc(k)}</kbd>`).join("")}</span>`;

function menu(id: string, label: string, cls: string, aria: string, items: [string, string, string][]) {
  return `<button class="${cls}" popovertarget="m-${id}" data-anchor="m-${id}" aria-label="${aria}">${label}</button>
    <div class="menu" popover id="m-${id}" data-anchor="m-${id}" role="menu">
      ${items.map(([act, icon, text]) => `<button role="menuitem" data-act="${act}">${icon}<span>${esc(text)}</span></button>`).join("")}
    </div>`;
}

function appIcon(exe: string, cls: string) {
  void loadIcon(exe);
  return icons.get(exe)
    ? `<img class="${cls}" data-icon="${esc(exe)}" alt="" />`
    : `<span class="${cls} mono" aria-hidden="true">${esc(appName(exe).slice(0, 1).toUpperCase())}</span>`;
}

function logo() {
  return `<svg class="logo" viewBox="0 0 64 64" aria-hidden="true"><rect width="64" height="64" rx="15" fill="var(--accent)"/>
    <rect x="12" y="15" width="22" height="34" rx="4" fill="var(--accent-ink)"/><rect x="38" y="15" width="14" height="15" rx="4" fill="var(--accent-ink)" opacity=".75"/><rect x="38" y="34" width="14" height="15" rx="4" fill="var(--accent-ink)" opacity=".5"/></svg>`;
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
  return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

// ---------- data ----------

async function load(select?: string) {
  [store, monitors] = await Promise.all([api.store(), api.monitors()]);
  conflicts = new Set((await api.conflicts()).map(([id]) => id));
  if (select) selected = select;
  if (!store.layouts.some((l) => l.id === selected)) selected = store.layouts[0]?.id ?? null;
  applyTheme();
  await refreshPreview();
}

async function refreshPreview() {
  const current = layout();
  landings = current ? await api.preview(current) : [];
  render();
}

async function loadIcon(exe: string) {
  if (icons.has(exe)) return;
  icons.set(exe, null);
  const icon = await api.icon(exe).catch(() => null);
  if (!icon) return;
  const canvas = Object.assign(document.createElement("canvas"), { width: icon.size, height: icon.size });
  canvas.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(icon.rgba), icon.size, icon.size), 0, 0);
  icons.set(exe, canvas.toDataURL());
  scheduleRender();
}

let pending = 0;
const scheduleRender = () => {
  cancelAnimationFrame(pending);
  pending = requestAnimationFrame(render);
};

async function saveLayout(l: Layout, preview = false) {
  try {
    await api.save(l);
  } catch (error) {
    toast(String(error));
  }
  if (preview) await refreshPreview();
  else render();
}

async function saveSettings() {
  applyTheme();
  try {
    await api.saveSettings(store.settings);
  } catch (error) {
    toast(String(error));
    await load();
    return;
  }
  render();
}

function applyTheme() {
  const theme = store.settings.theme;
  if (theme === "light" || theme === "dark") document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
}

async function capture() {
  try {
    const name = `Layout ${store.layouts.length + 1}`;
    const created = await api.capture(name);
    view = "layout";
    await load(created.id);
    const title = app.querySelector<HTMLInputElement>(".title");
    title?.focus();
    title?.select();
    toast(`Saved ${created.windows.length} windows. Give the layout a name.`);
  } catch (error) {
    toast(String(error));
  }
}

// ---------- actions ----------

app.addEventListener("click", async (event) => {
  const el = (event.target as HTMLElement).closest<HTMLElement>("[data-act]");
  if (!el || (el as HTMLButtonElement).disabled) return;
  const l = layout();
  const i = Number(el.dataset.i);
  switch (el.dataset.act) {
    case "capture": return capture();
    case "select":
      view = "layout";
      recording = false;
      selected = el.dataset.id!;
      return refreshPreview();
    case "settings":
      view = "settings";
      recording = false;
      return render();
    case "unpause":
      store.settings.paused = false;
      return saveSettings();
    case "restore":
      if (l) await api.restore(l.id).catch((e) => toast(String(e)));
      return;
    case "update":
      if (!l) return;
      try {
        const updated = await api.update(l.id);
        await load(updated.id);
        toast(`Updated: ${updated.windows.length} windows`);
      } catch (error) {
        toast(String(error));
      }
      return;
    case "duplicate":
      if (l) await load((await api.duplicate(l.id)).id);
      return;
    case "delete":
      if (l && (await confirmDialog(`Delete “${l.name}”?`, "Its shortcut stops working. Your windows stay where they are.", "Delete"))) {
        await api.remove(l.id);
        selected = null;
        await load();
      }
      return;
    case "record":
      recording = !recording;
      render();
      if (recording) app.querySelector<HTMLElement>(".shortcut")?.focus();
      return;
    case "bind":
      if (l) {
        const updated = await api.bind(l.id, !l.bound);
        Object.assign(l, updated);
        render();
      }
      return;
    case "include":
      if (!l) return;
      l.windows[i].included = !l.windows[i].included;
      return saveLayout(l, true);
    case "launch":
      if (!l) return;
      l.windows[i].launch = l.windows[i].launch ? null : { args: [], workingDir: null, waitSeconds: store.settings.launchWaitSeconds };
      return saveLayout(l);
    case "remove-window":
      if (!l) return;
      l.windows.splice(i, 1);
      return saveLayout(l, true);
    case "set-autostart":
      store.settings.autostart = !store.settings.autostart;
      return saveSettings();
    case "set-auto":
      store.settings.autoRestore = !store.settings.autoRestore;
      return saveSettings();
    case "set-paused":
      store.settings.paused = !store.settings.paused;
      return saveSettings();
    case "wait":
      store.settings.launchWaitSeconds = clampWait(store.settings.launchWaitSeconds + Number(el.dataset.v));
      return saveSettings();
    case "theme":
      store.settings.theme = el.dataset.v!;
      return saveSettings();
  }
});

const clampWait = (n: number) => Math.min(60, Math.max(1, Math.round(n) || 1));

app.addEventListener("change", (event) => {
  const el = event.target as HTMLInputElement;
  const l = layout();
  if (el.dataset.act === "rename" && l) {
    const name = el.value.trim();
    if (name && name !== l.name) {
      l.name = name;
      void saveLayout(l);
    } else el.value = l.name;
  }
  if (el.dataset.act === "wait-input") {
    store.settings.launchWaitSeconds = clampWait(Number(el.value));
    void saveSettings();
  }
});

app.addEventListener("keydown", (event) => {
  const el = event.target as HTMLElement;
  if (el.matches(".title") && (event.key === "Enter" || event.key === "Escape")) {
    if (event.key === "Escape") (el as HTMLInputElement).value = layout()?.name ?? "";
    (el as HTMLInputElement).blur();
    return;
  }
  if (el.dataset.act === "wait-input" && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
    event.preventDefault();
    store.settings.launchWaitSeconds = clampWait(store.settings.launchWaitSeconds + (event.key === "ArrowUp" ? 1 : -1));
    void saveSettings().then(() => app.querySelector<HTMLInputElement>(".num")?.focus());
    return;
  }
  if (el.matches('.seg [role="radio"]') && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
    const radios = [...app.querySelectorAll<HTMLElement>('.seg [role="radio"]')];
    const next = radios[(radios.indexOf(el) + (event.key === "ArrowRight" ? 1 : radios.length - 1)) % radios.length];
    store.settings.theme = next.dataset.v!;
    void saveSettings().then(() => app.querySelector<HTMLElement>(`.seg [data-v="${next.dataset.v}"]`)?.focus());
    return;
  }
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
  const mods = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Win"].filter(Boolean) as string[];
  if (!key || (!mods.length && !key.startsWith("F")) || (mods.length === 1 && mods[0] === "Shift")) return;
  recording = false;
  l.shortcut = [...mods, key].join("+");
  const clash = store.layouts.find((o) => o.id !== l.id && o.shortcut === l.shortcut);
  if (clash) toast(`${clash.name} already uses ${l.shortcut}. It now belongs to ${l.name}.`);
  if (clash) {
    clash.shortcut = null;
    void api.save(clash);
  }
  conflicts.delete(l.id);
  void saveLayout(l).then(focusShortcut);
}
const focusShortcut = () => app.querySelector<HTMLElement>(".shortcut")?.focus();

app.addEventListener("focusout", (event) => {
  if (recording && (event.target as HTMLElement).matches(".shortcut")) {
    recording = false;
    render();
  }
});

// Hover links a window on the map with its row.
app.addEventListener("pointerover", (event) => {
  const el = (event.target as Element).closest<HTMLElement | SVGElement>("[data-w]");
  for (const hot of app.querySelectorAll(".hot")) hot.classList.remove("hot");
  if (el) for (const twin of app.querySelectorAll(`[data-w="${el.dataset.w}"]`)) twin.classList.add("hot");
  tip(event.target as Element);
});
app.addEventListener("focusin", (event) => tip(event.target as Element));
app.addEventListener("pointerleave", () => tip(null));

// Custom tooltip: one element, filled from the nearest [data-tip].
function tip(target: Element | null) {
  const box = document.querySelector<HTMLElement>(".tip");
  const host = target?.closest<HTMLElement>("[data-tip]");
  if (!box) return;
  if (!host) {
    box.hidden = true;
    return;
  }
  box.textContent = host.dataset.tip!;
  box.hidden = false;
  const r = host.getBoundingClientRect();
  const width = box.offsetWidth;
  const left = Math.min(window.innerWidth - width - 8, Math.max(8, r.left + r.width / 2 - width / 2));
  const below = r.bottom + 8 + box.offsetHeight < window.innerHeight;
  box.style.left = `${left}px`;
  box.style.top = `${below ? r.bottom + 8 : r.top - box.offsetHeight - 8}px`;
}

function confirmDialog(title: string, text: string, ok: string) {
  const dialog = document.querySelector<HTMLDialogElement>(".confirm")!;
  dialog.innerHTML = `<h2 id="confirm-title">${esc(title)}</h2><p class="sub wrap">${esc(text)}</p>
    <div class="actions"><button class="ghost" value="cancel">Cancel</button><button class="danger" value="ok">${esc(ok)}</button></div>`;
  return new Promise<boolean>((resolve) => {
    dialog.onclick = (e) => {
      const button = (e.target as HTMLElement).closest("button");
      if (button) dialog.close(button.value);
      else if (e.target === dialog) dialog.close("cancel");
    };
    dialog.onclose = () => resolve(dialog.returnValue === "ok");
    dialog.returnValue = "";
    dialog.showModal();
    dialog.querySelector<HTMLElement>(".ghost")!.focus();
  });
}

// Overlay scrollbar: the native one is hidden (scrollbar-width: none); this thumb mirrors it and can be dragged.
function scrollbar() {
  const scroll = app.querySelector<HTMLElement>(".scroll")!;
  const thumb = app.querySelector<HTMLElement>(".thumb")!;
  const update = () => {
    const { scrollHeight: total, clientHeight: seen, scrollTop: top } = scroll;
    thumb.hidden = total <= seen + 1;
    const size = Math.max(32, (seen * seen) / total);
    thumb.style.height = `${size}px`;
    thumb.style.translate = `0 ${(top / (total - seen || 1)) * (seen - size)}px`;
  };
  scroll.addEventListener("scroll", update, { passive: true });
  new ResizeObserver(update).observe(scroll.firstElementChild!);
  thumb.onpointerdown = (e) => {
    const startY = e.clientY;
    const startTop = scroll.scrollTop;
    thumb.setPointerCapture(e.pointerId);
    thumb.onpointermove = (m) => {
      const ratio = (scroll.scrollHeight - scroll.clientHeight) / (scroll.clientHeight - thumb.offsetHeight || 1);
      scroll.scrollTop = startTop + (m.clientY - startY) * ratio;
    };
    thumb.onpointerup = () => {
      thumb.onpointermove = null;
    };
  };
  update();
}

function toast(text: string) {
  document.querySelector(".toast")?.remove();
  const el = Object.assign(document.createElement("div"), { className: "toast", role: "status", textContent: text });
  document.body.append(el);
  setTimeout(() => el.remove(), 4000);
}

window.addEventListener("resize", () => app.querySelector(".scroll")?.dispatchEvent(new Event("scroll")));
void api.on("restore-result", (text) => toast(String(text)));
void api.on("shortcuts-registered", async () => {
  conflicts = new Set((await api.conflicts()).map(([id]) => id));
  render();
});
void api.on("store-changed", () => void load());
void api.on("save-requested", () => void capture());
void load();
