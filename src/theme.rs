use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::native_interop::{read_registry_string, read_registry_value};

const PERSONALIZE_PATH: PCWSTR =
    w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
const LIGHT_THEME_KEY: PCWSTR = w!("SystemUsesLightTheme");
const TASKBAR_PATH: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced");
const TASKBAR_ALIGNMENT_KEY: PCWSTR = w!("TaskbarAl");
const VERSION_PATH: PCWSTR = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
const BUILD_NUMBER_KEY: PCWSTR = w!("CurrentBuildNumber");
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
    read_registry_string(HKEY_LOCAL_MACHINE, VERSION_PATH, BUILD_NUMBER_KEY)?
        .trim()
        .parse()
        .ok()
}

fn read_dword(root: HKEY, path: PCWSTR, name: PCWSTR) -> Option<u32> {
    let bytes = read_registry_value(root, path, name)?;
    bytes.first_chunk::<4>().copied().map(u32::from_le_bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn registry_reads_the_windows_build_number() {
        assert!(super::windows_build().is_some_and(|build| build >= 10_000));
    }
}
