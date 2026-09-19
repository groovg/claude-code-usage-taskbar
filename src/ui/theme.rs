use std::path::PathBuf;

use eframe::egui;
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, GetDC, GetFontData, ReleaseDC, SelectObject, GDI_ERROR,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

use crate::localization::LanguageId;
use crate::ui::tokens::{CONTROL_CORNER_RADIUS, CONTROL_HEIGHT, DROPDOWN_CORNER_RADIUS};

const LUCIDE_FONT_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lucide-subset.ttf"));
const UI_FALLBACK_FONT_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ui-fallback.ttf"));

/// Installs the shared fonts, palette, widget visuals, and spacing used by the UI.
pub(crate) fn configure_style(context: &egui::Context, language: LanguageId) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "ui-fallback".into(),
        egui::FontData::from_static(UI_FALLBACK_FONT_BYTES).into(),
    );
    fonts.font_data.insert(
        "lucide".into(),
        egui::FontData::from_static(LUCIDE_FONT_BYTES).into(),
    );
    let native_menu_font = load_native_menu_font(&mut fonts);

    let mut proportional = Vec::new();
    if load_windows_font(&mut fonts, "segoe-ui", "segoeui.ttf") {
        proportional.push("segoe-ui".into());
    }
    load_language_fonts(&mut fonts, &mut proportional, language);
    proportional.push("ui-fallback".into());
    fonts
        .families
        .insert(egui::FontFamily::Proportional, proportional.clone());
    let mut native_menu_family = native_menu_font.into_iter().collect::<Vec<_>>();
    native_menu_family.extend(proportional.iter().cloned());
    native_menu_family.dedup();
    fonts.families.insert(
        egui::FontFamily::Name("native-menu".into()),
        native_menu_family,
    );

    let mut monospace = Vec::new();
    if load_windows_font(&mut fonts, "consolas", "consola.ttf") {
        monospace.push("consolas".into());
    }
    monospace.extend(proportional);
    fonts
        .families
        .insert(egui::FontFamily::Monospace, monospace);
    fonts.families.insert(
        egui::FontFamily::Name("ui-fallback".into()),
        vec!["ui-fallback".into()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("lucide".into()),
        vec!["lucide".into()],
    );
    context.set_fonts(fonts);

    // The dashboard uses dark surfaces regardless of the Windows app theme.
    // Keep egui's text colors and widget style on the same dark palette.
    context.set_theme(egui::Theme::Dark);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = MENU_SURFACE;
    visuals.window_fill = egui::Color32::from_rgb(38, 38, 38);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(48, 48, 48);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(58, 58, 58);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(65, 54, 51);
    visuals.selection.bg_fill = egui::Color32::from_rgb(76, 78, 84);
    visuals.faint_bg_color = egui::Color32::from_rgb(37, 37, 37);
    // Text edits use `extreme_bg_color`, while dropdowns and numeric fields use
    // the inactive widget surface. Keep them on the same surface so changing a
    // field type does not also change its apparent depth.
    visuals.extreme_bg_color = visuals.widgets.inactive.weak_bg_fill;
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(CONTROL_CORNER_RADIUS);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(CONTROL_CORNER_RADIUS);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(CONTROL_CORNER_RADIUS);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(CONTROL_CORNER_RADIUS);
    visuals.menu_corner_radius = egui::CornerRadius::same(DROPDOWN_CORNER_RADIUS);
    context.set_visuals(visuals);

    let mut style = (*context.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = egui::vec2(9.0, 8.0);
    // Keep intrinsic button content below CONTROL_HEIGHT so interact_size can
    // define one exact visual height for text, icon, and mixed-content buttons.
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = CONTROL_HEIGHT;
    style.spacing.indent = 16.0;
    context.set_style_of(egui::Theme::Dark, style);
}

/// Loads the exact font selected by Windows for native menus. Reading the
/// selected GDI font avoids assuming that every machine still uses Segoe UI.
fn load_native_menu_font(fonts: &mut egui::FontDefinitions) -> Option<String> {
    unsafe {
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some(std::ptr::from_mut(&mut metrics).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;

        let face_length = metrics
            .lfMenuFont
            .lfFaceName
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(metrics.lfMenuFont.lfFaceName.len());
        let face = String::from_utf16_lossy(&metrics.lfMenuFont.lfFaceName[..face_length]);
        if face.eq_ignore_ascii_case("Segoe UI")
            && load_windows_font(fonts, "native-menu-face", "segoeui.ttf")
        {
            return Some("native-menu-face".into());
        }
        if face.eq_ignore_ascii_case("Segoe UI Variable")
            && load_windows_font(fonts, "native-menu-face", "SegUIVar.ttf")
        {
            return Some("native-menu-face".into());
        }

        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return None;
        }
        let font = CreateFontIndirectW(std::ptr::from_ref(&metrics.lfMenuFont));
        if font.is_invalid() {
            ReleaseDC(None, hdc);
            return None;
        }
        let previous = SelectObject(hdc, font.into());
        let size = GetFontData(hdc, 0, 0, None, 0);
        let mut bytes = if size == GDI_ERROR as u32 || size == 0 {
            None
        } else {
            let mut bytes = vec![0; size as usize];
            (GetFontData(hdc, 0, 0, Some(bytes.as_mut_ptr().cast()), size) == size).then_some(bytes)
        };
        if !previous.is_invalid() {
            SelectObject(hdc, previous);
        }
        let _ = DeleteObject(font.into());
        ReleaseDC(None, hdc);

        let bytes = bytes.take()?;
        let name = "native-menu-face".to_string();
        fonts
            .font_data
            .insert(name.clone(), egui::FontData::from_owned(bytes).into());
        Some(name)
    }
}

fn load_windows_font(fonts: &mut egui::FontDefinitions, name: &str, file_name: &str) -> bool {
    let windows_directory = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let Ok(bytes) = std::fs::read(windows_directory.join("Fonts").join(file_name)) else {
        return false;
    };
    fonts
        .font_data
        .insert(name.into(), egui::FontData::from_owned(bytes).into());
    true
}

fn load_language_fonts(
    fonts: &mut egui::FontDefinitions,
    family: &mut Vec<String>,
    preferred_language: LanguageId,
) {
    // Every language selector uses native names. Prefer the active language's
    // glyph forms, then add the other Windows UI script fonts as fallbacks so
    // every native name remains readable in a single selector.
    for language in std::iter::once(preferred_language).chain(
        LanguageId::ALL
            .into_iter()
            .filter(|language| *language != preferred_language),
    ) {
        let candidate = language.windows_font();
        if let Some((name, file_name)) = candidate {
            if !family.iter().any(|existing| existing == name)
                && load_windows_font(fonts, name, file_name)
            {
                family.push(name.to_owned());
            }
        }
    }
}

pub(crate) const ACCENT: egui::Color32 = egui::Color32::from_rgb(217, 119, 87);
pub(crate) const ACCENT_HOVER_BORDER: egui::Color32 = egui::Color32::from_rgb(157, 73, 45);
pub(crate) const MENU_SURFACE: egui::Color32 = egui::Color32::from_rgb(32, 32, 32);
pub(crate) const MUTED: egui::Color32 = egui::Color32::from_rgb(143, 146, 156);
pub(crate) const SELECTED_MENU_FILL: egui::Color32 = egui::Color32::from_rgb(49, 49, 49);
pub(crate) const HELPER_SURFACE: egui::Color32 = egui::Color32::from_rgb(21, 22, 26);
pub(crate) const HELPER_CARD_SURFACE: egui::Color32 = egui::Color32::from_rgb(24, 25, 29);
pub(crate) const HELPER_BORDER: egui::Color32 = egui::Color32::from_rgb(48, 50, 57);
pub(crate) const SUCCESS: egui::Color32 = egui::Color32::from_rgb(78, 201, 143);
pub(crate) const DANGER: egui::Color32 = egui::Color32::from_rgb(232, 119, 95);
pub(crate) const TOGGLE_INACTIVE: egui::Color32 = egui::Color32::from_rgb(72, 72, 72);
pub(crate) const TOGGLE_INACTIVE_HOVER: egui::Color32 = egui::Color32::from_rgb(92, 92, 92);
pub(crate) const TOGGLE_KNOB: egui::Color32 = egui::Color32::from_rgb(245, 245, 245);
pub(crate) const TOGGLE_LABEL: egui::Color32 = egui::Color32::from_rgb(218, 218, 218);
pub(crate) const ANCHOR_OUTLINE: egui::Color32 = egui::Color32::from_rgb(128, 131, 140);
pub(crate) const ANCHOR_IDLE_FILL: egui::Color32 = egui::Color32::from_rgb(35, 37, 42);
pub(crate) const CHECKERBOARD_DARK: egui::Color32 = egui::Color32::from_gray(72);
pub(crate) const CHECKERBOARD_LIGHT: egui::Color32 = egui::Color32::from_gray(176);
pub(crate) const SECTION_SURFACE: egui::Color32 = egui::Color32::from_rgb(35, 35, 35);
pub(crate) const SECTION_BORDER: egui::Color32 = egui::Color32::from_rgb(54, 54, 54);
pub(crate) const SETTING_SEPARATOR_COLOR: egui::Color32 = egui::Color32::from_rgb(53, 53, 53);
pub(crate) const MENU_HOVER: egui::Color32 = egui::Color32::from_rgb(42, 42, 42);
pub(crate) const MENU_TEXT: egui::Color32 = egui::Color32::from_rgb(245, 245, 245);
pub(crate) const ASSET_CARD_SELECTED: egui::Color32 = egui::Color32::from_rgb(57, 48, 46);
pub(crate) const ASSET_CARD_SURFACE: egui::Color32 = egui::Color32::from_rgb(31, 32, 36);
pub(crate) const ASSET_CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(55, 57, 64);
pub(crate) const ASSET_PREVIEW_SURFACE: egui::Color32 = egui::Color32::from_rgb(24, 25, 28);
pub(crate) const SPLITTER_HOVER_SURFACE: egui::Color32 = egui::Color32::from_rgb(38, 40, 46);
pub(crate) const SPLITTER_IDLE: egui::Color32 = egui::Color32::from_rgb(65, 68, 76);
