// CCS/1 UI text, English + Polish. Every user-facing string in src/ lives here (tray + Rust errors: src-tauri/src/i18n.rs).
// RULE:pl typed as `typeof en` -> tsc rejects a missing/extra key; scripts/i18n-check.mjs checks shapes + stray text
// LANG:start=localStorage (no flash) then backend ui_lang() (saved choice ?? Windows UI language, non-Polish -> en)

export type Lang = "en" | "pl";

/** Polish plural: 1 → one, 2-4 (not 12-14) → few, else many. */
const plural = (n: number, one: string, few: string, many: string) =>
  n === 1 ? one : n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14) ? few : many;
const windows = (n: number) => `${n} ${n === 1 ? "window" : "windows"}`;
const okna = (n: number) => `${n} ${plural(n, "okno", "okna", "okien")}`;

const en = {
  // ---- Navigation
  nav: "Navigation",
  layouts: "Layouts", settings: "Settings", about: "About",
  saveLayout: "Save current layout",
  // ---- Empty state
  emptyText: "Arrange your windows on your monitors, then save them as a layout. Ctrl+Alt+1 brings the first one back from any app.",
  // ---- Layout page
  layoutName: "Layout name",
  newLayoutName: (n: number) => `Layout ${n}`,
  included: (n: number, total: number) => `${n} of ${windows(total)} included`,
  update: "Update from current",
  updateTip: "Replace the saved windows with how they sit right now. Include and launch choices are kept.",
  restore: "Restore now",
  more: "More actions", duplicate: "Duplicate", delete: "Delete",
  shortcut: "Shortcut",
  shortcutText: "Restores this layout from any app.",
  shortcutTaken: "Another app already uses this shortcut. Pick a different one.",
  shortcutNone: "None. Click to set",
  shortcutPress: "Press keys…",
  shortcutHelp: "Esc cancels · Backspace clears",
  shortcutLabel: (keys: string) => `Shortcut: ${keys}. Press to change.`,
  none: "none",
  shortcutMoved: (other: string, keys: string) => `${other} used ${keys}.`,
  shortcutMovedText: (name: string) => `The shortcut now belongs to ${name}.`,
  bind: "Bind to this monitor setup",
  bindText: "Restore this layout by itself when these monitors connect.",
  bindOn: "Restores by itself when these monitors connect.",
  bindNeedsAuto: "Turn on auto-restore in Settings to use it.",
  mapLabel: "Monitor map showing where each window lands",
  noMonitors: "No monitors found.",
  notConnected: "Not connected",
  notConnectedUses: (name: string) => `Not connected · uses ${name}`,
  windowsTitle: "Windows",
  noWindows: "No windows saved. Use Update from current.",
  include: "Include", includeApp: (app: string) => `Include ${app}`,
  launch: "Launch", launchApp: (app: string) => `Launch ${app} if it isn't running`,
  launchTip: "Start the app if it isn't running, then place its window.",
  removeWindow: (app: string) => `Remove ${app} from this layout`,
  states: { normal: "Normal", maximized: "Maximised", minimized: "Minimised" },
  movedTip: (from: string, to: string) => `${from} isn't connected. This window goes to ${to}.`,
  // ---- Messages
  saved: (n: number) => `Saved ${windows(n)}.`,
  nameIt: "Give the layout a name.",
  updated: (n: number) => `Updated with ${windows(n)}.`,
  deleted: (name: string) => `Deleted ${name}.`,
  undo: "Undo",
  restoreDone: "Layout restored.",
  restorePartly: "Layout partly restored.",
  restoredN: (n: number) => `${n} restored`,
  notRunningN: (n: number) => `${n} not running`,
  blockedN: (n: number) => `${n} blocked`,
  pausedTitle: "Shortcuts and auto-restore are paused.",
  pausedText: "Restore now still works.",
  resume: "Resume",
  dismiss: "Close",
  actionFailed: "That did not work.",
  saveFailed: "The change was not saved.",
  loadFailed: "PaneKeeper could not read its data.",
  openFailed: "That could not be opened.",
  // ---- Settings
  general: "General",
  startWithWindows: "Start with Windows", startText: "PaneKeeper starts in the tray when you sign in.",
  language: "Language", languageText: "Language of the window, tray menu and messages.",
  theme: "Theme", themeText: "As in Windows, or one you choose.",
  themes: { system: "Use system setting", light: "Light", dark: "Dark" },
  autoRestore: "Auto-restore on display change",
  autoRestoreText: "When monitors connect or disconnect, restore the layout bound to that monitor setup.",
  pause: "Pause shortcuts",
  pauseText: "Shortcuts and auto-restore do nothing while paused. Restore now still works.",
  launchWait: "Launch wait",
  launchWaitText: "How long a launched app gets to open its window before PaneKeeper moves on.",
  launchWaitLabel: "Launch wait in seconds",
  secMore: "One second more", secLess: "One second less",
  seconds: "s",
  advanced: "Advanced",
  dataFolder: "Data folder", dataFolderText: "Your layouts and settings, in layouts.json.",
  openFolder: "Open folder",
  on: "On", off: "Off",
  // ---- About
  version: (v: string) => `Version ${v}`,
  aboutText: "Saves where your app windows sit on every monitor and puts them back with one shortcut.",
  safety: "PaneKeeper only moves and resizes windows. It never closes them.",
  projectPage: "Project page", source: "Source code", issues: "Report a problem", licence: "MIT licence",
};

const pl: typeof en = {
  // ---- Navigation
  nav: "Nawigacja",
  layouts: "Układy", settings: "Ustawienia", about: "O aplikacji",
  saveLayout: "Zapisz bieżący układ",
  // ---- Empty state
  emptyText: "Ułóż okna na monitorach i zapisz je jako układ. Ctrl+Alt+1 przywróci pierwszy z nich z dowolnej aplikacji.",
  // ---- Layout page
  layoutName: "Nazwa układu",
  newLayoutName: (n) => `Układ ${n}`,
  included: (n, total) => `Uwzględnione okna: ${n} z ${total}`,
  update: "Zaktualizuj z bieżącego",
  updateTip: "Zastąp zapisane okna ich obecnym położeniem. Wybory uwzględniania i uruchamiania zostają.",
  restore: "Przywróć teraz",
  more: "Więcej akcji", duplicate: "Duplikuj", delete: "Usuń",
  shortcut: "Skrót",
  shortcutText: "Przywraca ten układ z dowolnej aplikacji.",
  shortcutTaken: "Ten skrót zajmuje już inna aplikacja. Wybierz inny.",
  shortcutNone: "Brak. Kliknij, aby ustawić",
  shortcutPress: "Naciśnij klawisze…",
  shortcutHelp: "Esc anuluje · Backspace czyści",
  shortcutLabel: (keys) => `Skrót: ${keys}. Naciśnij, aby zmienić.`,
  none: "brak",
  shortcutMoved: (other, keys) => `${other} miał skrót ${keys}.`,
  shortcutMovedText: (name) => `Ten skrót należy teraz do: ${name}.`,
  bind: "Powiąż z tym zestawem monitorów",
  bindText: "Przywracaj ten układ samoczynnie, gdy te monitory się podłączą.",
  bindOn: "Przywraca się samoczynnie, gdy te monitory się podłączą.",
  bindNeedsAuto: "Aby z tego korzystać, włącz automatyczne przywracanie w Ustawieniach.",
  mapLabel: "Mapa monitorów pokazująca, gdzie trafi każde okno",
  noMonitors: "Nie znaleziono monitorów.",
  notConnected: "Niepodłączony",
  notConnectedUses: (name) => `Niepodłączony · używa: ${name}`,
  windowsTitle: "Okna",
  noWindows: "Brak zapisanych okien. Użyj przycisku Zaktualizuj z bieżącego.",
  include: "Uwzględnij", includeApp: (app) => `Uwzględnij ${app}`,
  launch: "Uruchom", launchApp: (app) => `Uruchom ${app}, jeśli nie działa`,
  launchTip: "Uruchom aplikację, jeśli nie działa, a potem ustaw jej okno.",
  removeWindow: (app) => `Usuń ${app} z tego układu`,
  states: { normal: "Zwykłe", maximized: "Zmaksymalizowane", minimized: "Zminimalizowane" },
  movedTip: (from, to) => `${from} nie jest podłączony. To okno trafi na: ${to}.`,
  // ---- Messages
  saved: (n) => `Zapisano ${okna(n)}.`,
  nameIt: "Nadaj układowi nazwę.",
  updated: (n) => `Zaktualizowano: ${okna(n)}.`,
  deleted: (name) => `Usunięto ${name}.`,
  undo: "Cofnij",
  restoreDone: "Przywrócono układ.",
  restorePartly: "Układ przywrócono częściowo.",
  restoredN: (n) => `przywrócone: ${n}`,
  notRunningN: (n) => `niedziałające: ${n}`,
  blockedN: (n) => `zablokowane: ${n}`,
  pausedTitle: "Skróty i automatyczne przywracanie są wstrzymane.",
  pausedText: "Przywróć teraz nadal działa.",
  resume: "Wznów",
  dismiss: "Zamknij",
  actionFailed: "To się nie udało.",
  saveFailed: "Zmiana nie została zapisana.",
  loadFailed: "PaneKeeper nie wczytał swoich danych.",
  openFailed: "Nie udało się tego otworzyć.",
  // ---- Settings
  general: "Ogólne",
  startWithWindows: "Uruchamiaj z Windows", startText: "PaneKeeper uruchamia się w zasobniku po zalogowaniu.",
  language: "Język", languageText: "Język okna, menu w zasobniku i komunikatów.",
  theme: "Motyw", themeText: "Jak w Windows albo wybrany.",
  themes: { system: "Jak w systemie", light: "Jasny", dark: "Ciemny" },
  autoRestore: "Automatyczne przywracanie po zmianie monitorów",
  autoRestoreText: "Gdy monitory się podłączą lub odłączą, przywróć układ powiązany z tym zestawem monitorów.",
  pause: "Wstrzymaj skróty",
  pauseText: "Po wstrzymaniu skróty i automatyczne przywracanie nic nie robią. Przywróć teraz nadal działa.",
  launchWait: "Czas na uruchomienie",
  launchWaitText: "Ile czasu uruchomiona aplikacja ma na otwarcie okna, zanim PaneKeeper przejdzie dalej.",
  launchWaitLabel: "Czas na uruchomienie w sekundach",
  secMore: "O sekundę dłużej", secLess: "O sekundę krócej",
  seconds: "s",
  advanced: "Zaawansowane",
  dataFolder: "Folder danych", dataFolderText: "Twoje układy i ustawienia, w pliku layouts.json.",
  openFolder: "Otwórz folder",
  on: "Wł.", off: "Wył.",
  // ---- About
  version: (v) => `Wersja ${v}`,
  aboutText: "Zapamiętuje, gdzie stoją okna aplikacji na każdym monitorze, i przywraca je jednym skrótem.",
  safety: "PaneKeeper tylko przesuwa okna i zmienia ich rozmiar. Nigdy ich nie zamyka.",
  projectPage: "Strona projektu", source: "Kod źródłowy", issues: "Zgłoś problem", licence: "Licencja MIT",
};

export const dictionaries = { en, pl };
export let lang: Lang = "en";
export let t: typeof en = en;

/** Switches every later `t.*` read; callers re-render. */
export function setLang(next: Lang) {
  lang = next;
  t = next === "pl" ? pl : en;
  if (typeof document !== "undefined") document.documentElement.lang = next;
}

/** Windows UI language for a first start: Polish stays Polish, anything else is English. */
export function systemLang(): Lang {
  return typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("pl") ? "pl" : "en";
}
