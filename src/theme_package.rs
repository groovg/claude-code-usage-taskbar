//! Portable Theme Studio packages containing a theme, its menu, and used assets.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::context_menu::{self, ContextMenuDocument};
use crate::theme_engine::{self, managed_asset_file_name, LayerBackground, ThemeDocument};

const THEME_ENTRY: &str = "theme.json";
const CONTEXT_MENU_ENTRY: &str = "context-menu.json";
const ASSET_PREFIX: &str = "assets/";
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 1_024;

#[derive(Clone, Debug)]
pub struct ImportedTheme {
    pub theme: ThemeDocument,
    pub theme_path: PathBuf,
    pub context_menu: Option<(ContextMenuDocument, PathBuf)>,
    pub imported_assets: usize,
}

#[derive(Debug)]
struct DecodedPackage {
    theme: ThemeDocument,
    context_menu: Option<ContextMenuDocument>,
    assets: HashMap<String, Vec<u8>>,
}

pub fn is_theme_package(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

pub fn export_package(
    destination: &Path,
    theme: &ThemeDocument,
    context_menu: &ContextMenuDocument,
) -> Result<usize, String> {
    theme.check()?;
    context_menu.check()?;

    let asset_paths = theme_asset_paths(theme);
    let theme_bytes = serde_json::to_vec_pretty(theme).map_err(|error| error.to_string())?;
    let context_menu_bytes =
        serde_json::to_vec_pretty(context_menu).map_err(|error| error.to_string())?;
    if theme_bytes.len() as u64 > MAX_ENTRY_BYTES {
        return Err("theme.json exceeds the 64 MB package limit".into());
    }
    if context_menu_bytes.len() as u64 > MAX_ENTRY_BYTES {
        return Err("context-menu.json exceeds the 64 MB package limit".into());
    }
    let mut package_bytes = theme_bytes.len() as u64 + context_menu_bytes.len() as u64;
    let mut assets = Vec::with_capacity(asset_paths.len());
    for relative_path in &asset_paths {
        let file_name = managed_asset_file_name(relative_path)?;
        let source = theme_engine::assets_directory().join(file_name);
        let bytes = std::fs::read(&source).map_err(|error| {
            format!(
                "Theme asset '{}' could not be read: {error}",
                source.display()
            )
        })?;
        if bytes.len() as u64 > MAX_ENTRY_BYTES {
            return Err(format!(
                "Theme asset '{}' exceeds the 64 MB package limit",
                source.display()
            ));
        }
        package_bytes = package_bytes.saturating_add(bytes.len() as u64);
        if package_bytes > MAX_PACKAGE_BYTES {
            return Err("Theme package expands beyond the 256 MB limit".into());
        }
        assets.push((relative_path, bytes));
    }

    let file = File::create(destination).map_err(|error| {
        format!(
            "Unable to create theme package '{}': {error}",
            destination.display()
        )
    })?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    archive
        .start_file(THEME_ENTRY, options)
        .map_err(|error| error.to_string())?;
    archive
        .write_all(&theme_bytes)
        .map_err(|error| error.to_string())?;

    archive
        .start_file(CONTEXT_MENU_ENTRY, options)
        .map_err(|error| error.to_string())?;
    archive
        .write_all(&context_menu_bytes)
        .map_err(|error| error.to_string())?;

    for (relative_path, bytes) in assets {
        archive
            .start_file(relative_path, options)
            .map_err(|error| error.to_string())?;
        archive
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
    }

    let mut file = archive.finish().map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(asset_paths.len())
}

pub fn import_path(source: &Path) -> Result<ImportedTheme, String> {
    if is_theme_package(source) {
        let file = File::open(source)
            .map_err(|error| format!("Unable to open '{}': {error}", source.display()))?;
        import_decoded(decode_package(file)?)
    } else if source
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        let bytes = std::fs::read(source)
            .map_err(|error| format!("Unable to open '{}': {error}", source.display()))?;
        let theme = decode_theme(&bytes)?;
        let (theme, theme_path) = persist_theme(theme)?;
        Ok(ImportedTheme {
            theme,
            theme_path,
            context_menu: None,
            imported_assets: 0,
        })
    } else {
        Err("Choose a Theme Studio package (.zip) or theme file (.json)".into())
    }
}

fn import_decoded(mut package: DecodedPackage) -> Result<ImportedTheme, String> {
    let used_assets = theme_asset_paths(&package.theme);
    for relative_path in &used_assets {
        if !package.assets.contains_key(relative_path) {
            return Err(format!(
                "Theme package is missing the used asset '{relative_path}'"
            ));
        }
    }

    // The menu goes first: a rejected one must not leave assets behind.
    let imported_menu = match package.context_menu {
        Some(menu) => {
            let packaged_id = menu.id.clone();
            let (menu, path) = persist_context_menu(menu)?;
            if menu.id != packaged_id {
                rename_menu_references(&mut package.theme, &packaged_id, &menu.id);
            }
            Some((menu, path))
        }
        None => None,
    };

    let mut imported_assets = 0;
    for (relative_path, bytes) in package.assets {
        let file_name = managed_asset_file_name(&relative_path)?;
        let imported = theme_engine::import_asset_bytes(file_name, &bytes)?;
        if imported.relative_path != relative_path {
            for background in package.theme.backgrounds_mut() {
                if let LayerBackground::Image { path, .. } = background {
                    if *path == relative_path {
                        *path = imported.relative_path.clone();
                    }
                }
            }
        }
        imported_assets += 1;
    }

    let (theme, theme_path) = persist_theme(package.theme)?;
    Ok(ImportedTheme {
        theme,
        theme_path,
        context_menu: imported_menu,
        imported_assets,
    })
}

fn decode_package<R: Read + Seek>(reader: R) -> Result<DecodedPackage, String> {
    let mut archive = ZipArchive::new(reader).map_err(|error| error.to_string())?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!(
            "Theme package contains too many entries (maximum {MAX_ENTRIES})"
        ));
    }

    let mut names = HashSet::new();
    let mut total_bytes = 0u64;
    let mut theme = None;
    let mut context_menu = None;
    let mut assets = HashMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.is_dir() {
            continue;
        }
        // zip 9 reports a name that is not valid UTF-8 as an error.
        let entry_name = entry.name().map_err(|error| error.to_string())?;
        let name = safe_entry_name(&entry_name)?;
        if !names.insert(name.clone()) {
            return Err(format!("Theme package contains duplicate entry '{name}'"));
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(format!("Theme package entry '{name}' exceeds 64 MB"));
        }
        total_bytes = total_bytes.saturating_add(entry.size());
        if total_bytes > MAX_PACKAGE_BYTES {
            return Err("Theme package expands beyond the 256 MB limit".into());
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .by_ref()
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_ENTRY_BYTES {
            return Err(format!("Theme package entry '{name}' exceeds 64 MB"));
        }
        match name.as_str() {
            THEME_ENTRY => theme = Some(decode_theme(&bytes)?),
            CONTEXT_MENU_ENTRY => context_menu = Some(decode_context_menu(&bytes)?),
            _ if name.starts_with(ASSET_PREFIX) => {
                managed_asset_file_name(&name)?;
                assets.insert(name, bytes);
            }
            _ => return Err(format!("Unsupported theme package entry '{name}'")),
        }
    }
    let theme = theme.ok_or("Theme package does not contain theme.json")?;
    Ok(DecodedPackage {
        theme,
        context_menu,
        assets,
    })
}

fn decode_theme(bytes: &[u8]) -> Result<ThemeDocument, String> {
    let mut theme: ThemeDocument =
        serde_json::from_slice(bytes).map_err(|error| format!("Invalid theme JSON: {error}"))?;
    theme.prepare_runtime();
    theme.check()?;
    Ok(theme)
}

fn decode_context_menu(bytes: &[u8]) -> Result<ContextMenuDocument, String> {
    let menu: ContextMenuDocument = serde_json::from_slice(bytes)
        .map_err(|error| format!("Invalid context menu JSON: {error}"))?;
    menu.check()?;
    Ok(menu)
}

fn persist_theme(mut theme: ThemeDocument) -> Result<(ThemeDocument, PathBuf), String> {
    if theme.is_builtin() {
        let path = theme_engine::ensure_starter_theme()?;
        let existing = theme_engine::load_theme(&path)?;
        if !themes_match(&existing, &theme) {
            return Err("An imported package cannot replace a built-in theme".into());
        }
        return Ok((existing, path));
    }
    theme.prepare_runtime();
    let path = theme_engine::theme_file_path(&theme.id);
    if path.exists() {
        if theme_engine::load_theme(&path).is_ok_and(|existing| themes_match(&existing, &theme)) {
            return Ok((theme, path));
        }
        // Keep the library's theme; the import becomes a numbered copy.
        (theme.id, theme.name) = numbered_copy(&theme.id, &theme.name, |id| {
            theme_engine::theme_file_path(id).exists()
        });
    }
    let path = theme_engine::save_theme(&theme)?;
    Ok((theme, path))
}

fn themes_match(left: &ThemeDocument, right: &ThemeDocument) -> bool {
    serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
}

/// `<id>-<n>` and `<name> (<n>)` for the first `n` from 2 whose id is free.
fn numbered_copy(id: &str, name: &str, taken: impl Fn(&str) -> bool) -> (String, String) {
    let mut number = 2;
    while taken(&format!("{id}-{number}")) {
        number += 1;
    }
    (format!("{id}-{number}"), format!("{name} ({number})"))
}

fn persist_context_menu(
    mut menu: ContextMenuDocument,
) -> Result<(ContextMenuDocument, PathBuf), String> {
    if menu.is_builtin() {
        context_menu::ensure_builtin_context_menus()?;
        let existing = context_menu::resolve_context_menu(Some(&menu.id))?;
        if existing != menu {
            return Err("An imported package cannot replace a built-in context menu".into());
        }
        let path = context_menu::context_menus_directory().join(format!("{}.json", menu.id));
        return Ok((existing, path));
    }
    let library = context_menu::list_context_menus()?;
    let taken = |id: &str| {
        library
            .iter()
            .any(|existing| existing.id.trim().eq_ignore_ascii_case(id.trim()))
            || context_menu::context_menu_file(id).exists()
    };
    if taken(&menu.id) {
        let path = context_menu::context_menu_file(&menu.id);
        if context_menu::load_context_menu(&path).is_ok_and(|existing| existing == menu) {
            return Ok((menu, path));
        }
        // Keep the library's menu; the import becomes a numbered copy.
        (menu.id, menu.name) = numbered_copy(&menu.id, &menu.name, taken);
    }
    let path = context_menu::save_context_menu(&menu)?;
    Ok((menu, path))
}

/// Points the theme's `show_context_menu("<from>")` actions at a renamed menu.
// shortcut: only the exact form Studio writes is matched, not a menu name or
// other spacing; parse the actions if hand-written themes need it.
fn rename_menu_references(theme: &mut ThemeDocument, from: &str, to: &str) {
    let call = |id: &str| {
        let id = id.replace('\\', "\\\\").replace('"', "\\\"");
        format!("show_context_menu(\"{id}\")")
    };
    let (from, to) = (call(from), call(to));
    let objects = theme.surfaces.iter_mut().flat_map(|surface| {
        std::iter::once(&mut surface.mouse_events).chain(
            surface
                .children
                .iter_mut()
                .map(|object| &mut object.mouse_events),
        )
    });
    for events in objects.flatten() {
        for event in theme_engine::MouseEventKind::ALL {
            let handler = events.handler_mut(event);
            *handler = handler.replace(&from, &to);
        }
    }
}

fn safe_entry_name(name: &str) -> Result<String, String> {
    if name.contains('\\') || name.starts_with('/') || name.contains(':') {
        return Err(format!("Unsafe theme package path '{name}'"));
    }
    let path = Path::new(name);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("Unsafe theme package path '{name}'"));
    }
    Ok(name.to_string())
}

fn theme_asset_paths(theme: &ThemeDocument) -> BTreeSet<String> {
    theme
        .backgrounds()
        .filter_map(|background| match background {
            LayerBackground::Image { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn package_decoder_accepts_theme_menu_and_assets() {
        let theme = ThemeDocument::starter();
        let menu = context_menu::classic_context_menu();
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut bytes);
            let options = SimpleFileOptions::default();
            writer.start_file(THEME_ENTRY, options).unwrap();
            writer
                .write_all(&serde_json::to_vec(&theme).unwrap())
                .unwrap();
            writer.start_file(CONTEXT_MENU_ENTRY, options).unwrap();
            writer
                .write_all(&serde_json::to_vec(&menu).unwrap())
                .unwrap();
            writer.finish().unwrap();
        }
        bytes.set_position(0);
        let decoded = decode_package(bytes).unwrap();
        assert_eq!(decoded.theme.id, theme.id);
        assert_eq!(decoded.context_menu.unwrap().id, menu.id);
    }

    fn package(theme: ThemeDocument, context_menu: Option<ContextMenuDocument>) -> DecodedPackage {
        DecodedPackage {
            theme,
            context_menu,
            assets: HashMap::new(),
        }
    }

    fn user_theme(id: &str, name: &str) -> ThemeDocument {
        let mut theme = ThemeDocument::starter();
        theme.id = id.into();
        theme.name = name.into();
        theme
    }

    #[test]
    fn import_keeps_an_existing_theme_with_the_same_id() {
        let _app_data = crate::app_settings::TestAppData::new();
        let mine = user_theme("shared-theme", "Mine");
        let mine_path = theme_engine::save_theme(&mine).unwrap();

        let imported = import_decoded(package(user_theme("shared-theme", "Theirs"), None)).unwrap();
        assert_eq!(theme_engine::load_theme(&mine_path).unwrap().name, "Mine");
        assert_eq!(imported.theme.id, "shared-theme-2");
        assert_eq!(imported.theme.name, "Theirs (2)");
        assert_eq!(
            theme_engine::load_theme(&imported.theme_path).unwrap().name,
            "Theirs (2)"
        );

        // The same theme again is the one already in the library.
        let again = import_decoded(package(mine, None)).unwrap();
        assert_eq!(again.theme_path, mine_path);
    }

    #[test]
    fn import_keeps_an_existing_menu_and_points_the_theme_at_its_copy() {
        let _app_data = crate::app_settings::TestAppData::new();
        let mut mine = ContextMenuDocument::blank("Work");
        mine.id = "work".into();
        let mine_path = context_menu::save_context_menu(&mine).unwrap();
        let mut theirs = mine.clone();
        theirs.items.remove(0);
        let mut theme = user_theme("menu-theme", "Menu theme");
        theme.surfaces[0].mouse_events = Some(theme_engine::MouseEvents {
            right_click: "show_context_menu(\"work\")".into(),
            ..Default::default()
        });

        let imported = import_decoded(package(theme, Some(theirs))).unwrap();
        let (menu, menu_path) = imported.context_menu.unwrap();
        assert_eq!(context_menu::load_context_menu(&mine_path).unwrap(), mine);
        assert_eq!(
            (menu.id.as_str(), menu.name.as_str()),
            ("work-2", "Work (2)")
        );
        assert_eq!(context_menu::load_context_menu(&menu_path).unwrap(), menu);
        assert_eq!(
            imported.theme.surfaces[0]
                .mouse_events
                .as_ref()
                .unwrap()
                .right_click,
            "show_context_menu(\"work-2\")"
        );
    }

    #[test]
    fn a_rejected_menu_leaves_no_asset_behind() {
        let _app_data = crate::app_settings::TestAppData::new();
        let mut changed = context_menu::classic_context_menu();
        changed.name = "Changed".into();
        let mut image = Cursor::new(Vec::new());
        image::RgbaImage::new(1, 1)
            .write_to(&mut image, image::ImageFormat::Png)
            .unwrap();
        let mut decoded = package(user_theme("asset-theme", "Asset theme"), Some(changed));
        decoded
            .assets
            .insert("assets/orphan.png".into(), image.into_inner());

        assert!(import_decoded(decoded).is_err());
        assert!(!theme_engine::assets_directory().join("orphan.png").exists());
    }

    #[test]
    fn package_decoder_rejects_unsafe_paths() {
        assert!(safe_entry_name("../theme.json").is_err());
        assert!(safe_entry_name("assets\\logo.png").is_err());
        assert!(safe_entry_name("C:/theme.json").is_err());
        assert_eq!(
            safe_entry_name("assets/logo.png").unwrap(),
            "assets/logo.png"
        );
    }
}
