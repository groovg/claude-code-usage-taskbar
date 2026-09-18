use windows::core::PCWSTR;
use windows::Win32::System::Registry::*;

use crate::native_interop::wide_str;

const PERSONALIZE_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const LIGHT_THEME_KEY: &str = "SystemUsesLightTheme";
const TASKBAR_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const TASKBAR_ALIGNMENT_KEY: &str = "TaskbarAl";
const VERSION_PATH: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const BUILD_NUMBER_KEY: &str = "CurrentBuildNumber";
const FIRST_WINDOWS_11_BUILD: u32 = 22000;

/// Check if the system is in dark mode by reading the registry
pub fn is_dark_mode() -> bool {
    !is_light_theme()
}

fn is_light_theme() -> bool {
    // Default to dark mode when the value cannot be read.
    read_dword(HKEY_CURRENT_USER, PERSONALIZE_PATH, LIGHT_THEME_KEY) == Some(1)
}

/// Whether the taskbar buttons sit in the middle of the bar, leaving its left
/// end free for a widget. Windows 11 centres them unless the user picked
/// "Left" in taskbar settings (the value is absent until they touch it);
/// Windows 10 always starts at the left edge.
pub fn taskbar_buttons_centered() -> bool {
    match read_dword(HKEY_CURRENT_USER, TASKBAR_PATH, TASKBAR_ALIGNMENT_KEY) {
        Some(alignment) => alignment == 1,
        None => windows_build().is_some_and(|build| build >= FIRST_WINDOWS_11_BUILD),
    }
}

fn windows_build() -> Option<u32> {
    let bytes = read_value(HKEY_LOCAL_MACHINE, VERSION_PATH, BUILD_NUMBER_KEY)?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
        .trim_end_matches('\0')
        .trim()
        .parse()
        .ok()
}

fn read_dword(root: HKEY, path: &str, name: &str) -> Option<u32> {
    let bytes = read_value(root, path, name)?;
    (bytes.len() >= 4).then(|| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_value(root: HKEY, path: &str, name: &str) -> Option<Vec<u8>> {
    unsafe {
        let path = wide_str(path);
        let name = wide_str(name);

        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            root,
            PCWSTR::from_raw(path.as_ptr()),
            None,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return None;
        }

        let mut size: u32 = 0;
        let result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(name.as_ptr()),
            None,
            None,
            None,
            Some(&mut size),
        );
        if result.is_err() || size == 0 {
            let _ = RegCloseKey(hkey);
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        let result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(name.as_ptr()),
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);
        result.is_ok().then_some(buffer)
    }
}
