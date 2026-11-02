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
    pub bounds: Rect,
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

pub fn workspace_to_screen(rect: Rect, offset: (i32, i32)) -> Rect {
    Rect {
        left: rect.left + offset.0,
        top: rect.top + offset.1,
        right: rect.right + offset.0,
        bottom: rect.bottom + offset.1,
    }
}

pub fn screen_to_workspace(rect: Rect, offset: (i32, i32)) -> Rect {
    workspace_to_screen(rect, (-offset.0, -offset.1))
}

fn primary_workspace_offset(monitors: &[Monitor]) -> (i32, i32) {
    monitors
        .iter()
        .find(|monitor| monitor.primary)
        .map(|monitor| {
            (
                monitor.work_area.left - monitor.bounds.left,
                monitor.work_area.top - monitor.bounds.top,
            )
        })
        .unwrap_or((0, 0))
}

pub fn target_normal_rect(saved: &WindowSnapshot, target: &Monitor) -> Rect {
    if saved.monitor.id == target.id && saved.monitor.dpi == target.dpi {
        clamp_rect(saved.normal_rect, target.work_area)
    } else {
        map_rect(
            saved.normal_rect,
            saved.monitor.work_area,
            target.work_area,
            saved.monitor.dpi,
            target.dpi,
        )
    }
}

/// Saved monitor if connected, else the primary one (restore and the UI preview share this).
pub fn pick_target_monitor<'a>(saved: &WindowSnapshot, monitors: &'a [Monitor]) -> Option<&'a Monitor> {
    monitors
        .iter()
        .find(|monitor| monitor.id == saved.monitor.id)
        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| monitors.first())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Landing {
    pub monitor_id: String,
    pub rect: Rect,
}

/// Where each saved window would land on the given monitors, in screen coordinates.
pub fn preview(saved: &[WindowSnapshot], monitors: &[Monitor]) -> Vec<Option<Landing>> {
    saved
        .iter()
        .map(|window| {
            pick_target_monitor(window, monitors).map(|target| Landing {
                monitor_id: target.id.clone(),
                rect: target_normal_rect(window, target),
            })
        })
        .collect()
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
                GetWindowPlacement, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
                GetWindowThreadProcessId, IsWindowVisible, SetWindowPlacement, GWL_EXSTYLE,
                MONITORINFOF_PRIMARY, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, WINDOWPLACEMENT,
                WS_EX_TOOLWINDOW,
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
        workspace_offset: (i32, i32),
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
        let normal = workspace_to_screen(rect(placement.rcNormalPosition), state.workspace_offset);
        let frame = rect(frame);
        let monitor = monitor_for_window(hwnd);
        let show_state = match placement.showCmd {
            value if value == SW_SHOWMAXIMIZED as u32 => ShowState::Maximized,
            value if value == SW_SHOWMINIMIZED as u32 => ShowState::Minimized,
            _ => ShowState::Normal,
        };
        let frame_insets = if show_state == ShowState::Normal {
            let mut window_rect: RECT = zeroed();
            if GetWindowRect(hwnd, &mut window_rect) != 0 {
                let window_rect = rect(window_rect);
                Insets {
                    left: frame.left - window_rect.left,
                    top: frame.top - window_rect.top,
                    right: window_rect.right - frame.right,
                    bottom: window_rect.bottom - frame.bottom,
                }
            } else {
                Insets::default()
            }
        } else {
            Insets::default()
        };
        state.windows.push(WindowSnapshot {
            exe,
            class: wide(&class),
            title: wide(&title),
            normal_rect: normal,
            frame_rect: frame,
            frame_insets,
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
        let name = friendly_name(&device).unwrap_or_else(|| {
            if ok != 0 {
                wide(&display.DeviceString)
            } else {
                device
            }
        });
        let mut dpi_x = 96;
        let mut dpi_y = 96;
        let _ = GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        Monitor {
            id,
            name,
            bounds: rect(info.monitorInfo.rcMonitor),
            work_area: rect(info.monitorInfo.rcWork),
            dpi: dpi_x,
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        }
    }

    /// EDID name ("DELL U2723QE") via DisplayConfig; EnumDisplayDevices only says "Generic PnP Monitor".
    unsafe fn friendly_name(gdi_device: &str) -> Option<String> {
        use windows_sys::Win32::Devices::Display::{
            DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
            DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
            DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
        };
        let (mut path_count, mut mode_count) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count) != 0 {
            return None;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            std::ptr::null_mut(),
        ) != 0
        {
            return None;
        }
        paths.truncate(path_count as usize);
        paths.into_iter().find_map(|path| {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            source.header.adapterId = path.sourceInfo.adapterId;
            source.header.id = path.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0
                || wide(&source.viewGdiDeviceName) != gdi_device
            {
                return None;
            }
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            target.header.adapterId = path.targetInfo.adapterId;
            target.header.id = path.targetInfo.id;
            let name = (DisplayConfigGetDeviceInfo(&mut target.header) == 0)
                .then(|| wide(&target.monitorFriendlyDeviceName))?;
            (!name.trim().is_empty()).then_some(name)
        })
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

    pub fn monitors() -> Vec<Monitor> {
        unsafe {
            enable_dpi_awareness();
            connected_monitors()
        }
    }

    pub fn capture() -> Result<Vec<WindowSnapshot>, String> {
        unsafe {
            enable_dpi_awareness();
            let monitors = connected_monitors();
            let mut state = EnumState {
                own_pid: GetCurrentProcessId(),
                process_ids: None,
                workspace_offset: primary_workspace_offset(&monitors),
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
            enable_dpi_awareness();
            let mut live = (GetCurrentProcessId(), Vec::new());
            EnumWindows(Some(enum_live), &mut live as *mut _ as LPARAM);
            let matches = match_windows(saved, &live.1);
            let monitors = connected_monitors();
            let workspace_offset = primary_workspace_offset(&monitors);
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
                    let target_monitor =
                        pick_target_monitor(snapshot, &monitors).unwrap_or(&current_monitor);
                    let target_screen = target_normal_rect(snapshot, target_monitor);
                    let target = screen_to_workspace(target_screen, workspace_offset);
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
    pub unsafe fn foreground_process_id() -> u32 {
        let window = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        let mut process_id = 0;
        GetWindowThreadProcessId(window, &mut process_id);
        process_id
    }

    #[cfg(test)]
    pub unsafe fn capture_processes(process_ids: &[u32]) -> Vec<WindowSnapshot> {
        enable_dpi_awareness();
        let monitors = connected_monitors();
        let mut state = EnumState {
            own_pid: GetCurrentProcessId(),
            process_ids: Some(process_ids.iter().copied().collect()),
            workspace_offset: primary_workspace_offset(&monitors),
            windows: Vec::new(),
        };
        EnumWindows(Some(enum_window), &mut state as *mut _ as LPARAM);
        state.windows
    }

    pub fn enable_dpi_awareness() {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }
}

#[cfg(windows)]
pub use platform::{capture, enable_dpi_awareness, monitors, restore};

#[cfg(all(test, windows))]
pub(crate) use platform::{
    capture_processes as test_capture_processes,
    foreground_process_id as test_foreground_process_id,
    move_window_for_process as test_move_window_for_process,
};

#[cfg(not(windows))]
pub fn capture() -> Result<Vec<WindowSnapshot>, String> {
    Err("PaneKeeper only supports Windows".into())
}

#[cfg(not(windows))]
pub fn enable_dpi_awareness() {}

#[cfg(not(windows))]
pub fn monitors() -> Vec<Monitor> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn restore(saved: &[WindowSnapshot]) -> Vec<RestoreResult> {
    saved
        .iter()
        .map(|s| RestoreResult::Failed {
            title: s.title.clone(),
            error: "PaneKeeper only supports Windows".into(),
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
            bounds: area,
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
    fn preview_lands_on_the_saved_monitor_or_the_primary_one() {
        let area = |left| Rect { left, top: 0, right: left + 1920, bottom: 1040 };
        let mut secondary = monitor("B", area(1920), 96);
        secondary.primary = false;
        let mut on_a = saved("A window", 0);
        on_a.monitor = monitor("A", area(0), 96);
        on_a.normal_rect = Rect { left: 100, top: 100, right: 900, bottom: 700 };
        let mut on_gone = on_a.clone();
        on_gone.monitor = monitor("gone", area(-1920), 96);
        on_gone.normal_rect = Rect { left: -1820, top: 100, right: -1020, bottom: 700 };
        let landed = preview(&[on_a, on_gone], &[secondary, monitor("A", area(0), 96)]);
        let landed: Vec<_> = landed.into_iter().map(Option::unwrap).collect();
        assert_eq!(landed[0].monitor_id, "A");
        assert_eq!(landed[0].rect, Rect { left: 100, top: 100, right: 900, bottom: 700 });
        assert_eq!(landed[1].monitor_id, "A");
        assert!(landed[1].rect.left >= 0 && landed[1].rect.right <= 1920);
        assert!(preview(&[saved("x", 0)], &[]).pop().unwrap().is_none());
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

    #[test]
    fn top_taskbar_workspace_coordinates_round_trip() {
        let workspace = Rect {
            left: 100,
            top: 100,
            right: 900,
            bottom: 700,
        };
        let screen = workspace_to_screen(workspace, (0, 40));
        assert_eq!(
            screen,
            Rect {
                left: 100,
                top: 140,
                right: 900,
                bottom: 740
            }
        );
        assert_eq!(screen_to_workspace(screen, (0, 40)), workspace);
    }

    #[test]
    fn minimized_and_maximized_normal_rects_stay_visible_and_keep_size() {
        let source_area = Rect {
            left: 0,
            top: 40,
            right: 1920,
            bottom: 1080,
        };
        let same = monitor("saved", source_area, 96);
        let missing_target = monitor(
            "replacement",
            Rect {
                left: 1920,
                top: 0,
                right: 3840,
                bottom: 1040,
            },
            96,
        );
        for state in [ShowState::Minimized, ShowState::Maximized] {
            let mut snapshot = saved("Stateful", 0);
            snapshot.show_state = state;
            snapshot.monitor = same.clone();
            snapshot.normal_rect = Rect {
                left: 100,
                top: 140,
                right: 900,
                bottom: 740,
            };
            snapshot.frame_rect = Rect {
                left: -32_000,
                top: -32_000,
                right: -31_840,
                bottom: -31_972,
            };
            snapshot.frame_insets = Insets {
                left: -32_100,
                top: -32_140,
                right: 32_740,
                bottom: 32_712,
            };

            let on_same = target_normal_rect(&snapshot, &same);
            let on_missing = target_normal_rect(&snapshot, &missing_target);
            for (rect, area) in [
                (on_same, same.work_area),
                (on_missing, missing_target.work_area),
            ] {
                assert_eq!((rect.width(), rect.height()), (800, 600));
                assert!(rect.left >= area.left && rect.top >= area.top);
                assert!(rect.right <= area.right && rect.bottom <= area.bottom);
            }
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "opens, moves and restores two test-owned windows (examples/testwin.rs)"]
    fn live_round_trip_within_two_pixels() {
        let rect = |left| Rect { left, top: 160, right: left + 520, bottom: 560 };
        let test = crate::testwin::TestWindows::open("engine", &[rect(120), rect(700)]);
        let saved = test.capture();
        assert_eq!(saved.len(), 2);
        for (index, pid) in test.pids.iter().enumerate() {
            let moved = Rect { left: 80 + index as i32 * 360, top: 80, right: 400 + index as i32 * 360, bottom: 360 };
            assert!(unsafe { platform::move_window_for_process(*pid, moved) });
        }
        assert!(restore(&saved).iter().all(|result| matches!(result, RestoreResult::Restored { .. })));
        assert!(test.back_in_place(&saved));
    }
}
