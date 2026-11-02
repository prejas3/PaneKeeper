use crate::layouts::Layout;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ServiceEvent {
    RestoreLayout { layout_id: String },
    ShortcutConflict { layout_id: String, shortcut: String },
    /// Registration finished; the conflict list is complete.
    ShortcutsRegistered,
    DisplayChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}

pub fn parse_hotkey(value: &str) -> Result<Hotkey, String> {
    const ALT: u32 = 0x0001;
    const CTRL: u32 = 0x0002;
    const SHIFT: u32 = 0x0004;
    const WIN: u32 = 0x0008;
    const NO_REPEAT: u32 = 0x4000;
    let parts: Vec<_> = value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let Some(key_name) = parts.last() else {
        return Err("shortcut is empty".into());
    };
    let mut modifiers = NO_REPEAT;
    for modifier in &parts[..parts.len() - 1] {
        modifiers |= match modifier.to_ascii_lowercase().as_str() {
            "alt" => ALT,
            "ctrl" | "control" => CTRL,
            "shift" => SHIFT,
            "win" | "meta" => WIN,
            _ => return Err(format!("unknown modifier: {modifier}")),
        };
    }
    let upper = key_name.to_ascii_uppercase();
    let key = if upper.len() == 1 && upper.as_bytes()[0].is_ascii_alphanumeric() {
        upper.as_bytes()[0] as u32
    } else if let Some(number) = upper.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=24).contains(&number) {
            0x70 + number - 1
        } else {
            return Err("function key must be F1..F24".into());
        }
    } else {
        return Err(format!("unsupported key: {key_name}"));
    };
    Ok(Hotkey { modifiers, key })
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        ptr::{null, null_mut},
        sync::{mpsc, Arc, Mutex, OnceLock},
        thread,
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, KillTimer,
                PostMessageW, RegisterClassW, SetTimer, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
                MSG, WM_APP, WM_DESTROY, WM_DISPLAYCHANGE, WM_HOTKEY, WM_TIMER, WNDCLASSW,
            },
        },
    };

    const REFRESH: u32 = WM_APP + 41;
    const DISPLAY_TIMER: usize = 1;

    struct Context {
        layouts: Mutex<Vec<(String, String)>>,
        registered: Mutex<usize>,
        callback: Arc<dyn Fn(ServiceEvent) + Send + Sync>,
    }
    static CONTEXT: OnceLock<Arc<Context>> = OnceLock::new();

    pub struct EventService {
        hwnd: Arc<Mutex<isize>>,
    }

    impl EventService {
        pub fn start(
            layouts: &[Layout],
            callback: impl Fn(ServiceEvent) + Send + Sync + 'static,
        ) -> Result<Self, String> {
            let shortcuts = shortcut_bindings(layouts);
            let context = Arc::new(Context {
                layouts: Mutex::new(shortcuts),
                registered: Mutex::new(0),
                callback: Arc::new(callback),
            });
            CONTEXT
                .set(context)
                .map_err(|_| "event service is already running".to_string())?;
            let hwnd = Arc::new(Mutex::new(0));
            let thread_hwnd = hwnd.clone();
            let (ready_tx, ready_rx) = mpsc::channel();
            thread::Builder::new()
                .name("windowskeeper-events".into())
                .spawn(move || unsafe {
                    let class: Vec<u16> = "WindowsKeeperEvents\0".encode_utf16().collect();
                    let instance = GetModuleHandleW(null());
                    let wc = WNDCLASSW {
                        style: CS_HREDRAW | CS_VREDRAW,
                        lpfnWndProc: Some(window_proc),
                        hInstance: instance,
                        lpszClassName: class.as_ptr(),
                        ..std::mem::zeroed()
                    };
                    RegisterClassW(&wc);
                    let window = CreateWindowExW(
                        0,
                        class.as_ptr(),
                        class.as_ptr(),
                        0,
                        0,
                        0,
                        0,
                        0,
                        null_mut(),
                        null_mut(),
                        instance,
                        null(),
                    );
                    if window.is_null() {
                        let _ = ready_tx.send(Err(std::io::Error::last_os_error().to_string()));
                        return;
                    }
                    *thread_hwnd.lock().unwrap() = window as isize;
                    register_all(window);
                    let _ = ready_tx.send(Ok(()));
                    let mut message: MSG = std::mem::zeroed();
                    while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                })
                .map_err(|error| error.to_string())?;
            ready_rx.recv().map_err(|error| error.to_string())??;
            Ok(Self { hwnd })
        }

        pub fn refresh(&self, layouts: &[Layout]) {
            if let Some(context) = CONTEXT.get() {
                *context.layouts.lock().unwrap() = shortcut_bindings(layouts);
            }
            let hwnd = *self.hwnd.lock().unwrap() as HWND;
            if !hwnd.is_null() {
                unsafe {
                    PostMessageW(hwnd, REFRESH, 0, 0);
                }
            }
        }
    }

    fn shortcut_bindings(layouts: &[Layout]) -> Vec<(String, String)> {
        layouts
            .iter()
            .filter_map(|layout| {
                layout
                    .shortcut
                    .as_ref()
                    .map(|shortcut| (layout.id.clone(), shortcut.clone()))
            })
            .collect()
    }

    unsafe fn register_all(hwnd: HWND) {
        let Some(context) = CONTEXT.get() else { return };
        let layouts = context.layouts.lock().unwrap().clone();
        for (index, (layout_id, shortcut)) in layouts.iter().enumerate() {
            let id = index as i32 + 1;
            if let Ok(hotkey) = parse_hotkey(shortcut) {
                if RegisterHotKey(hwnd, id, hotkey.modifiers, hotkey.key) == 0 {
                    (context.callback)(ServiceEvent::ShortcutConflict {
                        layout_id: layout_id.clone(),
                        shortcut: shortcut.clone(),
                    });
                }
            }
        }
        *context.registered.lock().unwrap() = layouts.len();
        (context.callback)(ServiceEvent::ShortcutsRegistered);
    }

    unsafe fn unregister_all(hwnd: HWND) {
        if let Some(context) = CONTEXT.get() {
            let mut registered = context.registered.lock().unwrap();
            for index in 0..*registered {
                UnregisterHotKey(hwnd, index as i32 + 1);
            }
            *registered = 0;
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_HOTKEY => {
                if let Some(context) = CONTEXT.get() {
                    let layout_id = context
                        .layouts
                        .lock()
                        .unwrap()
                        .get(wparam as usize - 1)
                        .map(|(layout_id, _)| layout_id.clone());
                    if let Some(layout_id) = layout_id {
                        (context.callback)(ServiceEvent::RestoreLayout { layout_id });
                    }
                }
                0
            }
            WM_DISPLAYCHANGE => {
                KillTimer(hwnd, DISPLAY_TIMER);
                SetTimer(hwnd, DISPLAY_TIMER, 750, None);
                0
            }
            WM_TIMER if wparam == DISPLAY_TIMER => {
                KillTimer(hwnd, DISPLAY_TIMER);
                if let Some(context) = CONTEXT.get() {
                    (context.callback)(ServiceEvent::DisplayChanged);
                }
                0
            }
            REFRESH => {
                unregister_all(hwnd);
                register_all(hwnd);
                0
            }
            WM_DESTROY => {
                unregister_all(hwnd);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

#[cfg(windows)]
pub use platform::EventService;

#[cfg(not(windows))]
pub struct EventService;
#[cfg(not(windows))]
impl EventService {
    pub fn start(
        _: &[Layout],
        _: impl Fn(ServiceEvent) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        Ok(Self)
    }
    pub fn refresh(&self, _: &[Layout]) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_default_and_custom_shortcuts() {
        assert_eq!(
            parse_hotkey("Ctrl+Alt+1").unwrap(),
            Hotkey {
                modifiers: 0x4003,
                key: b'1' as u32
            }
        );
        assert_eq!(
            parse_hotkey("Win+Shift+F12").unwrap(),
            Hotkey {
                modifiers: 0x400c,
                key: 0x7b
            }
        );
        assert!(parse_hotkey("Ctrl+Nope").is_err());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "opens, moves, and restores two test-owned Notepad windows while another app has focus"]
    fn live_hotkey_restores_notepads_while_another_app_has_focus() {
        use crate::{
            layouts::{self, LayoutWindow},
            windows::{self, Rect},
        };
        use std::{
            fs,
            process::Command,
            sync::mpsc,
            thread,
            time::{Duration, Instant, SystemTime},
        };

        let token = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let root = std::env::temp_dir().join(format!("windowskeeper-hotkey-{token}"));
        fs::create_dir_all(&root).unwrap();
        let mut children: Vec<_> = ["a", "b"]
            .iter()
            .map(|part| {
                let path = root.join(format!("wk-{part}-{token}.txt"));
                fs::write(&path, "WindowsKeeper hotkey test").unwrap();
                Command::new("notepad.exe")
                    .arg("/newWindow")
                    .arg(path)
                    .spawn()
                    .unwrap()
            })
            .collect();
        let pids: Vec<u32> = children.iter().map(|child| child.id()).collect();
        let wait = |ready: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(15);
            while Instant::now() < deadline {
                if ready() {
                    return true;
                }
                thread::sleep(Duration::from_millis(50));
            }
            false
        };
        assert!(wait(&|| unsafe {
            windows::test_capture_processes(&pids).len() == 2
        }));
        let saved = unsafe { windows::test_capture_processes(&pids) };
        let layout = Layout {
            id: "live".into(),
            name: "Live".into(),
            shortcut: Some("Ctrl+Alt+9".into()),
            monitor_fingerprint: layouts::monitor_fingerprint(&windows::monitors()),
            bound: false,
            windows: saved
                .iter()
                .cloned()
                .map(|window| LayoutWindow {
                    window,
                    included: true,
                    launch: None,
                })
                .collect(),
        };
        for (index, pid) in pids.iter().enumerate() {
            assert!(unsafe {
                windows::test_move_window_for_process(
                    *pid,
                    Rect {
                        left: 60 + index as i32 * 340,
                        top: 70,
                        right: 360 + index as i32 * 340,
                        bottom: 330,
                    },
                )
            });
        }
        let foreground = unsafe { windows::test_foreground_process_id() };
        assert_ne!(foreground, 0);
        assert_ne!(foreground, std::process::id());
        let (tx, rx) = mpsc::channel();
        let restore = layout.clone();
        let _service = EventService::start(&[layout], move |event| match event {
            ServiceEvent::RestoreLayout { .. } => {
                let _ = layouts::restore_layout(&restore);
                let _ = tx.send(Ok(()));
            }
            ServiceEvent::ShortcutConflict { .. } => {
                let _ = tx.send(Err("shortcut conflict"));
            }
            ServiceEvent::ShortcutsRegistered | ServiceEvent::DisplayChanged => {}
        })
        .unwrap();
        assert!(rx.try_recv().is_err());
        send_ctrl_alt_9().unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Ok(()));
        assert!(wait(&|| {
            let current = unsafe { windows::test_capture_processes(&pids) };
            saved.iter().all(|expected| {
                current.iter().any(|actual| {
                    let a = actual.frame_rect;
                    let e = expected.frame_rect;
                    (a.left - e.left).abs() <= 2
                        && (a.top - e.top).abs() <= 2
                        && (a.right - e.right).abs() <= 2
                        && (a.bottom - e.bottom).abs() <= 2
                })
            })
        }));
        for child in &mut children {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    fn send_ctrl_alt_9() -> Result<(), String> {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

        fn key(vk: u16, up: bool) -> INPUT {
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }
        }

        let inputs = [
            key(VK_CONTROL, false),
            key(VK_MENU, false),
            key(b'9' as u16, false),
            key(b'9' as u16, true),
            key(VK_MENU, true),
            key(VK_CONTROL, true),
        ];
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if sent == inputs.len() as u32 {
            Ok(())
        } else {
            Err(format!("SendInput sent {sent}/{} events", inputs.len()))
        }
    }
}
