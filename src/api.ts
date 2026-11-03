import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// CCS/1 M:api | PURPOSE:typed bridge to src-tauri/src/lib.rs commands (serde camelCase)

export type Rect = { left: number; top: number; right: number; bottom: number };
export type Monitor = { id: string; name: string; bounds: Rect; workArea: Rect; dpi: number; primary: boolean };
export type ShowState = "normal" | "maximized" | "minimized";
export type WindowSnapshot = {
  exe: string;
  class: string;
  title: string;
  normalRect: Rect;
  frameRect: Rect;
  frameInsets: Rect;
  showState: ShowState;
  monitor: Monitor;
  zOrder: number;
};
export type LaunchConfig = { args: string[]; workingDir: string | null; waitSeconds: number };
export type LayoutWindow = { window: WindowSnapshot; included: boolean; launch: LaunchConfig | null };
export type Layout = {
  id: string;
  name: string;
  shortcut: string | null;
  monitorFingerprint: string;
  bound: boolean;
  windows: LayoutWindow[];
};
export type Theme = "system" | "light" | "dark";
export type Settings = { autoRestore: boolean; paused: boolean; autostart: boolean; launchWaitSeconds: number; theme: Theme; lang: "pl" | "en" | null };
export type Store = { layouts: Layout[]; settings: Settings };
export type Landing = { monitorId: string; rect: Rect };
export type Icon = { size: number; rgba: number[] };
/** Payload of "restore-result" (layouts.rs RestoreSummary); the UI words it. */
export type RestoreSummary = { restored: number; notRunning: number; blocked: { title: string; error: string }[] };

export const api = {
  store: () => invoke<Store>("list_layouts"),
  conflicts: () => invoke<[string, string][]>("shortcut_conflicts"),
  capture: (name: string) => invoke<Layout>("capture_layout", { name }),
  save: (layout: Layout) => invoke<void>("save_layout", { layout }),
  duplicate: (id: string) => invoke<Layout>("duplicate_layout", { id }),
  remove: (id: string) => invoke<void>("delete_layout", { id }),
  update: (id: string) => invoke<Layout>("update_layout", { id }),
  bind: (id: string, bound: boolean) => invoke<Layout>("bind_layout", { id, bound }),
  restore: (id: string) => invoke<void>("restore_layout", { id }),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  monitors: () => invoke<Monitor[]>("monitors"),
  preview: (layout: Layout) => invoke<(Landing | null)[]>("preview_layout", { layout }),
  icon: (exe: string) => invoke<Icon | null>("app_icon", { exe }),
  uiLang: () => invoke<"pl" | "en">("ui_lang"),
  /** "data" opens the data folder; an https URL opens in the browser. */
  openExternal: (target: string) => invoke<void>("open_external", { target }),
  on: (event: string, fn: (payload: unknown) => void) => listen(event, (e) => fn(e.payload)),
};
