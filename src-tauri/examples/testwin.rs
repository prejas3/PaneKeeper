// CCS/1 M:test-helper | PURPOSE:test-owned windows for the live tests and scripts/e2e-auto.mjs
// SEC:every command finds windows by class "WindowsKeeperTestWindow" only, so it can never touch anyone else's window
//
//   testwin window <title> [l t r b] [max|min]   open a test window (window rect, screen coords)
//   testwin query <title>                        print {"state","normal":[l,t,r,b],"pid"} or {"found":false}
//   testwin move <title> l t r b                 restore it and set its window rect
//   testwin show <title> normal|max|min
//   testwin focus <title>                        bring it to the foreground
//   testwin keys ctrl+alt+shift+F9               press a shortcut with SendInput
//   testwin foreground                           print {"class","pid"} of the foreground window
//   testwin monitors                             print [{"bounds","work","primary"}]

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    win::main();
}

#[cfg(windows)]
mod win {
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO,
            MONITOR_DEFAULTTOPRIMARY,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
                VK_LWIN, VK_MENU, VK_SHIFT,
            },
            WindowsAndMessaging::*,
        },
    };

    const CLASS: &str = "WindowsKeeperTestWindow";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn find(title: &str) -> HWND {
        unsafe { FindWindowW(wide(CLASS).as_ptr(), wide(title).as_ptr()) }
    }

    fn need(title: &str) -> HWND {
        let hwnd = find(title);
        if hwnd.is_null() {
            eprintln!("no test window titled {title:?}");
            std::process::exit(2);
        }
        hwnd
    }

    fn nums(args: &[String]) -> [i32; 4] {
        let v: Vec<i32> = args.iter().take(4).map(|a| a.parse().expect("number")).collect();
        [v[0], v[1], v[2], v[3]]
    }

    /// GetWindowPlacement is in workspace coordinates: shift by the primary monitor's work-area origin.
    fn workspace_offset() -> (i32, i32) {
        unsafe {
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            GetMonitorInfoW(MonitorFromPoint(zeroed(), MONITOR_DEFAULTTOPRIMARY), &mut info);
            (info.rcWork.left - info.rcMonitor.left, info.rcWork.top - info.rcMonitor.top)
        }
    }

    pub fn main() {
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let cmd = args.first().map(String::as_str).unwrap_or("window");
        let title = args.get(1).cloned().unwrap_or_else(|| "WindowsKeeper test window".into());
        unsafe {
            match cmd {
                "window" => window(&title, &args[2.min(args.len())..]),
                "query" => {
                    let hwnd = find(&title);
                    if hwnd.is_null() {
                        println!(r#"{{"found":false}}"#);
                        return;
                    }
                    let mut p: WINDOWPLACEMENT = zeroed();
                    p.length = size_of::<WINDOWPLACEMENT>() as u32;
                    GetWindowPlacement(hwnd, &mut p);
                    let (dx, dy) = workspace_offset();
                    let n = p.rcNormalPosition;
                    let state = match p.showCmd as i32 {
                        SW_SHOWMAXIMIZED => "maximized",
                        SW_SHOWMINIMIZED => "minimized",
                        _ => "normal",
                    };
                    let mut pid = 0;
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    println!(
                        r#"{{"found":true,"state":"{state}","normal":[{},{},{},{}],"pid":{pid}}}"#,
                        n.left + dx, n.top + dy, n.right + dx, n.bottom + dy
                    );
                }
                "move" => {
                    let hwnd = need(&title);
                    let [l, t, r, b] = nums(&args[2..]);
                    ShowWindow(hwnd, SW_RESTORE);
                    SetWindowPos(hwnd, null_mut(), l, t, r - l, b - t, SWP_NOZORDER | SWP_NOACTIVATE);
                }
                "show" => {
                    let hwnd = need(&title);
                    let how = match args.get(2).map(String::as_str) {
                        Some("max") => SW_MAXIMIZE,
                        Some("min") => SW_MINIMIZE,
                        _ => SW_RESTORE,
                    };
                    ShowWindow(hwnd, how);
                }
                "focus" => {
                    let hwnd = need(&title);
                    // A tap of Alt lets a background process take the foreground (the lock allows it after input).
                    press(&[VK_MENU]);
                    SetForegroundWindow(hwnd);
                }
                "keys" => press(&parse_keys(&title)),
                "foreground" => {
                    let hwnd = GetForegroundWindow();
                    let mut class = [0u16; 256];
                    let len = GetClassNameW(hwnd, class.as_mut_ptr(), 256);
                    let mut pid = 0;
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    println!(r#"{{"class":"{}","pid":{pid}}}"#, String::from_utf16_lossy(&class[..len.max(0) as usize]));
                }
                "monitors" => {
                    let mut out: Vec<String> = Vec::new();
                    EnumDisplayMonitors(null_mut(), null(), Some(monitor), &mut out as *mut _ as LPARAM);
                    println!("[{}]", out.join(","));
                }
                other => {
                    eprintln!("unknown command {other}");
                    std::process::exit(2);
                }
            }
        }
    }

    unsafe extern "system" fn monitor(handle: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(handle, &mut info);
        let (m, w) = (info.rcMonitor, info.rcWork);
        (*(data as *mut Vec<String>)).push(format!(
            r#"{{"bounds":[{},{},{},{}],"work":[{},{},{},{}],"primary":{}}}"#,
            m.left, m.top, m.right, m.bottom, w.left, w.top, w.right, w.bottom, info.dwFlags & 1 == 1
        ));
        1
    }

    fn parse_keys(spec: &str) -> Vec<u16> {
        spec.split('+')
            .map(|k| match k.to_ascii_lowercase().as_str() {
                "ctrl" => VK_CONTROL,
                "alt" => VK_MENU,
                "shift" => VK_SHIFT,
                "win" => VK_LWIN,
                f if f.starts_with('f') && f.len() > 1 => 0x70 + f[1..].parse::<u16>().expect("F key") - 1,
                c => c.to_ascii_uppercase().as_bytes()[0] as u16,
            })
            .collect()
    }

    /// Key downs in order, then key ups in reverse.
    unsafe fn press(keys: &[u16]) {
        let input = |vk: u16, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 },
            },
        };
        let mut all: Vec<INPUT> = keys.iter().map(|&k| input(k, false)).collect();
        all.extend(keys.iter().rev().map(|&k| input(k, true)));
        SendInput(all.len() as u32, all.as_ptr(), size_of::<INPUT>() as i32);
    }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_DESTROY {
            PostQuitMessage(0);
            return 0;
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    unsafe fn window(title: &str, rest: &[String]) {
        let instance = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: windows_sys::Win32::Graphics::Gdi::GetStockObject(windows_sys::Win32::Graphics::Gdi::WHITE_BRUSH) as _,
            ..zeroed()
        };
        RegisterClassW(&wc);
        let [l, t, r, b] = if rest.len() >= 4 { nums(rest) } else { [200, 200, 900, 700] };
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide(title).as_ptr(),
            WS_OVERLAPPEDWINDOW,
            l,
            t,
            r - l,
            b - t,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        // ponytail: a second SetWindowPos after the window exists, so a rect on a monitor with another DPI is exact.
        SetWindowPos(hwnd, null_mut(), l, t, r - l, b - t, SWP_NOZORDER | SWP_NOACTIVATE);
        let show = match rest.get(4).map(String::as_str) {
            Some("max") => SW_SHOWMAXIMIZED,
            Some("min") => SW_SHOWMINNOACTIVE,
            _ => SW_SHOWNOACTIVATE,
        };
        ShowWindow(hwnd, show);
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
