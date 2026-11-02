#[cfg(windows)]
mod platform {
    use std::{io, os::windows::ffi::OsStrExt, path::Path};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }

    pub fn set(enabled: bool, executable: &Path) -> Result<(), String> {
        let path = wide(std::ffi::OsStr::new(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        ));
        let name = wide(std::ffi::OsStr::new("PaneKeeper"));
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null_mut(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32).to_string());
        }
        let result = if enabled {
            let command = format!("\"{}\" --autostart", executable.display());
            let value = wide(std::ffi::OsStr::new(&command));
            unsafe {
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr() as _,
                    (value.len() * 2) as u32,
                )
            }
        } else {
            unsafe { RegDeleteValueW(key, name.as_ptr()) }
        };
        unsafe {
            RegCloseKey(key);
        }
        if result == 0 || (!enabled && result == 2) {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(result as i32).to_string())
        }
    }
}

#[cfg(windows)]
pub use platform::set;

#[cfg(not(windows))]
pub fn set(_: bool, _: &std::path::Path) -> Result<(), String> {
    Ok(())
}
