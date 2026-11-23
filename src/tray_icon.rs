use std::sync::Mutex;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Shell::{
    ExtractIconExW, Shell_NotifyIconGetRect, Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE,
    NIF_TIP, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::native_interop::WM_APP_TRAY;

const APP_TRAY_ICON_ID: u32 = 1;
const THEME_TRAY_ICON_ID_BASE: u32 = 1_000;

static REGISTERED_THEME_ICON_IDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Rasterized Theme Studio root to expose as a genuine notification-area icon.
pub struct ThemedTrayIcon {
    pub surface_index: usize,
    pub tooltip: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

/// Load the application icons embedded by build.rs from src/icons/icon.ico.
/// Native windows and the system tray share this source so Windows can choose
/// the exact large or small icon instead of scaling a single bitmap.
pub fn load_app_icons() -> (HICON, HICON) {
    // current_exe has no MAX_PATH limit, unlike a fixed GetModuleFileNameW buffer.
    let Ok(exe) = std::env::current_exe() else {
        return (HICON::default(), HICON::default());
    };
    let exe = windows::core::HSTRING::from(exe.as_os_str());
    unsafe {
        let mut small_icon = HICON::default();
        let mut large_icon = HICON::default();
        let extracted = ExtractIconExW(&exe, 0, Some(&mut large_icon), Some(&mut small_icon), 1);

        if extracted == 0 {
            (HICON::default(), HICON::default())
        } else {
            (large_icon, small_icon)
        }
    }
}

fn load_app_icon() -> HICON {
    let (large_icon, small_icon) = load_app_icons();
    if !small_icon.is_invalid() {
        if !large_icon.is_invalid() {
            unsafe {
                let _ = DestroyIcon(large_icon);
            }
        }
        small_icon
    } else {
        large_icon
    }
}

fn themed_icon_id(surface_index: usize) -> u32 {
    THEME_TRAY_ICON_ID_BASE.saturating_add(surface_index.min(u32::MAX as usize) as u32)
}

pub fn themed_surface_index(id: u32) -> Option<usize> {
    (id >= THEME_TRAY_ICON_ID_BASE).then(|| (id - THEME_TRAY_ICON_ID_BASE) as usize)
}

pub fn cursor_over_themed_icon(hwnd: HWND, surface_index: usize) -> bool {
    unsafe {
        let identifier = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: hwnd,
            uID: themed_icon_id(surface_index),
            ..Default::default()
        };
        let Ok(rect) = Shell_NotifyIconGetRect(&identifier) else {
            return false;
        };
        let mut point = POINT::default();
        GetCursorPos(&mut point).is_ok()
            && point.x >= rect.left
            && point.x < rect.right
            && point.y >= rect.top
            && point.y < rect.bottom
    }
}

fn create_themed_icon(icon: &ThemedTrayIcon) -> HICON {
    if icon.width == 0
        || icon.height == 0
        // Explorer ultimately displays one square notification-area slot. A
        // bounded source prevents an accidental Studio expression from asking
        // GDI and the shell to retain an enormous icon bitmap.
        || icon.width > 512
        || icon.height > 512
        || icon.pixels.len() != icon.width as usize * icon.height as usize
    {
        return HICON::default();
    }

    unsafe {
        let screen_dc = GetDC(None);
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: icon.width as i32,
                // Theme pixels are top-down, so use a top-down DIB as well.
                biHeight: -(icon.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let color_bitmap = CreateDIBSection(
            Some(memory_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
        .unwrap_or_default();
        if color_bitmap.is_invalid() || bits.is_null() {
            let _ = DeleteDC(memory_dc);
            ReleaseDC(None, screen_dc);
            return HICON::default();
        }
        let target = std::slice::from_raw_parts_mut(bits.cast::<u32>(), icon.pixels.len());
        for (target, source) in target.iter_mut().zip(&icon.pixels) {
            *target = unpremultiply(*source);
        }

        // The 32-bit colour bitmap's alpha channel defines the transparent
        // pixels and antialiased edges. Windows falls back to the mask when
        // every alpha is zero, so an all-ones mask keeps that icon transparent.
        let mask_row_bytes = (icon.width as usize).div_ceil(16) * 2;
        let mask = vec![0xFF_u8; mask_row_bytes * icon.height as usize];
        let mask_bitmap = CreateBitmap(
            icon.width as i32,
            icon.height as i32,
            1,
            1,
            Some(mask.as_ptr().cast()),
        );
        let icon_info = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bitmap,
            hbmColor: color_bitmap,
        };
        let result = CreateIconIndirect(&icon_info).unwrap_or_default();

        let _ = DeleteObject(mask_bitmap.into());
        let _ = DeleteObject(color_bitmap.into());
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, screen_dc);
        result
    }
}

/// The renderer's pixels are premultiplied ARGB; icon pixels are straight
/// alpha, so a translucent edge would otherwise draw too dark.
fn unpremultiply(pixel: u32) -> u32 {
    let alpha = pixel >> 24;
    match alpha {
        0 => 0,
        255 => pixel,
        _ => {
            let channel =
                |shift: u32| ((((pixel >> shift) & 0xFF) * 255 + alpha / 2) / alpha).min(255);
            (alpha << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
        }
    }
}

fn icon_data(hwnd: HWND, id: u32) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: id,
        ..Default::default()
    }
}

/// Register or refresh one notification icon, then release `hicon`.
fn add_or_modify(hwnd: HWND, id: u32, hicon: HICON, tooltip: &str) {
    let mut nid = NOTIFYICONDATAW {
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_APP_TRAY,
        hIcon: hicon,
        ..icon_data(hwnd, id)
    };
    copy_wide(tooltip, &mut nid.szTip);
    unsafe {
        // NIM_ADD succeeds on first registration. If the icon is already
        // present, NIM_MODIFY refreshes its image, callback and tooltip.
        if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
        let _ = DestroyIcon(hicon);
    }
}

fn remove_id(hwnd: HWND, id: u32) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd, id));
    }
}

fn remove_registered_theme_icons(hwnd: HWND) {
    let mut registered = REGISTERED_THEME_ICON_IDS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for id in registered.drain(..) {
        remove_id(hwnd, id);
    }
}

/// Register or refresh the single persistent application tray icon.
pub fn sync(hwnd: HWND, tooltip: &str) {
    remove_registered_theme_icons(hwnd);
    let hicon = load_app_icon();
    if hicon.is_invalid() {
        return;
    }
    add_or_modify(hwnd, APP_TRAY_ICON_ID, hicon, tooltip);
}

/// Register Theme Studio roots as independent notification-area icons. The
/// shell owns their order and overflow placement just like every other app icon.
/// Returns false when none could be shown; the app icon then stays registered.
pub fn sync_themed(hwnd: HWND, icons: &[ThemedTrayIcon]) -> bool {
    let mut refreshed_ids = Vec::with_capacity(icons.len());
    for icon in icons {
        let hicon = create_themed_icon(icon);
        if hicon.is_invalid() {
            continue;
        }
        let id = themed_icon_id(icon.surface_index);
        add_or_modify(hwnd, id, hicon, &icon.tooltip);
        refreshed_ids.push(id);
    }

    let mut registered = REGISTERED_THEME_ICON_IDS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for id in registered
        .iter()
        .copied()
        .filter(|id| !refreshed_ids.contains(id))
    {
        remove_id(hwnd, id);
    }
    let shown = !refreshed_ids.is_empty();
    if shown {
        remove_id(hwnd, APP_TRAY_ICON_ID);
    }
    *registered = refreshed_ids;
    shown
}

/// Show a Windows balloon notification from the application tray icon.
pub fn notify_balloon(hwnd: HWND, title: &str, message: &str) {
    let icon_id = REGISTERED_THEME_ICON_IDS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .first()
        .copied()
        .unwrap_or(APP_TRAY_ICON_ID);
    let mut nid = NOTIFYICONDATAW {
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_WARNING,
        ..icon_data(hwnd, icon_id)
    };
    copy_wide(title, &mut nid.szInfoTitle);
    copy_wide(message, &mut nid.szInfo);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

/// Remove the application tray icon from the shell.
pub fn remove_all(hwnd: HWND) {
    remove_id(hwnd, APP_TRAY_ICON_ID);
    remove_registered_theme_icons(hwnd);
}

fn copy_wide<const N: usize>(value: &str, buffer: &mut [u16; N]) {
    let wide: Vec<u16> = value.encode_utf16().collect();
    let mut len = wide.len().min(N - 1);
    if len > 0 && (0xD800..=0xDBFF).contains(&wide[len - 1]) {
        len -= 1;
    }
    buffer[..len].copy_from_slice(&wide[..len]);
    buffer[len] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_icon_ids_do_not_overlap_the_application_icon() {
        assert_ne!(themed_icon_id(0), APP_TRAY_ICON_ID);
        assert_ne!(themed_icon_id(42), themed_icon_id(43));
    }

    #[test]
    fn icons_get_straight_alpha_pixels_and_a_transparent_mask() {
        assert_eq!(unpremultiply(0xFF12_3456), 0xFF12_3456);
        assert_eq!(unpremultiply(0x0012_3456), 0);
        // 50% white, premultiplied, must stay white rather than turn grey.
        let icon = create_themed_icon(&ThemedTrayIcon {
            surface_index: 0,
            tooltip: String::new(),
            width: 16,
            height: 2,
            pixels: vec![0x8080_8080; 32],
        });
        assert!(!icon.is_invalid());
        let mut color = [0u32; 32];
        let mut mask = [0u8; 4];
        unsafe {
            let mut info = ICONINFO::default();
            GetIconInfo(icon, &mut info).unwrap();
            GetBitmapBits(info.hbmColor, 128, color.as_mut_ptr().cast());
            GetBitmapBits(info.hbmMask, 4, mask.as_mut_ptr().cast());
            let _ = DeleteObject(info.hbmColor.into());
            let _ = DeleteObject(info.hbmMask.into());
            let _ = DestroyIcon(icon);
        }
        assert!(
            color.iter().all(|pixel| *pixel == 0x80FF_FFFF),
            "{color:x?}"
        );
        assert_eq!(mask, [0xFF; 4]);
    }
}
