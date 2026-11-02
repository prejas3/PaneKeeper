use crate::windows::{self, RestoreResult, WindowSnapshot};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfig {
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub wait_seconds: u64,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            args: Vec::new(),
            working_dir: None,
            wait_seconds: 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutWindow {
    pub window: WindowSnapshot,
    pub included: bool,
    pub launch: Option<LaunchConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub id: String,
    pub name: String,
    pub shortcut: Option<String>,
    pub monitor_fingerprint: String,
    pub windows: Vec<LayoutWindow>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub auto_restore: bool,
    pub paused: bool,
    pub autostart: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutStore {
    pub layouts: Vec<Layout>,
    pub settings: Settings,
}

pub fn new_id() -> String {
    format!(
        "layout-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    )
}

pub fn monitor_fingerprint(monitors: &[windows::Monitor]) -> String {
    let mut ids: Vec<String> = monitors.iter().map(|monitor| monitor.id.clone()).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.join("|")
}

pub fn app_root() -> Result<PathBuf, String> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "APPDATA is unavailable".into())
        .map(|root| root.join("WindowsKeeper"))
}

pub fn load(path: &Path) -> LayoutStore {
    let Ok(text) = fs::read_to_string(path) else {
        return LayoutStore::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|error| {
        eprintln!("windowskeeper: {}: {error}", path.display());
        let _ = fs::rename(path, path.with_extension("json.bad"));
        LayoutStore::default()
    })
}

pub fn save(path: &Path, store: &LayoutStore) -> Result<(), String> {
    let write = || -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(store)?)?;
        replace_file(&tmp, path)
    };
    write().map_err(|error| error.to_string())
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        fs::remove_file(to)?;
    }
    fs::rename(from, to)
}

pub fn capture_layout(name: String, shortcut: Option<String>) -> Result<Layout, String> {
    let windows = windows::capture()?;
    Ok(Layout {
        id: new_id(),
        name,
        shortcut,
        monitor_fingerprint: monitor_fingerprint(&windows::monitors()),
        windows: windows
            .into_iter()
            .map(|window| LayoutWindow {
                window,
                included: true,
                launch: None,
            })
            .collect(),
    })
}

pub fn restore_layout(layout: &Layout) -> Vec<RestoreResult> {
    let selected: Vec<WindowSnapshot> = layout
        .windows
        .iter()
        .filter(|entry| entry.included)
        .map(|entry| entry.window.clone())
        .collect();
    let initial = windows::restore(&selected);
    for (entry, result) in layout
        .windows
        .iter()
        .filter(|entry| entry.included)
        .zip(initial.iter())
    {
        if matches!(result, RestoreResult::NotRunning { .. }) {
            if let Some(launch) = &entry.launch {
                let _ = launch_and_wait(&entry.window, launch);
            }
        }
    }
    windows::restore(&selected)
}

fn launch_and_wait(snapshot: &WindowSnapshot, launch: &LaunchConfig) -> Result<(), String> {
    let mut command = Command::new(&snapshot.exe);
    command.args(&launch.args);
    if let Some(dir) = &launch.working_dir {
        command.current_dir(dir);
    }
    command.spawn().map_err(|error| error.to_string())?;
    wait_for(Duration::from_secs(launch.wait_seconds), || {
        windows::capture()
            .map(|current| {
                current.iter().any(|window| {
                    window.exe.eq_ignore_ascii_case(&snapshot.exe)
                        && window.class.eq_ignore_ascii_case(&snapshot.class)
                })
            })
            .unwrap_or(false)
    })
}

fn wait_for(timeout: Duration, mut ready: impl FnMut() -> bool) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if ready() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for the launched app window".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::{Insets, Monitor, Rect, ShowState};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "windowskeeper-layout-test-{}-{}",
                new_id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn window(monitor_id: &str) -> WindowSnapshot {
        let area = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        WindowSnapshot {
            exe: "notepad.exe".into(),
            class: "Notepad".into(),
            title: "Test".into(),
            normal_rect: Rect {
                left: 10,
                top: 10,
                right: 600,
                bottom: 500,
            },
            frame_rect: Rect::default(),
            frame_insets: Insets::default(),
            show_state: ShowState::Normal,
            monitor: Monitor {
                id: monitor_id.into(),
                name: monitor_id.into(),
                bounds: area,
                work_area: area,
                dpi: 96,
                primary: true,
            },
            z_order: 0,
        }
    }

    #[test]
    fn store_round_trip() {
        let temp = Temp::new();
        let path = temp.0.join("layouts.json");
        let store = LayoutStore {
            layouts: vec![Layout {
                id: "work".into(),
                name: "Work".into(),
                shortcut: Some("Ctrl+Alt+1".into()),
                monitor_fingerprint: "A".into(),
                windows: vec![LayoutWindow {
                    window: window("A"),
                    included: true,
                    launch: Some(LaunchConfig::default()),
                }],
            }],
            settings: Settings {
                auto_restore: true,
                ..Default::default()
            },
        };
        save(&path, &store).unwrap();
        assert_eq!(load(&path), store);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn unreadable_store_is_kept_aside() {
        let temp = Temp::new();
        let path = temp.0.join("layouts.json");
        fs::write(&path, "{ broken").unwrap();
        assert_eq!(load(&path), LayoutStore::default());
        assert_eq!(
            fs::read_to_string(path.with_extension("json.bad")).unwrap(),
            "{ broken"
        );
    }

    #[test]
    fn monitor_fingerprint_is_stable_and_unique() {
        let a = window("A").monitor;
        let b = window("B").monitor;
        assert_eq!(monitor_fingerprint(&[b.clone(), a, b]), "A|B");
    }

    #[test]
    fn launch_wait_timeout_polls_the_condition() {
        let probes = AtomicUsize::new(0);
        let result = wait_for(Duration::ZERO, || {
            probes.fetch_add(1, Ordering::Relaxed);
            false
        });
        assert!(result.unwrap_err().contains("timed out"));
        assert_eq!(probes.load(Ordering::Relaxed), 1);
    }
}
