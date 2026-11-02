use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Insets {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub work_area: Rect,
    pub dpi: u32,
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShowState {
    Normal,
    Maximized,
    Minimized,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSnapshot {
    pub exe: String,
    pub class: String,
    pub title: String,
    pub normal_rect: Rect,
    pub frame_rect: Rect,
    pub frame_insets: Insets,
    pub show_state: ShowState,
    pub monitor: Monitor,
    pub z_order: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveWindow {
    pub handle: isize,
    pub process_id: u32,
    pub exe: String,
    pub class: String,
    pub title: String,
    pub z_order: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum RestoreResult {
    Restored { title: String },
    NotRunning { title: String },
    Failed { title: String, error: String },
}

/// Stable greedy assignment: strongest title pairs first, z-order breaks ties, no HWND is reused.
pub fn match_windows(saved: &[WindowSnapshot], live: &[LiveWindow]) -> Vec<Option<usize>> {
    let mut candidates = Vec::new();
    for (si, s) in saved.iter().enumerate() {
        for (li, l) in live.iter().enumerate() {
            if path_key(&s.exe) == path_key(&l.exe) && s.class.eq_ignore_ascii_case(&l.class) {
                candidates.push((
                    title_score(&s.title, &l.title),
                    s.z_order,
                    l.z_order,
                    si,
                    li,
                ));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut result = vec![None; saved.len()];
    let mut used = HashSet::new();
    for (_, _, _, si, li) in candidates {
        if result[si].is_none() && used.insert(li) {
            result[si] = Some(li);
        }
    }
    result
}

fn path_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

fn title_score(a: &str, b: &str) -> u32 {
    let a = a.trim().to_lowercase();
    let b = b.trim().to_lowercase();
    if a == b {
        return 10_000;
    }
    if !a.is_empty() && (a.contains(&b) || b.contains(&a)) {
        return 8_000 + a.len().min(b.len()) as u32;
    }
    let prefix = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count() as u32;
    let distance = levenshtein(&a, &b) as u32;
    4_000u32
        .saturating_add(prefix * 8)
        .saturating_sub(distance.min(3_999))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (0..=b.chars().count()).collect();
    for (i, ac) in a.chars().enumerate() {
        let mut diagonal = i;
        row[0] = i + 1;
        for (j, bc) in b.chars().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ac == bc {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[b.chars().count()]
}

pub fn map_rect(saved: Rect, from: Rect, to: Rect, from_dpi: u32, to_dpi: u32) -> Rect {
    let fw = from.width().max(1) as f64;
    let fh = from.height().max(1) as f64;
    let tw = to.width().max(1) as f64;
    let th = to.height().max(1) as f64;
    let dpi_scale = to_dpi.max(1) as f64 / from_dpi.max(1) as f64;
    let scale = (tw / fw).min(th / fh).min(dpi_scale.max(0.5)).max(0.25);
    let left = to.left + (((saved.left - from.left) as f64 / fw) * tw).round() as i32;
    let top = to.top + (((saved.top - from.top) as f64 / fh) * th).round() as i32;
    let width = (saved.width().max(1) as f64 * scale).round() as i32;
    let height = (saved.height().max(1) as f64 * scale).round() as i32;
    clamp_rect(
        Rect {
            left,
            top,
            right: left + width,
            bottom: top + height,
        },
        to,
    )
}

pub fn clamp_rect(mut rect: Rect, area: Rect) -> Rect {
    let width = rect.width().max(1).min(area.width().max(1));
    let height = rect.height().max(1).min(area.height().max(1));
    rect.left = rect.left.clamp(area.left, area.right - width);
    rect.top = rect.top.clamp(area.top, area.bottom - height);
    rect.right = rect.left + width;
    rect.bottom = rect.top + height;
    rect
}

pub fn placement_from_frame(frame: Rect, insets: Insets, from_dpi: u32, to_dpi: u32) -> Rect {
    let scale = to_dpi.max(1) as f64 / from_dpi.max(1) as f64;
    let s = |value: i32| (value as f64 * scale).round() as i32;
    Rect {
        left: frame.left - s(insets.left),
        top: frame.top - s(insets.top),
        right: frame.right + s(insets.right),
        bottom: frame.bottom + s(insets.bottom),
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::mem::{size_of, zeroed};
    use windows_sys::core::BOOL;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, RECT},
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS},
            Gdi::{
                EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow,
                DISPLAY_DEVICEW, HDC, HMONITOR, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
            },
        },
        System::Threading::{
            GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::{
            HiDpi::{
                GetDpiForMonitor, SetProcessDpiAwarenessContext,
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
            },
            WindowsAndMessaging::{
                EnumWindows, GetClassNameW, GetDesktopWindow, GetShellWindow, GetWindowLongW,
                GetWindowPlacement, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
                IsWindowVisible, SetWindowPlacement, GWL_EXSTYLE, MONITORINFOF_PRIMARY,
                SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, WINDOWPLACEMENT, WS_EX_TOOLWINDOW,
            },
        },
    };

    fn rect(value: RECT) -> Rect {
        Rect {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
    fn wide(value: &[u16]) -> String {
        String::from_utf16_lossy(
            &value[..value.iter().position(|x| *x == 0).unwrap_or(value.len())],
        )
    }

    struct EnumState {
        own_pid: u32,
        process_ids: Option<HashSet<u32>>,
        windows: Vec<WindowSnapshot>,
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> BOOL {
        let state = &mut *(data as *mut EnumState);
        if IsWindowVisible(hwnd) == 0 || hwnd == GetShellWindow() || hwnd == GetDesktopWindow() {
            return 1;
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
            return 1;
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            &mut cloaked as *mut _ as _,
            size_of::<u32>() as u32,
        ) == 0
            && cloaked != 0
        {
            return 1;
        }
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return 1;
        }
        let mut process = 0;
        GetWindowThreadProcessId(hwnd, &mut process);
        if process == state.own_pid
            || state
                .process_ids
                .as_ref()
                .is_some_and(|ids| !ids.contains(&process))
        {
            return 1;
        }
        let mut title = vec![0u16; length as usize + 1];
        GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let mut class = vec![0u16; 256];
        GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        let Some(exe) = executable(process) else {
            return 1;
        };
        let mut placement: WINDOWPLACEMENT = zeroed();
        placement.length = size_of::<WINDOWPLACEMENT>() as u32;
        if GetWindowPlacement(hwnd, &mut placement) == 0 {
            return 1;
        }
        let mut frame: RECT = zeroed();
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            &mut frame as *mut _ as _,
            size_of::<RECT>() as u32,
        ) != 0
        {
            frame = placement.rcNormalPosition;
        }
        let normal = rect(placement.rcNormalPosition);
        let frame = rect(frame);
        let monitor = monitor_for_window(hwnd);
        let show_state = match placement.showCmd {
            value if value == SW_SHOWMAXIMIZED as u32 => ShowState::Maximized,
            value if value == SW_SHOWMINIMIZED as u32 => ShowState::Minimized,
            _ => ShowState::Normal,
        };
        state.windows.push(WindowSnapshot {
            exe,
            class: wide(&class),
            title: wide(&title),
            normal_rect: normal,
            frame_rect: frame,
            frame_insets: Insets {
                left: frame.left - normal.left,
                top: frame.top - normal.top,
                right: normal.right - frame.right,
                bottom: normal.bottom - frame.bottom,
            },
            show_state,
            monitor,
            z_order: state.windows.len(),
        });
        1
    }

    unsafe fn executable(pid: u32) -> Option<String> {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut value = vec![0u16; 32_768];
        let mut length = value.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, value.as_mut_ptr(), &mut length);
        CloseHandle(handle);
        (ok != 0).then(|| String::from_utf16_lossy(&value[..length as usize]))
    }

    unsafe fn monitor_from_handle(handle: HMONITOR) -> Monitor {
        let mut info: MONITORINFOEXW = zeroed();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        GetMonitorInfoW(handle, &mut info as *mut _ as _);
        let device = wide(&info.szDevice);
        let mut display: DISPLAY_DEVICEW = zeroed();
        display.cb = size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = EnumDisplayDevicesW(info.szDevice.as_ptr(), 0, &mut display, 0);
        let id = if ok != 0 {
            wide(&display.DeviceID)
        } else {
            device.clone()
        };
        let name = if ok != 0 {
            wide(&display.DeviceString)
        } else {
            device
        };
        let mut dpi_x = 96;
        let mut dpi_y = 96;
        let _ = GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        Monitor {
            id,
            name,
            work_area: rect(info.monitorInfo.rcWork),
            dpi: dpi_x,
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        }
    }

    unsafe fn monitor_for_window(hwnd: HWND) -> Monitor {
        monitor_from_handle(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST))
    }

    unsafe extern "system" fn enum_monitor(
        handle: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        (*(data as *mut Vec<Monitor>)).push(monitor_from_handle(handle));
        1
    }

    unsafe fn connected_monitors() -> Vec<Monitor> {
        let mut monitors = Vec::new();
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(enum_monitor),
            &mut monitors as *mut _ as LPARAM,
        );
        monitors
    }

    pub fn capture() -> Result<Vec<WindowSnapshot>, String> {
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let mut state = EnumState {
                own_pid: GetCurrentProcessId(),
                process_ids: None,
                windows: Vec::new(),
            };
            if EnumWindows(Some(enum_window), &mut state as *mut _ as LPARAM) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(state.windows)
        }
    }

    unsafe extern "system" fn enum_live(hwnd: HWND, data: LPARAM) -> BOOL {
        let state = &mut *(data as *mut (u32, Vec<LiveWindow>));
        if IsWindowVisible(hwnd) == 0
            || GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0
        {
            return 1;
        }
        let length = GetWindowTextLengthW(hwnd);
        let mut process = 0;
        GetWindowThreadProcessId(hwnd, &mut process);
        if length <= 0 || process == state.0 {
            return 1;
        }
        let Some(exe) = executable(process) else {
            return 1;
        };
        let mut title = vec![0u16; length as usize + 1];
        GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let mut class = vec![0u16; 256];
        GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        state.1.push(LiveWindow {
            handle: hwnd as isize,
            process_id: process,
            exe,
            class: wide(&class),
            title: wide(&title),
            z_order: state.1.len(),
        });
        1
    }

    pub fn restore(saved: &[WindowSnapshot]) -> Vec<RestoreResult> {
        unsafe {
            let mut live = (GetCurrentProcessId(), Vec::new());
            EnumWindows(Some(enum_live), &mut live as *mut _ as LPARAM);
            let matches = match_windows(saved, &live.1);
            let monitors = connected_monitors();
            saved
                .iter()
                .enumerate()
                .map(|(index, snapshot)| {
                    let Some(li) = matches[index] else {
                        return RestoreResult::NotRunning {
                            title: snapshot.title.clone(),
                        };
                    };
                    let hwnd = live.1[li].handle as HWND;
                    let current_monitor = monitor_for_window(hwnd);
                    let target_monitor = monitors
                        .iter()
                        .find(|monitor| monitor.id == snapshot.monitor.id)
                        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
                        .unwrap_or(&current_monitor);
                    let frame = if target_monitor.id == snapshot.monitor.id {
                        clamp_rect(snapshot.frame_rect, target_monitor.work_area)
                    } else {
                        map_rect(
                            snapshot.frame_rect,
                            snapshot.monitor.work_area,
                            target_monitor.work_area,
                            snapshot.monitor.dpi,
                            target_monitor.dpi,
                        )
                    };
                    let target = placement_from_frame(
                        frame,
                        snapshot.frame_insets,
                        snapshot.monitor.dpi,
                        target_monitor.dpi,
                    );
                    let mut placement: WINDOWPLACEMENT = zeroed();
                    placement.length = size_of::<WINDOWPLACEMENT>() as u32;
                    if GetWindowPlacement(hwnd, &mut placement) == 0 {
                        return RestoreResult::Failed {
                            title: snapshot.title.clone(),
                            error: std::io::Error::last_os_error().to_string(),
                        };
                    }
                    placement.showCmd = 1;
                    placement.rcNormalPosition = RECT {
                        left: target.left,
                        top: target.top,
                        right: target.right,
                        bottom: target.bottom,
                    };
                    if SetWindowPlacement(hwnd, &placement) == 0 {
                        return RestoreResult::Failed {
                            title: snapshot.title.clone(),
                            error: std::io::Error::last_os_error().to_string(),
                        };
                    }
                    placement.showCmd = match snapshot.show_state {
                        ShowState::Maximized => SW_SHOWMAXIMIZED as u32,
                        ShowState::Minimized => SW_SHOWMINIMIZED as u32,
                        ShowState::Normal => 1,
                    };
                    if SetWindowPlacement(hwnd, &placement) == 0 {
                        return RestoreResult::Failed {
                            title: snapshot.title.clone(),
                            error: std::io::Error::last_os_error().to_string(),
                        };
                    }
                    RestoreResult::Restored {
                        title: snapshot.title.clone(),
                    }
                })
                .collect()
        }
    }

    #[cfg(test)]
    pub unsafe fn move_window_for_process(process_id: u32, rect: Rect) -> bool {
        let mut live: (u32, Vec<LiveWindow>) = (GetCurrentProcessId(), Vec::new());
        EnumWindows(Some(enum_live), &mut live as *mut _ as LPARAM);
        let Some(window) = live.1.iter().find(|window| window.process_id == process_id) else {
            return false;
        };
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos(
            window.handle as HWND,
            std::ptr::null_mut(),
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE
                | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
        ) != 0
    }

    #[cfg(test)]
    pub unsafe fn capture_processes(process_ids: &[u32]) -> Vec<WindowSnapshot> {
        let mut state = EnumState {
            own_pid: GetCurrentProcessId(),
            process_ids: Some(process_ids.iter().copied().collect()),
            windows: Vec::new(),
        };
        EnumWindows(Some(enum_window), &mut state as *mut _ as LPARAM);
        state.windows
    }
}

#[cfg(windows)]
pub use platform::{capture, restore};

#[cfg(not(windows))]
pub fn capture() -> Result<Vec<WindowSnapshot>, String> {
    Err("WindowsKeeper only supports Windows".into())
}

#[cfg(not(windows))]
pub fn restore(saved: &[WindowSnapshot]) -> Vec<RestoreResult> {
    saved
        .iter()
        .map(|s| RestoreResult::Failed {
            title: s.title.clone(),
            error: "WindowsKeeper only supports Windows".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(id: &str, area: Rect, dpi: u32) -> Monitor {
        Monitor {
            id: id.into(),
            name: id.into(),
            work_area: area,
            dpi,
            primary: true,
        }
    }
    fn saved(title: &str, z: usize) -> WindowSnapshot {
        WindowSnapshot {
            exe: r"C:\Apps\Browser.exe".into(),
            class: "Chrome_WidgetWin_1".into(),
            title: title.into(),
            normal_rect: Rect::default(),
            frame_rect: Rect::default(),
            frame_insets: Insets::default(),
            show_state: ShowState::Normal,
            monitor: monitor("m", Rect::default(), 96),
            z_order: z,
        }
    }
    fn live(title: &str, z: usize) -> LiveWindow {
        LiveWindow {
            handle: z as isize + 10,
            process_id: z as u32 + 100,
            exe: r"c:/apps/browser.exe".into(),
            class: "chrome_widgetwin_1".into(),
            title: title.into(),
            z_order: z,
        }
    }

    #[test]
    fn matches_multiple_windows_by_title_without_reuse() {
        let result = match_windows(
            &[saved("Docs - Browser", 0), saved("Mail - Browser", 1)],
            &[live("Mail - Browser", 0), live("Docs - Browser", 1)],
        );
        assert_eq!(result, vec![Some(1), Some(0)]);
        assert_ne!(result[0], result[1]);
    }

    #[test]
    fn tolerates_changed_titles_and_uses_z_order_for_ties() {
        let result = match_windows(
            &[saved("Project - Browser", 0), saved("Project - Browser", 1)],
            &[live("Project – Browser", 0), live("Project – Browser", 1)],
        );
        assert_eq!(result, vec![Some(0), Some(1)]);
    }

    #[test]
    fn rejects_wrong_executable_or_class() {
        let mut other = live("Docs - Browser", 0);
        other.exe = "other.exe".into();
        assert_eq!(
            match_windows(&[saved("Docs - Browser", 0)], &[other]),
            vec![None]
        );
    }

    #[test]
    fn missing_monitor_maps_proportionally() {
        let from = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let to = Rect {
            left: 1920,
            top: 0,
            right: 3200,
            bottom: 984,
        };
        let mapped = map_rect(
            Rect {
                left: 960,
                top: 520,
                right: 1440,
                bottom: 920,
            },
            from,
            to,
            96,
            96,
        );
        assert_eq!(
            mapped,
            Rect {
                left: 2560,
                top: 492,
                right: 2880,
                bottom: 759
            }
        );
    }

    #[test]
    fn dpi_and_invisible_border_math_rounds_consistently() {
        let placement = placement_from_frame(
            Rect {
                left: 200,
                top: 100,
                right: 1000,
                bottom: 700,
            },
            Insets {
                left: 8,
                top: 1,
                right: 8,
                bottom: 8,
            },
            96,
            144,
        );
        assert_eq!(
            placement,
            Rect {
                left: 188,
                top: 98,
                right: 1012,
                bottom: 712
            }
        );
    }

    #[test]
    fn clamps_oversized_and_offscreen_rects() {
        let area = Rect {
            left: 0,
            top: 0,
            right: 1280,
            bottom: 720,
        };
        assert_eq!(
            clamp_rect(
                Rect {
                    left: -500,
                    top: 900,
                    right: 1500,
                    bottom: 1800
                },
                area
            ),
            area
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "opens and moves two test-owned Notepad windows"]
    fn live_notepad_round_trip_within_two_pixels() {
        use std::{
            fs,
            process::Command,
            thread,
            time::{Duration, Instant, SystemTime},
        };

        let token = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let root = std::env::temp_dir().join(format!("windowskeeper-live-{token}"));
        fs::create_dir_all(&root).unwrap();
        let names = [format!("wk-a-{token}.txt"), format!("wk-b-{token}.txt")];
        let paths = names.clone().map(|name| root.join(name));
        for path in &paths {
            fs::write(path, "WindowsKeeper live test").unwrap();
        }
        let mut children: Vec<_> = paths
            .iter()
            .map(|path| {
                Command::new("notepad.exe")
                    .arg("/newWindow")
                    .arg(path)
                    .spawn()
                    .unwrap()
            })
            .collect();
        let process_ids: Vec<u32> = children.iter().map(|child| child.id()).collect();

        let wait_for = |predicate: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(15);
            while Instant::now() < deadline {
                if predicate() {
                    return true;
                }
                thread::sleep(Duration::from_millis(50));
            }
            false
        };
        assert!(wait_for(&|| unsafe {
            platform::capture_processes(&process_ids).len() == 2
        }));
        let saved = unsafe { platform::capture_processes(&process_ids) };
        assert_eq!(saved.len(), 2);
        for (index, process_id) in process_ids.iter().enumerate() {
            let moved = Rect {
                left: 80 + index as i32 * 360,
                top: 80,
                right: 400 + index as i32 * 360,
                bottom: 360,
            };
            assert!(unsafe { platform::move_window_for_process(*process_id, moved) });
        }
        assert!(restore(&saved)
            .iter()
            .all(|result| matches!(result, RestoreResult::Restored { .. })));
        assert!(wait_for(&|| {
            let current = unsafe { platform::capture_processes(&process_ids) };
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
}
