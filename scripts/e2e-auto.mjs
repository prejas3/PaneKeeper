// CCS/1 #189 isolated real-app E2E: `npm run e2e` (builds the release exe + testwin, then runs this). Exit 0 = all passed.
// Drives the real release exe through WebView2's DevTools port; APPDATA and WebView2 data live in one temp tree.
// SEC:only test-owned windows are ever moved: testwin.exe (src-tauri/examples/testwin.rs) acts on its own window class only,
//     and every other window the app captures is removed from the layout before anything is restored.
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const EXE = process.argv[2] ?? here("../src-tauri/target/release/panekeeper.exe");
const TW = process.argv[3] ?? here("../src-tauri/target/release/examples/testwin.exe");
const STAMP = Date.now();
const ROOT = path.join(os.tmpdir(), `PaneKeeper-Test-${STAMP}`);
const P = (...p) => path.join(ROOT, ...p);
const PREFIX = `wk-e2e-${STAMP}`;
const LAUNCHED = "PaneKeeper test window"; // testwin's title when started without arguments
const SHORTCUT = "Ctrl+Alt+Shift+F9"; // unusual on purpose: no clash with the default Ctrl+Alt+1…9
const localDir = path.join(process.env.LOCALAPPDATA ?? os.tmpdir(), "com.szymonjankiewicz.panekeeper");
const localExisted = fs.existsSync(localDir);
const results = [];
const check = (name, ok, detail = "") => {
  results.push({ name, ok: !!ok, detail });
  console.log(`${ok ? "PASS" : "FAIL"} ${name} ${detail}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(fn, ms = 15000, step = 150) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v || Date.now() > end) return v;
    await sleep(step);
  }
}

// ---------- test windows ----------
const tw = (...args) => {
  const out = execFileSync(TW, args.map(String), { encoding: "utf8" }).trim();
  return out ? JSON.parse(out) : null;
};
const children = [];
const open = (title, rect, state = "normal") => {
  children.push(spawn(TW, ["window", title, ...rect.map(String), state], { stdio: "ignore" }));
};
const query = (title) => tw("query", title);
const near = (a, b) => a.every((v, i) => Math.abs(v - b[i]) <= 2);
const inside = (r, area) => r[0] >= area[0] && r[1] >= area[1] && r[2] <= area[2] && r[3] <= area[3];
const rectOf = (r) => [r.left, r.top, r.right, r.bottom];

const monitors = tw("monitors");
const primary = monitors.find((m) => m.primary);
const others = monitors.filter((m) => !m.primary);
const at = (m, dx, dy, w, h) => [m.work[0] + dx, m.work[1] + dy, m.work[0] + dx + w, m.work[1] + dy + h];
const A = { title: `${PREFIX}-A`, rect: at(primary, 120, 90, 760, 520), state: "normal" };
const B = { title: `${PREFIX}-B`, rect: at(others[0] ?? primary, 80, 60, 700, 480), state: "maximized" };
const C = { title: `${PREFIX}-C`, rect: at(others[1] ?? others[0] ?? primary, 140, 100, 640, 460), state: "normal" };
const tests = [A, B, C];

// ---------- app + DevTools ----------
let proc;
let PORT = 0;
let launches = 0;
/** A port nothing listens on right now (a fixed one can be taken by another run or app). */
const freePort = () =>
  new Promise((resolve, reject) => {
    const server = net.createServer().once("error", reject);
    server.listen(0, "127.0.0.1", () => { const { port } = server.address(); server.close(() => resolve(port)); });
  });
const targets = async () => {
  try {
    return await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  } catch {
    return null;
  }
};
/** Open the DevTools socket of the app page; the first one can close while WebView2 is still starting. */
async function connect() {
  let last = "no DevTools target";
  for (let attempt = 1; attempt <= 8; attempt++) {
    const target = await until(async () => {
      if (proc.exitCode !== null) throw new Error(`app exited with code ${proc.exitCode} before DevTools was ready (log: ${appLog})`);
      return (await targets())?.find((t) => t.type === "page" && /^(https?|tauri):\/\/tauri\.localhost/.test(t.url));
    }, 60000);
    if (!target) break;
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    const opened = await new Promise((resolve) => {
      ws.onopen = () => resolve(true);
      ws.onerror = (e) => { last = `WebSocket error on attempt ${attempt}: ${e.message ?? e.type}`; resolve(false); };
    });
    if (opened) return ws;
    await sleep(500);
  }
  throw new Error(`${last} (port ${PORT}, log: ${appLog})`);
}
let appLog = "";
async function launch() {
  PORT = await freePort();
  appLog = P(`app-${++launches}.log`);
  const log = fs.openSync(appLog, "w");
  proc = spawn(EXE, [], {
    env: { ...process.env, APPDATA: P("AppData"), WEBVIEW2_USER_DATA_FOLDER: P("WebView2"), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
    stdio: ["ignore", log, log],
  });
  const ws = await connect();
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => { const msg = JSON.parse(m.data); waiting.get(msg.id)?.resolve(msg); waiting.delete(msg.id); };
  // A closed socket fails every pending call instead of leaving the run hanging.
  ws.onclose = () => { for (const w of waiting.values()) w.reject(new Error(`DevTools socket closed (app exit code ${proc.exitCode})`)); waiting.clear(); };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      if (ws.readyState !== WebSocket.OPEN) return reject(new Error("DevTools socket is not open"));
      const i = ++id;
      waiting.set(i, { resolve, reject });
      ws.send(JSON.stringify({ id: i, method, params }));
    });
  const js = async (expr) => {
    const res = await send("Runtime.evaluate", { expression: `(async () => { ${expr} })()`, awaitPromise: true, returnByValue: true });
    if (res.result?.exceptionDetails) throw new Error(JSON.stringify(res.result.exceptionDetails));
    return res.result?.result?.value;
  };
  const narrow = (width) => width ? send("Emulation.setDeviceMetricsOverride", { width, height: 640, deviceScaleFactor: 0, mobile: false }) : send("Emulation.clearDeviceMetricsOverride");
  const shot = async (name) => { const r = await send("Page.captureScreenshot", { format: "png" }); fs.writeFileSync(P(name), Buffer.from(r.result.data, "base64")); };
  if (!(await until(() => js(`return !!document.querySelector(".save")`), 30000))) throw new Error(`app UI did not render (log: ${appLog})`);
  // Restore results arrive as an InfoBar (#restore) carrying its counts; keep every one for the checks (WI-196).
  await js(`window.__toasts = []; new MutationObserver((m) => m.forEach((r) => r.addedNodes.forEach((n) => n.id === "restore" && __toasts.push([n.dataset.restored, n.dataset.notRunning, n.dataset.blocked].join("/"))))).observe(document.body, { childList: true, subtree: true });`);
  return { js, shot, ws, narrow };
}
async function stop() {
  if (proc.exitCode !== null) return;
  proc.kill();
  await until(async () => proc.exitCode !== null && !(await targets()), 10000);
}
/** Screenshots in light and dark on the solid background, then the page as it was. */
async function themed(js, shot, name) {
  const was = await js(`const h = document.documentElement; const was = [h.className, h.dataset.theme ?? ""]; h.classList.remove("mica"); return was;`);
  for (const theme of ["light", "dark"]) {
    await js(`document.documentElement.dataset.theme = "${theme}"`);
    await sleep(100);
    await shot(`${name}-${theme}.png`);
  }
  await js(`const h = document.documentElement; h.className = ${JSON.stringify(was[0])}; if (${JSON.stringify(was[1])}) h.dataset.theme = ${JSON.stringify(was[1])}; else delete h.dataset.theme;`);
}
const click = (sel) => `const el = document.querySelector(${JSON.stringify(sel)}); if (!el) throw new Error("missing " + ${JSON.stringify(sel)}); el.click();`;
const store = () => JSON.parse(fs.readFileSync(P("AppData/PaneKeeper/layouts.json"), "utf8"));
const layout = () => store().layouts[0];
const toastLike = (js, text) => until(() => js(`return __toasts.some((t) => t.startsWith(${JSON.stringify(text)}))`), 20000);
const clearToasts = (js) => js(`__toasts.length = 0`);

/** Every test window is back at its saved rect and state. */
const inPlace = (saved, titles = tests.map((t) => t.title)) =>
  until(() =>
    titles.every((title, i) => {
      const s = saved.find((w) => w.window.title === tests[i].title).window;
      const q = query(title);
      return q.found && q.state === s.showState && near(q.normal, rectOf(s.normalRect));
    }),
  );
function scramble() {
  tw("move", A.title, primary.work[0] + 400, primary.work[1] + 300, primary.work[0] + 900, primary.work[1] + 700);
  tw("show", B.title, "normal");
  tw("move", B.title, primary.work[0] + 30, primary.work[1] + 30, primary.work[0] + 430, primary.work[1] + 330);
  tw("show", C.title, "min");
}

try {
  fs.mkdirSync(P("AppData"), { recursive: true });
  for (const t of tests) open(t.title, t.rect, t.state === "maximized" ? "max" : "normal");
  check("3 test windows open", await until(() => tests.every((t) => query(t.title).found)), `${monitors.length} monitor(s)`);

  let { js, shot, narrow } = await launch();

  // 1. save a layout, keep only the test windows, set the shortcut
  await js(click('[data-act="capture"]'));
  await until(() => fs.existsSync(P("AppData/PaneKeeper/layouts.json")) && layout()?.windows.length);
  for (;;) { // remove every captured window that isn't ours: the user's windows are never part of the test
    const removed = await js(`const row = [...document.querySelectorAll(".win")].find((r) => !r.querySelector(".sub").textContent.startsWith(${JSON.stringify(PREFIX)}));
      if (!row) return false; row.querySelector('[data-act="remove-window"]').click(); return true;`);
    if (!removed) break;
    await sleep(120);
  }
  await until(() => layout().windows.length === 3);
  const saved = layout().windows;
  check(
    "1. save: only the 3 test windows, states and monitors recorded",
    saved.length === 3 && tests.every((t) => saved.some((w) => w.window.title === t.title && w.window.showState === t.state)),
    saved.map((w) => `${w.window.title.slice(-1)}=${w.window.showState}@${w.window.monitor.name}`).join(" "),
  );
  check("1. save: each window on the monitor it was opened on", tests.every((t) => {
    const w = saved.find((s) => s.window.title === t.title).window;
    return inside(rectOf(w.normalRect).map((v, i) => v + (i < 2 ? 8 : -8)), rectOf(w.monitor.bounds));
  }));
  await js(`${click('[data-act="record"]')} await new Promise((r) => setTimeout(r, 50));
    document.querySelector(".shortcut").dispatchEvent(new KeyboardEvent("keydown", { key: "F9", code: "F9", ctrlKey: true, altKey: true, shiftKey: true, bubbles: true }));`);
  check("1. shortcut set through the capture field", await until(() => layout().shortcut === SHORTCUT), layout().shortcut);
  check("1. shortcut registered without a conflict", await until(() => js(`return !!document.querySelector(".shortcut") && !document.querySelector(".shortcut.taken")`)));
  // SJ Utility UI §11: both themes on the solid (no Mica) fallback, since a capture shows Mica as blank;
  // and a 420 px window without horizontal scrolling (WI-196)
  await themed(js, shot, "01-layout");
  await narrow(420);
  await sleep(300);
  check("1. 420 px wide: no horizontal scroll", await js(`return document.documentElement.scrollWidth <= innerWidth && document.querySelector(".content").scrollWidth <= document.querySelector(".content").clientWidth`));
  await shot("01-layout-420.png");
  await narrow(0);

  // 2. scramble, then Restore now
  scramble();
  await clearToasts(js);
  await js(click('[data-act="restore"]'));
  check("2. Restore now: rects within 2 px, states right", await inPlace(saved));
  check("2. InfoBar counts", await toastLike(js, "3/0/0"));

  // 3. global shortcut while a test window has focus
  scramble();
  tw("focus", A.title);
  const fg = await until(() => { const f = tw("foreground"); return f.class === "PaneKeeperTestWindow" && f; }, 5000);
  check("3. another window has focus", fg, JSON.stringify(tw("foreground")));
  tw("keys", SHORTCUT);
  check("3. shortcut restores from another app", await inPlace(saved));

  // 4. launch if not running
  const cRow = saved.findIndex((w) => w.window.title === C.title);
  await js(click(`[data-act="launch"][data-i="${cRow}"]`));
  check("4. launch switched on and saved", await until(() => !!layout().windows[cRow].launch));
  process.kill(query(C.title).pid);
  await until(() => !query(C.title).found);
  await clearToasts(js);
  await js(click('[data-act="restore"]'));
  const sC = saved[cRow].window;
  check(
    "4. closed app is started and placed",
    await until(() => { const q = query(LAUNCHED); return q.found && q.state === sC.showState && near(q.normal, rectOf(sC.normalRect)); }, 20000),
    JSON.stringify(query(LAUNCHED)),
  );
  check("4. InfoBar counts", await toastLike(js, "3/0/0"), await js(`return __toasts.join(" | ")`));

  // 5 + 6. fake a missing monitor for A, restart, restore with the shortcut
  await stop();
  const edited = store();
  const fakeA = edited.layouts[0].windows.find((w) => w.window.title === A.title).window;
  const left = Math.min(...monitors.map((m) => m.bounds[0])) - 4000;
  fakeA.monitor = { ...fakeA.monitor, id: "MONITOR\\FAKE0000\\e2e", name: "E2E gone", bounds: { left, top: 0, right: left + 1920, bottom: 1080 }, workArea: { left, top: 0, right: left + 1920, bottom: 1040 }, primary: false };
  fakeA.normalRect = { left: left + 1000, top: 500, right: left + 1900, bottom: 1030 };
  fs.writeFileSync(P("AppData/PaneKeeper/layouts.json"), JSON.stringify(edited, null, 1));
  ({ js, shot } = await launch());
  check("6. restart: layout and shortcut kept", await until(() => js(`return [...document.querySelectorAll(".nav-keys")].some((k) => k.textContent.replaceAll(" ", "") === ${JSON.stringify(SHORTCUT)})`)));
  check("6. first render after restart: map labels at normal size", await until(() => js(`const h = document.querySelector(".label-name")?.getBoundingClientRect().height; return h > 0 && h < 24`)), await js(`return document.querySelector(".label-name")?.getBoundingClientRect().height`));
  check("5. missing monitor drawn dashed on the map", await until(() => js(`return document.querySelectorAll(".screen.missing").length === 1`)));
  await shot("02-missing-monitor.png");
  tw("move", A.title, primary.work[0] + 400, primary.work[1] + 300, primary.work[0] + 900, primary.work[1] + 700);
  tw("show", B.title, "normal");
  tw("focus", B.title);
  tw("keys", SHORTCUT);
  check(
    "5. window from the missing monitor lands fully visible on a connected one",
    await until(() => { const q = query(A.title); return q.found && q.normal[2] - q.normal[0] > 100 && monitors.some((m) => inside(q.normal, m.work)); }),
    JSON.stringify(query(A.title)),
  );
  check("6. shortcut still works after restart", await until(() => query(B.title).state === "maximized"));

  // 7. language: the Settings switch changes the whole window, is saved and survives a restart (WI-196)
  const navText = () => js(`return document.querySelector('[data-act="go"][data-v="settings"]')?.textContent.trim()`);
  const before = await js(`return document.documentElement.lang`);
  const other = before === "pl" ? "en" : "pl";
  const settingsBefore = await navText();
  await js(click('[data-act="go"][data-v="settings"]'));
  await until(() => js(`return !!document.querySelector(".card .select")`), 5000);
  await js(`document.querySelector(".card .select").click()`); // first select on Settings = language
  await until(() => js(`return document.querySelectorAll('[role="option"]').length > 1`), 5000);
  await js(`[...document.querySelectorAll('[role="option"]')].find(o => o.textContent === ${JSON.stringify(other === "pl" ? "Polski" : "English")}).click()`);
  const switched = await until(async () => (await js(`return document.documentElement.lang`)) === other && (await navText()) !== settingsBefore, 5000);
  check("7. language switch changes the window", !!switched, `${before} -> ${other}: ${settingsBefore} -> ${await navText()}`);
  check("7. language saved in layouts.json", await until(() => store().settings.lang === other, 5000));
  await themed(js, shot, "03-settings-other-language");
  await stop();
  ({ js, shot } = await launch());
  await js(click('[data-act="go"][data-v="about"]'));
  await sleep(300);
  await themed(js, shot, "04-about");
  check("7. language survives restart", await until(async () => (await js(`return document.documentElement.lang`)) === other && (await navText()) !== settingsBefore, 10000));
} catch (error) {
  check("run without errors", false, String(error?.stack ?? error));
} finally {
  if (proc) await stop();
  for (const title of [...tests.map((t) => t.title), LAUNCHED]) {
    const q = (() => { try { return query(title); } catch { return { found: false }; } })();
    if (q.found) try { process.kill(q.pid); } catch {}
  }
  for (const child of children) child.kill();
}

// Tauri creates an empty app folder in LOCALAPPDATA on start, even with WebView2 data redirected:
// remove it when this run made it (never a real install's folder, which is not empty or was there before).
if (!localExisted) try { fs.rmdirSync(localDir); } catch {}

const failed = results.filter((r) => !r.ok);
console.log(`\n${results.length - failed.length}/${results.length} passed`);
console.log("Not scripted: an elevated (admin) window blocked by UIPI. Covered by layouts::tests::summary_counts_and_names_blocked_windows.");
if (failed.length || process.env.E2E_KEEP) console.log(`Kept for inspection: ${ROOT}`);
// WebView2's helper processes let go of their files a moment after the app exits.
else if (!process.env.E2E_KEEP) fs.rmSync(ROOT, { recursive: true, force: true, maxRetries: 20, retryDelay: 250 });
process.exit(failed.length ? 1 : 0);
