pub mod autostart;
pub mod events;
pub mod i18n;
pub mod icons;
pub mod layouts;
#[cfg(all(test, windows))]
mod testwin;
pub mod windows;

use events::{EventService, ServiceEvent};
use i18n::tr;
use layouts::{Layout, LayoutStore, Settings};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, SubmenuBuilder},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

struct AppState {
    store: Arc<Mutex<LayoutStore>>,
    path: PathBuf,
    events: Mutex<Option<EventService>>,
    tray: Mutex<Option<TrayIcon>>,
    shortcut_conflicts: Mutex<Vec<(String, String)>>,
}

fn persist(state: &AppState) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    layouts::save(&state.path, &store)
}
fn refresh_events(state: &AppState) {
    if let Ok(mut conflicts) = state.shortcut_conflicts.lock() {
        conflicts.clear();
    }
    if let (Ok(events), Ok(store)) = (state.events.lock(), state.store.lock()) {
        if let Some(events) = events.as_ref() {
            events.refresh(&store.layouts);
        }
    }
}

fn build_tray_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, String> {
    let state = app.state::<AppState>();
    let store = state.store.lock().map_err(|e| e.to_string())?.clone();
    let mut restore = SubmenuBuilder::new(app, tr("Restore layout", "Przywróć układ"));
    if store.layouts.is_empty() {
        restore = restore
            .text("no-layouts", tr("No saved layouts", "Brak zapisanych układów"))
            .enabled(false);
    }
    for layout in &store.layouts {
        restore = restore.text(format!("layout:{}", layout.id), &layout.name);
    }
    let restore = restore.build().map_err(|e| e.to_string())?;
    let save = MenuItem::with_id(app, "save", tr("Save current layout…", "Zapisz bieżący układ…"), true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let open = MenuItem::with_id(app, "open", tr("Open PaneKeeper", "Otwórz PaneKeeper"), true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let pause = CheckMenuItem::with_id(
        app,
        "pause",
        tr("Pause shortcuts", "Wstrzymaj skróty"),
        true,
        store.settings.paused,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let start = CheckMenuItem::with_id(
        app,
        "autostart",
        tr("Start with Windows", "Uruchamiaj z Windows"),
        true,
        store.settings.autostart,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let quit =
        MenuItem::with_id(app, "quit", tr("Quit", "Zamknij"), true, None::<&str>).map_err(|e| e.to_string())?;
    Menu::with_items(app, &[&restore, &save, &open, &pause, &start, &quit])
        .map_err(|e| e.to_string())
}

fn refresh_tray(app: &AppHandle) {
    if let Ok(menu) = build_tray_menu(app) {
        if let Ok(tray) = app.state::<AppState>().tray.lock() {
            if let Some(tray) = tray.as_ref() {
                let _ = tray.set_menu(Some(menu));
            }
        }
    }
}

#[tauri::command]
fn list_layouts(state: State<'_, AppState>) -> Result<LayoutStore, String> {
    Ok(state.store.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn shortcut_conflicts(state: State<'_, AppState>) -> Result<Vec<(String, String)>, String> {
    Ok(state
        .shortcut_conflicts
        .lock()
        .map_err(|e| e.to_string())?
        .clone())
}

#[tauri::command]
fn capture_layout(
    name: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Layout, String> {
    let count = state.store.lock().map_err(|e| e.to_string())?.layouts.len();
    let shortcut = (count < 9).then(|| format!("Ctrl+Alt+{}", count + 1));
    let layout = layouts::capture_layout(name, shortcut)?;
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .layouts
        .push(layout.clone());
    persist(&state)?;
    refresh_events(&state);
    refresh_tray(&app);
    Ok(layout)
}

#[tauri::command]
fn save_layout(layout: Layout, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    if let Some(existing) = store.layouts.iter_mut().find(|item| item.id == layout.id) {
        *existing = layout;
    } else {
        store.layouts.push(layout);
    }
    drop(store);
    persist(&state)?;
    refresh_events(&state);
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn duplicate_layout(
    id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Layout, String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    let mut copy = store
        .layouts
        .iter()
        .find(|layout| layout.id == id)
        .cloned()
        .ok_or_else(not_found)?;
    copy.id = layouts::new_id();
    copy.name = tr!("{} copy", "{} — kopia", copy.name);
    copy.shortcut = None;
    store.layouts.push(copy.clone());
    drop(store);
    persist(&state)?;
    refresh_tray(&app);
    Ok(copy)
}

#[tauri::command]
fn delete_layout(id: String, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .layouts
        .retain(|layout| layout.id != id);
    persist(&state)?;
    refresh_events(&state);
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn save_settings(
    mut settings: Settings,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    settings.lang = settings.lang.filter(|lang| lang == "pl" || lang == "en");
    let (changed, theme_changed) = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let changed = store.settings.autostart != settings.autostart;
        let theme_changed = store.settings.theme != settings.theme;
        store.settings = settings.clone();
        (changed, theme_changed)
    };
    i18n::set(settings.lang.as_deref().unwrap_or_else(|| i18n::system_lang()));
    if theme_changed {
        if let Some(window) = app.get_webview_window("main") {
            apply_theme(&window, &settings.theme);
        }
    }
    if changed {
        autostart::set(
            settings.autostart,
            &std::env::current_exe().map_err(|e| e.to_string())?,
        )?;
    }
    persist(&state)?;
    refresh_tray(&app);
    Ok(())
}

/// The UI language in use: the saved choice, else the Windows UI language.
#[tauri::command]
fn ui_lang() -> &'static str {
    i18n::lang()
}

/// Opens the data folder in Explorer, or an https link in the default browser.
#[tauri::command]
fn open_external(target: String, state: State<'_, AppState>) -> Result<(), String> {
    let target = match target.as_str() {
        "data" => state.path.parent().unwrap_or(&state.path).display().to_string(),
        url if url.starts_with("https://") && !url.contains('"') => url.to_string(),
        _ => return Err(tr("That link can't be opened.", "Tego linku nie można otworzyć.").into()),
    };
    std::process::Command::new("explorer.exe")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Native frame + Mica follow the app theme. Plain Mica follows Windows' theme, not the window's (FolderPilot FO-168),
/// so a chosen theme picks MicaLight/MicaDark explicitly.
fn apply_theme(window: &tauri::WebviewWindow, theme: &str) {
    use tauri::window::{Effect, EffectsBuilder};
    let (native, mica) = match theme {
        "light" => (Some(tauri::Theme::Light), Effect::MicaLight),
        "dark" => (Some(tauri::Theme::Dark), Effect::MicaDark),
        _ => (None, Effect::Mica),
    };
    let _ = window.set_theme(native);
    let _ = window.set_effects(EffectsBuilder::new().effect(mica).build());
}

#[tauri::command]
fn monitors() -> Vec<windows::Monitor> {
    windows::monitors()
}

#[tauri::command]
fn preview_layout(layout: Layout) -> Vec<Option<windows::Landing>> {
    let saved: Vec<_> = layout.windows.iter().map(|entry| entry.window.clone()).collect();
    windows::preview(&saved, &windows::monitors())
}

#[tauri::command]
fn app_icon(exe: String) -> Option<icons::Icon> {
    icons::app_icon(&exe)
}

#[tauri::command]
fn update_layout(id: String, state: State<'_, AppState>, app: AppHandle) -> Result<Layout, String> {
    let current = find_layout(&state, &id)?;
    let updated = layouts::update_from_current(&current)?;
    replace_layout(&state, updated.clone())?;
    refresh_tray(&app);
    Ok(updated)
}

#[tauri::command]
fn bind_layout(
    id: String,
    bound: bool,
    state: State<'_, AppState>,
) -> Result<Layout, String> {
    let mut layout = find_layout(&state, &id)?;
    layout.bound = bound;
    if bound {
        layout.monitor_fingerprint = layouts::monitor_fingerprint(&windows::monitors());
    }
    replace_layout(&state, layout.clone())?;
    Ok(layout)
}

fn not_found() -> String {
    tr(
        "That layout no longer exists. Pick another one in the list.",
        "Ten układ już nie istnieje. Wybierz inny z listy.",
    )
    .into()
}

fn find_layout(state: &AppState, id: &str) -> Result<Layout, String> {
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .layouts
        .iter()
        .find(|layout| layout.id == id)
        .cloned()
        .ok_or_else(not_found)
}

fn replace_layout(state: &AppState, layout: Layout) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let slot = store
            .layouts
            .iter_mut()
            .find(|item| item.id == layout.id)
            .ok_or_else(not_found)?;
        *slot = layout;
    }
    persist(state)
}

#[tauri::command]
fn restore_layout(id: String, app: AppHandle) -> Result<(), String> {
    restore_by_id(&app, &id)
}

fn restore_by_id(app: &AppHandle, id: &str) -> Result<(), String> {
    let layout = app
        .state::<AppState>()
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .layouts
        .iter()
        .find(|layout| layout.id == id)
        .cloned()
        .ok_or_else(not_found)?;
    let wait = app
        .state::<AppState>()
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .settings
        .launch_wait_seconds;
    let mut layout = layout;
    for launch in layout.windows.iter_mut().filter_map(|entry| entry.launch.as_mut()) {
        launch.wait_seconds = wait;
    }
    let app = app.clone();
    thread::spawn(move || {
        let results = layouts::restore_layout(&layout);
        let _ = app.emit("restore-result", layouts::summary(&results));
    });
    Ok(())
}

fn handle_service_event(app: AppHandle, event: ServiceEvent) {
    match event {
        ServiceEvent::RestoreLayout { layout_id } => {
            let paused = app
                .state::<AppState>()
                .store
                .lock()
                .map(|store| store.settings.paused)
                .unwrap_or(true);
            if !paused {
                let _ = restore_by_id(&app, &layout_id);
            }
        }
        ServiceEvent::ShortcutConflict {
            layout_id,
            shortcut,
        } => {
            if let Ok(mut conflicts) = app.state::<AppState>().shortcut_conflicts.lock() {
                conflicts.push((layout_id.clone(), shortcut.clone()));
            }
            let _ = app.emit("shortcut-conflict", (layout_id, shortcut));
        }
        ServiceEvent::ShortcutsRegistered => {
            let _ = app.emit("shortcuts-registered", ());
        }
        ServiceEvent::DisplayChanged => {
            let state = app.state::<AppState>();
            let store = match state.store.lock() {
                Ok(store) => store.clone(),
                Err(_) => return,
            };
            if store.settings.paused || !store.settings.auto_restore {
                return;
            }
            let fingerprint = layouts::monitor_fingerprint(&windows::monitors());
            if let Some(layout) = store
                .layouts
                .iter()
                .find(|layout| layout.bound && layout.monitor_fingerprint == fingerprint)
            {
                let _ = restore_by_id(&app, &layout.id);
            }
        }
    }
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    windows::enable_dpi_awareness();
    tauri::Builder::default()
        .setup(|app| {
            let path = layouts::app_root()?.join("layouts.json");
            let store = layouts::load(&path);
            i18n::set(store.settings.lang.as_deref().unwrap_or_else(|| i18n::system_lang()));
            if let Some(window) = app.get_webview_window("main") {
                apply_theme(&window, &store.settings.theme);
            }
            let store = Arc::new(Mutex::new(store));
            app.manage(AppState {
                store: store.clone(),
                path,
                events: Mutex::new(None),
                tray: Mutex::new(None),
                shortcut_conflicts: Mutex::new(Vec::new()),
            });
            let handle = app.handle().clone();
            let initial = store.lock().map_err(|e| e.to_string())?.layouts.clone();
            let service = EventService::start(&initial, move |event| {
                handle_service_event(handle.clone(), event)
            })?;
            *app.state::<AppState>()
                .events
                .lock()
                .map_err(|e| e.to_string())? = Some(service);

            let menu = build_tray_menu(app.handle())?;
            let tray = TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .ok_or("missing app icon")?, // i18n-ok: build defect, before any window
                )
                .tooltip("PaneKeeper")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    let id = event.id.as_ref();
                    if let Some(layout_id) = id.strip_prefix("layout:") {
                        let _ = restore_by_id(app, layout_id);
                        return;
                    }
                    match id {
                        "open" => show_main(app),
                        "save" => {
                            show_main(app);
                            let _ = app.emit("save-requested", ());
                        }
                        "pause" => {
                            let state = app.state::<AppState>();
                            if let Ok(mut store) = state.store.lock() {
                                store.settings.paused = !store.settings.paused;
                            }
                            let _ = persist(&state);
                            refresh_tray(app);
                            let _ = app.emit("store-changed", ());
                        }
                        "autostart" => {
                            let state = app.state::<AppState>();
                            if let Ok(mut store) = state.store.lock() {
                                let enabled = !store.settings.autostart;
                                if autostart::set(
                                    enabled,
                                    &std::env::current_exe().unwrap_or_default(),
                                )
                                .is_ok()
                                {
                                    store.settings.autostart = enabled;
                                }
                            }
                            let _ = persist(&state);
                            refresh_tray(app);
                            let _ = app.emit("store-changed", ());
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .build(app)?;
            *app.state::<AppState>()
                .tray
                .lock()
                .map_err(|e| e.to_string())? = Some(tray);
            if !std::env::args().any(|arg| arg == "--autostart") {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_layouts,
            shortcut_conflicts,
            capture_layout,
            save_layout,
            duplicate_layout,
            delete_layout,
            save_settings,
            restore_layout,
            monitors,
            preview_layout,
            app_icon,
            ui_lang,
            open_external,
            update_layout,
            bind_layout
        ])
        .run(tauri::generate_context!())
        .expect("error while running PaneKeeper"); // i18n-ok: crash text
}
