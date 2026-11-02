// CCS/1 M:app-icons | IO:exe path -> 32x32 RGBA (UI turns it into a canvas data URL) | SEC:reads the shell icon only, never runs the exe

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Icon {
    pub size: u32,
    pub rgba: Vec<u8>,
}

/// BGRA rows from GetDIBits -> RGBA; an icon without alpha takes it from the AND mask (0 = opaque).
pub fn bgra_to_rgba(mut pixels: Vec<u8>, mask: Option<&[u8]>) -> Vec<u8> {
    let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
    for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = match mask {
                Some(mask) if mask.get(i * 4).copied().unwrap_or(0) != 0 => 0,
                _ => 255,
            };
        }
    }
    pixels
}

#[cfg(windows)]
pub fn app_icon(exe: &str) -> Option<Icon> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::{
        Graphics::Gdi::{
            DeleteObject, GetDC, GetDIBits, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
            DIB_RGB_COLORS,
        },
        UI::{
            Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
            WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO},
        },
    };
    const SIZE: i32 = 32;
    let path: Vec<u16> = exe.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut info: SHFILEINFOW = zeroed();
        if SHGetFileInfoW(
            path.as_ptr(),
            0,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        ) == 0
            || info.hIcon.is_null()
        {
            return None;
        }
        let mut parts: ICONINFO = zeroed();
        let ok = GetIconInfo(info.hIcon, &mut parts) != 0;
        let read = |bitmap| {
            let mut header: BITMAPINFO = zeroed();
            header.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE,
                biHeight: -SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..zeroed()
            };
            let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
            let dc = GetDC(std::ptr::null_mut());
            let lines = GetDIBits(dc, bitmap, 0, SIZE as u32, pixels.as_mut_ptr() as _, &mut header, DIB_RGB_COLORS);
            ReleaseDC(std::ptr::null_mut(), dc);
            (lines == SIZE).then_some(pixels)
        };
        let result = if ok && !parts.hbmColor.is_null() {
            let color = read(parts.hbmColor);
            let mask = if parts.hbmMask.is_null() { None } else { read(parts.hbmMask) };
            color.map(|color| Icon {
                size: SIZE as u32,
                rgba: bgra_to_rgba(color, mask.as_deref()),
            })
        } else {
            None
        };
        if ok {
            DeleteObject(parts.hbmColor as _);
            DeleteObject(parts.hbmMask as _);
        }
        DestroyIcon(info.hIcon);
        result
    }
}

#[cfg(not(windows))]
pub fn app_icon(_: &str) -> Option<Icon> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_bgra_and_falls_back_to_the_mask_for_alpha() {
        // two pixels, no alpha channel: first opaque (mask 0), second transparent (mask white)
        let pixels = vec![1, 2, 3, 0, 4, 5, 6, 0];
        let mask = [0, 0, 0, 0, 255, 255, 255, 0];
        assert_eq!(bgra_to_rgba(pixels, Some(&mask)), vec![3, 2, 1, 255, 6, 5, 4, 0]);
        // real alpha is kept as is
        assert_eq!(bgra_to_rgba(vec![1, 2, 3, 128], None), vec![3, 2, 1, 128]);
    }

    #[cfg(windows)]
    #[test]
    fn reads_the_notepad_icon() {
        let exe = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()) + r"\System32\notepad.exe";
        let icon = app_icon(&exe).expect("notepad has an icon");
        assert_eq!(icon.rgba.len(), 32 * 32 * 4);
        assert!(icon.rgba.chunks_exact(4).any(|p| p[3] > 0));
    }
}
