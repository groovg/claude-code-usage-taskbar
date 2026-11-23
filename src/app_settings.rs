//! Shared, atomically persisted state used by the widget and studio processes.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

use crate::models::AppUsageData;
use crate::providers::{ProviderId, ProviderSet};

pub const POLL_1_MIN_SECONDS: u32 = 60;
pub const POLL_5_MIN_SECONDS: u32 = 300;
pub const POLL_15_MIN_SECONDS: u32 = 900;
pub const POLL_1_HOUR_SECONDS: u32 = 3_600;
pub const POLL_1_MIN: u32 = POLL_1_MIN_SECONDS * 1_000;
pub const POLL_5_MIN: u32 = POLL_5_MIN_SECONDS * 1_000;
pub const POLL_15_MIN: u32 = POLL_15_MIN_SECONDS * 1_000;
pub const POLL_1_HOUR: u32 = POLL_1_HOUR_SECONDS * 1_000;
// SetTimer clamps longer intervals to USER_TIMER_MAXIMUM (i32::MAX ms).
pub const MAX_POLL_MINUTES: u32 = i32::MAX as u32 / POLL_1_MIN;

/// Which end of the taskbar the built-in widgets dock to. Custom themes keep
/// the placement set in Theme Studio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetPosition {
    /// Follow the taskbar: the free left end when Windows centres its buttons
    /// (Windows 11 default), beside the tray when the buttons start at the
    /// left edge (Windows 10, or Windows 11 set to "Left"), where a widget
    /// would otherwise sit on the Start button.
    #[default]
    Auto,
    /// Left edge of the taskbar.
    Left,
    /// Beside the notification area, where the widget started out.
    Right,
}

impl WidgetPosition {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Left, Self::Right];

    /// English catalogue key resolved through the localization layer.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic",
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }

    /// The end of the taskbar to use right now: never `Auto`.
    pub fn resolved(self) -> Self {
        self.for_alignment(crate::theme::taskbar_buttons_centered())
    }

    pub fn for_alignment(self, buttons_centered: bool) -> Self {
        match self {
            Self::Auto if buttons_centered => Self::Left,
            Self::Auto => Self::Right,
            explicit => explicit,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingsFile {
    #[serde(default)]
    pub accounts: crate::accounts::AccountSettings,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update_check_unix: Option<u64>,
    #[serde(default = "default_true")]
    show_claude_code: bool,
    #[serde(default)]
    show_codex: bool,
    #[serde(default)]
    show_antigravity: bool,
    #[serde(default)]
    show_opencode: bool,
    #[serde(default)]
    show_cursor: bool,
    /// Show what is left of each allowance instead of what has been spent, so
    /// the widget counts down towards a limit rather than up from zero.
    #[serde(default)]
    pub usage_countdown: bool,
    #[serde(default)]
    pub widget_position: WidgetPosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_theme_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard_height: Option<f32>,
}

impl Default for SettingsFile {
    fn default() -> Self {
        Self {
            accounts: Default::default(),
            poll_interval_ms: default_poll_interval(),
            language: None,
            last_update_check_unix: None,
            show_claude_code: true,
            show_codex: false,
            show_antigravity: false,
            show_opencode: false,
            show_cursor: false,
            usage_countdown: false,
            widget_position: WidgetPosition::Auto,
            active_theme_path: None,
            dashboard_width: None,
            dashboard_height: None,
        }
    }
}

impl SettingsFile {
    pub fn normalize(&mut self) {
        self.accounts.claude.normalize();
        self.accounts.codex.normalize();
        if !(POLL_1_MIN..=MAX_POLL_MINUTES * POLL_1_MIN).contains(&self.poll_interval_ms)
            || !self.poll_interval_ms.is_multiple_of(POLL_1_MIN)
        {
            self.poll_interval_ms = default_poll_interval();
        }
        if self.enabled_providers().is_empty() {
            self.set_enabled_providers(ProviderSet::default());
        }
        self.dashboard_width = valid_dashboard_dimension(self.dashboard_width);
        self.dashboard_height = valid_dashboard_dimension(self.dashboard_height);
    }

    pub fn enabled_providers(&self) -> ProviderSet {
        ProviderSet::from_enabled(
            ProviderId::ALL
                .into_iter()
                .filter(|provider| self.provider_enabled(*provider)),
        )
    }

    pub fn provider_enabled(&self, provider: ProviderId) -> bool {
        match provider {
            ProviderId::Claude => self.show_claude_code,
            ProviderId::Codex => self.show_codex,
            ProviderId::Antigravity => self.show_antigravity,
            ProviderId::OpenCode => self.show_opencode,
            ProviderId::Cursor => self.show_cursor,
        }
    }

    pub fn set_provider_enabled(&mut self, provider: ProviderId, enabled: bool) {
        match provider {
            ProviderId::Claude => self.show_claude_code = enabled,
            ProviderId::Codex => self.show_codex = enabled,
            ProviderId::Antigravity => self.show_antigravity = enabled,
            ProviderId::OpenCode => self.show_opencode = enabled,
            ProviderId::Cursor => self.show_cursor = enabled,
        }
    }

    pub fn set_enabled_providers(&mut self, providers: ProviderSet) {
        for provider in ProviderId::ALL {
            self.set_provider_enabled(provider, providers.contains(provider));
        }
    }

    pub fn toggle_provider(&mut self, provider: ProviderId) -> bool {
        let mut providers = self.enabled_providers();
        if !providers.toggle(provider) {
            return false;
        }
        self.set_enabled_providers(providers);
        true
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UsageCache {
    pub updated_unix: u64,
    pub poll_ok: bool,
    pub data: AppUsageData,
}

#[cfg(test)]
thread_local! {
    static TEST_APP_DATA: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Points this test thread's app data at a fresh temp folder until dropped.
#[cfg(test)]
pub(crate) struct TestAppData(PathBuf);

#[cfg(test)]
impl TestAppData {
    pub(crate) fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "claude-code-usage-taskbar-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&directory);
        TEST_APP_DATA.with(|data| *data.borrow_mut() = Some(directory.clone()));
        Self(directory)
    }
}

#[cfg(test)]
impl Drop for TestAppData {
    fn drop(&mut self) {
        TEST_APP_DATA.with(|data| *data.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn app_data_directory() -> PathBuf {
    #[cfg(test)]
    if let Some(directory) = TEST_APP_DATA.with(|data| data.borrow().clone()) {
        return directory;
    }
    let root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("ClaudeCodeUsageTaskbar")
}

pub fn settings_path() -> PathBuf {
    app_data_directory().join("settings.json")
}
pub fn usage_cache_path() -> PathBuf {
    app_data_directory().join("usage-cache.json")
}

pub fn load_settings() -> SettingsFile {
    read_settings().unwrap_or_else(|_| {
        let mut settings = SettingsFile::default();
        settings.normalize();
        settings
    })
}

/// The settings file, or the defaults when it is missing or is not JSON at
/// all (a copy of such a file is kept as `settings.json.bad`). `Err` only when
/// the file exists but can't be read.
fn read_settings() -> std::io::Result<SettingsFile> {
    let path = settings_path();
    let mut settings = match std::fs::read(&path) {
        Ok(bytes) => std::str::from_utf8(&bytes)
            .ok()
            .and_then(decode_settings)
            .unwrap_or_else(|| {
                let backup = path.with_extension("json.bad");
                if std::fs::read(&backup).ok().as_ref() != Some(&bytes) {
                    let _ = std::fs::write(&backup, &bytes);
                }
                SettingsFile::default()
            }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => SettingsFile::default(),
        Err(error) => return Err(error),
    };
    settings.normalize();
    Ok(settings)
}

pub fn save_settings(settings: &SettingsFile) -> Result<(), String> {
    let mut normalized = settings.clone();
    normalized.normalize();
    write_json_atomic(&settings_path(), &settings_json(&normalized))
}

/// Saves only the fields `edited` changed from `base`; every other field keeps
/// the file's current value, so changes another process made since `base` was
/// read survive. Returns the settings as saved.
pub fn save_settings_changes(
    base: &SettingsFile,
    edited: &SettingsFile,
) -> Result<SettingsFile, String> {
    let current =
        read_settings().map_err(|error| format!("Unable to read the settings file: {error}"))?;
    let mut merged = merge_settings(base, edited, &current);
    merged.normalize();
    save_settings(&merged)?;
    Ok(merged)
}

fn merge_settings(
    base: &SettingsFile,
    edited: &SettingsFile,
    current: &SettingsFile,
) -> SettingsFile {
    // No language and "system" are the same choice; Studio's picker turns one
    // into the other just by showing it.
    let compared = |settings: &SettingsFile| {
        let mut settings = settings.clone();
        settings.language.take_if(|code| code == "system");
        settings_fields(&settings)
    };
    let (base, changed, edited) = (compared(base), compared(edited), settings_fields(edited));
    let mut merged = settings_fields(current);
    for key in base.keys().chain(changed.keys()) {
        if base.get(key) != changed.get(key) {
            match edited.get(key) {
                Some(value) => merged.insert(key.clone(), value.clone()),
                None => merged.remove(key),
            };
        }
    }
    serde_json::from_value(merged.into()).unwrap_or_else(|_| current.clone())
}

fn decode_settings(content: &str) -> Option<SettingsFile> {
    // Through a Value, as before: duplicate keys keep the last one.
    let serde_json::Value::Object(fields) = serde_json::from_str(content).ok()? else {
        return None;
    };
    // A field with a bad value falls back to its default alone.
    let mut valid = serde_json::Map::new();
    for (key, value) in fields {
        valid.insert(key.clone(), value);
        if serde_json::from_value::<SettingsFile>(valid.clone().into()).is_err() {
            valid.remove(&key);
        }
    }
    serde_json::from_value(valid.into()).ok()
}

fn settings_json(settings: &SettingsFile) -> serde_json::Value {
    serde_json::to_value(settings).unwrap_or_default()
}

fn settings_fields(settings: &SettingsFile) -> serde_json::Map<String, serde_json::Value> {
    match settings_json(settings) {
        serde_json::Value::Object(fields) => fields,
        _ => serde_json::Map::new(),
    }
}

pub fn load_usage_cache() -> Option<UsageCache> {
    let mut cache: UsageCache = read_json(&usage_cache_path())?;
    cache.data.invalidate_changed_credentials();
    Some(cache)
}

pub fn save_usage_cache(data: &AppUsageData, poll_ok: bool) -> Result<(), String> {
    write_json_atomic(
        &usage_cache_path(),
        &UsageCache {
            updated_unix: now_unix(),
            poll_ok,
            data: data.clone(),
        },
    )
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid settings path")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.json");
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let json = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(&json).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    let source = wide_path(&temporary);
    let destination = wide_path(path);
    let moved = unsafe {
        MoveFileExW(
            PCWSTR::from_raw(source.as_ptr()),
            PCWSTR::from_raw(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err("Unable to replace the settings file".into());
    }
    Ok(())
}

fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn default_poll_interval() -> u32 {
    POLL_15_MIN
}
fn default_true() -> bool {
    true
}
fn valid_dashboard_dimension(value: Option<f32>) -> Option<f32> {
    value.filter(|value| value.is_finite() && (64.0..=16_384.0).contains(value))
}
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_poll_minutes_and_presets_survive_settings_round_trip() {
        for minutes in [1, 2, 5, 7, 15, 60, 120, 1_440, MAX_POLL_MINUTES] {
            let interval = minutes * POLL_1_MIN;
            let mut decoded =
                decode_settings(&format!(r#"{{"poll_interval_ms":{interval}}}"#)).unwrap();
            decoded.normalize();
            assert_eq!(decoded.poll_interval_ms, interval);
            let mut reloaded = decode_settings(&settings_json(&decoded).to_string()).unwrap();
            reloaded.normalize();
            assert_eq!(reloaded.poll_interval_ms, interval);
        }
    }

    #[test]
    fn invalid_poll_intervals_fall_back_to_the_default() {
        for interval in [
            0,
            POLL_1_MIN - 1,
            POLL_1_MIN + 1,
            (MAX_POLL_MINUTES + 1) * POLL_1_MIN,
            u32::MAX,
        ] {
            let mut decoded =
                decode_settings(&format!(r#"{{"poll_interval_ms":{interval}}}"#)).unwrap();
            decoded.normalize();
            assert_eq!(decoded.poll_interval_ms, default_poll_interval());
        }
    }

    #[test]
    fn named_accounts_round_trip_without_changing_legacy_provider_preferences() {
        let old = decode_settings(r#"{"show_claude_code":true,"show_codex":true}"#).unwrap();
        assert_eq!(old.accounts, crate::accounts::AccountSettings::default());
        let mut settings = old;
        settings.accounts.codex.add();
        settings.accounts.codex.profiles[1].config_dir = "C:\\Users\\Test\\.codex-work".into();
        settings.accounts.codex.profiles[1].enabled = true;
        settings.accounts.codex.selected = "account_1".into();
        let decoded = decode_settings(&settings_json(&settings).to_string()).unwrap();
        assert_eq!(decoded.accounts, settings.accounts);
        assert_eq!(decoded.enabled_providers(), settings.enabled_providers());
    }

    #[test]
    fn settings_never_disable_every_provider() {
        let mut settings = SettingsFile {
            show_claude_code: false,
            show_codex: false,
            show_antigravity: false,
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.enabled_providers(), ProviderSet::default());
    }

    #[test]
    fn provider_selection_keeps_the_existing_settings_keys() {
        let mut settings = SettingsFile::default();
        settings.set_enabled_providers(ProviderSet::from_enabled([
            ProviderId::Codex,
            ProviderId::Antigravity,
            ProviderId::OpenCode,
            ProviderId::Cursor,
        ]));

        let json = settings_json(&settings);
        assert_eq!(json["show_claude_code"], false);
        assert_eq!(json["show_codex"], true);
        assert_eq!(json["show_antigravity"], true);
        assert_eq!(json["show_opencode"], true);
        assert_eq!(json["show_cursor"], true);

        let decoded = decode_settings(&json.to_string()).unwrap();
        assert_eq!(decoded.enabled_providers(), settings.enabled_providers());
    }

    #[test]
    fn provider_toggle_keeps_the_last_provider_enabled() {
        let mut settings = SettingsFile::default();
        assert!(!settings.toggle_provider(ProviderId::Claude));
        assert_eq!(settings.enabled_providers(), ProviderSet::default());
    }

    #[test]
    fn usage_direction_defaults_to_counting_up_and_round_trips() {
        let settings = SettingsFile::default();
        assert!(!settings.usage_countdown);
        assert_eq!(settings_json(&settings)["usage_countdown"], false);

        let counting_up = decode_settings(r#"{"poll_interval_ms":900000}"#).unwrap();
        assert!(!counting_up.usage_countdown);

        let counting_down = decode_settings(r#"{"usage_countdown":true}"#).unwrap();
        assert!(counting_down.usage_countdown);
        assert_eq!(settings_json(&counting_down)["usage_countdown"], true);

        // Older files carry no position and follow the taskbar; the choice
        // round-trips.
        assert_eq!(counting_down.widget_position, WidgetPosition::Auto);
        assert_eq!(settings_json(&counting_down)["widget_position"], "auto");
        let docked_right = decode_settings(r#"{"widget_position":"right"}"#).unwrap();
        assert_eq!(docked_right.widget_position, WidgetPosition::Right);
        assert_eq!(settings_json(&docked_right)["widget_position"], "right");
    }

    #[test]
    fn automatic_position_follows_the_taskbar_alignment() {
        // Centred buttons leave the left end free; left-aligned buttons put
        // the Start button there, so the widget moves beside the tray.
        assert_eq!(
            WidgetPosition::Auto.for_alignment(true),
            WidgetPosition::Left
        );
        assert_eq!(
            WidgetPosition::Auto.for_alignment(false),
            WidgetPosition::Right
        );
        for explicit in [WidgetPosition::Left, WidgetPosition::Right] {
            assert_eq!(explicit.for_alignment(true), explicit);
            assert_eq!(explicit.for_alignment(false), explicit);
        }
    }

    #[test]
    fn dashboard_dimensions_are_preserved_and_validated() {
        let settings = decode_settings(
            r#"{
                "dashboard_width": 1280.5,
                "dashboard_height": 760.0
            }"#,
        )
        .unwrap();
        assert_eq!(settings.dashboard_width, Some(1280.5));
        assert_eq!(settings.dashboard_height, Some(760.0));

        let mut invalid = SettingsFile {
            dashboard_width: Some(0.0),
            dashboard_height: Some(20_000.0),
            ..Default::default()
        };
        invalid.normalize();
        assert_eq!(invalid.dashboard_width, None);
        assert_eq!(invalid.dashboard_height, None);
    }

    #[test]
    fn one_bad_field_keeps_the_other_settings() {
        let settings = decode_settings(
            r#"{"show_codex":true,"usage_countdown":"yes","poll_interval_ms":300000}"#,
        )
        .unwrap();
        assert!(settings.show_codex);
        assert!(!settings.usage_countdown);
        assert_eq!(settings.poll_interval_ms, POLL_5_MIN);
    }

    #[test]
    fn a_file_that_does_not_parse_is_kept_as_bad() {
        let _app_data = TestAppData::new();
        let broken = r#"{"show_codex":true,}"#;
        std::fs::create_dir_all(app_data_directory()).unwrap();
        std::fs::write(settings_path(), broken).unwrap();

        assert!(!load_settings().show_codex);
        save_settings(&SettingsFile::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(settings_path().with_extension("json.bad")).unwrap(),
            broken
        );
    }

    #[test]
    fn saving_changes_keeps_fields_changed_by_another_process() {
        let _app_data = TestAppData::new();
        let loaded = load_settings();
        // The tray turns Codex on and picks a language after Studio loaded.
        let mut tray = loaded.clone();
        tray.set_provider_enabled(ProviderId::Codex, true);
        tray.language = Some("de".into());
        tray.last_update_check_unix = Some(42);
        save_settings(&tray).unwrap();

        // Studio changes only the countdown; showing its language picker
        // turned "no language" into "system".
        let mut studio = loaded.clone();
        studio.usage_countdown = true;
        studio.language = Some("system".into());
        let saved = save_settings_changes(&loaded, &studio).unwrap();
        let on_disk = load_settings();
        assert_eq!(settings_json(&saved), settings_json(&on_disk));
        assert!(on_disk.usage_countdown);
        assert!(on_disk.provider_enabled(ProviderId::Codex));
        assert_eq!(on_disk.language.as_deref(), Some("de"));
        assert_eq!(on_disk.last_update_check_unix, Some(42));

        // A language Studio did pick wins.
        studio.language = Some("fr".into());
        save_settings_changes(&loaded, &studio).unwrap();
        assert_eq!(load_settings().language.as_deref(), Some("fr"));
    }
}
