pub mod autostart;
pub mod events;
pub mod layouts;
pub mod windows;

use events::{EventService, ServiceEvent};
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
    let mut restore = SubmenuBuilder::new(app, "Restore layout");
    if store.layouts.is_empty() {
        restore = restore
            .text("no-layouts", "No saved layouts")
            .enabled(false);
    }
    for layout in &store.layouts {
        restore = restore.text(format!("layout:{}", layout.id), &layout.name);
    }
    let restore = restore.build().map_err(|e| e.to_string())?;
    let save = MenuItem::with_id(app, "save", "Save current layout…", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let open = MenuItem::with_id(app, "open", "Open WindowsKeeper", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let pause = CheckMenuItem::with_id(
        app,
        "pause",
        "Pause",
        true,
        store.settings.paused,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let start = CheckMenuItem::with_id(
        app,
        "autostart",
        "Start with Windows",
        true,
        store.settings.autostart,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let quit =
        MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).map_err(|e| e.to_string())?;
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
    Ok(state.shortcut_conflicts.lock().map_err(|e| e.to_string())?.clone())
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
        .ok_or("layout not found")?;
    copy.id = layouts::new_id();
    copy.name = format!("{} copy", copy.name);
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
    settings: Settings,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let changed = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let changed = store.settings.autostart != settings.autostart;
        store.settings = settings.clone();
        changed
    };
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
        .ok_or("layout not found")?;
    let app = app.clone();
    thread::spawn(move || {
        let results = layouts::restore_layout(&layout);
        let restored = results
            .iter()
            .filter(|result| matches!(result, windows::RestoreResult::Restored { .. }))
            .count();
        let not_running = results
            .iter()
            .filter(|result| matches!(result, windows::RestoreResult::NotRunning { .. }))
            .count();
        let blocked = results.len().saturating_sub(restored + not_running);
        let _ = app.emit(
            "restore-result",
            format!("{restored} restored · {not_running} not running · {blocked} blocked"),
        );
    });
    Ok(())
}

fn handle_service_event(app: AppHandle, event: ServiceEvent) {
    match event {
        ServiceEvent::RestoreLayout { layout_id } => {
            let _ = restore_by_id(&app, &layout_id);
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
                .find(|layout| layout.monitor_fingerprint == fingerprint)
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
            let store = Arc::new(Mutex::new(layouts::load(&path)));
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
                        .ok_or("missing app icon")?,
                )
                .tooltip("WindowsKeeper")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    let id = event.id.as_ref();
                    if let Some(layout_id) = id.strip_prefix("layout:") {
                        let _ = restore_by_id(app, layout_id);
                        return;
                    }
                    match id {
                        "save" | "open" => show_main(app),
                        "pause" => {
                            let state = app.state::<AppState>();
                            if let Ok(mut store) = state.store.lock() {
                                store.settings.paused = !store.settings.paused;
                            }
                            let _ = persist(&state);
                            refresh_tray(app);
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
            restore_layout
        ])
        .run(tauri::generate_context!())
        .expect("error while running WindowsKeeper");
}
