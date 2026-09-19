use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use crate::diagnose;
use crate::models::{AppUsageData, UsageData, UsageSection};
use crate::providers::{ProviderId, ProviderSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PollError {
    AuthRequired,
    NoCredentials,
    TokenExpired,
    RequestFailed,
}

impl PollError {
    pub fn is_auth(self) -> bool {
        matches!(self, Self::AuthRequired | Self::TokenExpired)
    }

    /// One line for a tooltip: what went wrong and what fixes it.
    pub fn description(self) -> &'static str {
        match self {
            Self::AuthRequired => "the service rejected the login; sign in again",
            Self::NoCredentials => "no login found; sign in with the CLI or desktop app",
            Self::TokenExpired => {
                "the login expired and could not be renewed; run the CLI once to refresh it"
            }
            Self::RequestFailed => "the usage service could not be reached; retrying",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialWatchMode {
    ActiveSource(ProviderId),
    AllSources(ProviderId),
}

pub type CredentialWatchSnapshot = Vec<String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PollFailure {
    pub provider: ProviderId,
    pub error: PollError,
}

/// Claude and Codex fan out per account; every other provider is polled once.
pub(crate) use accounts::poll_accounts as poll;

/// Keep the previous reading for any enabled provider that failed this cycle.
///
/// A poll succeeds as long as one provider answers, so without this a single
/// provider's outage blanks its row on every refresh while the others carry
/// on updating. The carried figures are marked stale rather than passed off as
/// current.
pub fn carry_forward_failures(
    fresh: AppUsageData,
    previous: &AppUsageData,
    enabled: ProviderSet,
) -> AppUsageData {
    let mut merged = fresh;
    accounts::carry_accounts(&mut merged, previous);
    for provider in enabled.iter() {
        if merged
            .accounts
            .iter()
            .any(|account| account.provider == provider)
            || previous
                .accounts
                .iter()
                .any(|account| account.provider == provider)
        {
            continue;
        }
        if merged.get(provider).is_some() {
            continue;
        }
        if let Some(last) = previous.get(provider) {
            let mut carried = last.clone();
            carried.stale = true;
            merged.insert(provider, carried);
        }
    }
    merged
}

const MAX_CONCURRENT_PROVIDER_POLLS: usize = 3;

mod accounts;
mod antigravity;
mod claude;
mod claude_context;
mod claude_desktop;
mod codex;
mod cursor;
mod opencode;

/// Context-window usage of the newest local Claude Code session. Cheap enough
/// to call between API polls: a directory listing and one file tail.
pub(crate) use claude_context::read as claude_session_context;

/// The provider's default credentials, found the way its own CLI finds them.
fn poll_provider(provider: ProviderId) -> Result<UsageData, PollError> {
    match provider {
        ProviderId::Claude => claude::poll_claude_code(),
        ProviderId::Codex => codex::poll_codex(),
        ProviderId::Antigravity => antigravity::poll_antigravity(),
        ProviderId::OpenCode => opencode::poll_opencode(),
        ProviderId::Cursor => cursor::poll_cursor(),
    }
}

pub fn credential_watch_snapshot(mode: CredentialWatchMode) -> CredentialWatchSnapshot {
    let (provider, all_sources) = match mode {
        CredentialWatchMode::ActiveSource(provider) => (provider, false),
        CredentialWatchMode::AllSources(provider) => (provider, true),
    };
    match provider {
        ProviderId::Claude => claude::credential_watch_snapshot(all_sources),
        ProviderId::Codex => codex::credential_watch_snapshot(),
        ProviderId::Antigravity => vec![antigravity::credential_watch_signature()],
        ProviderId::OpenCode => vec![opencode::credential_watch_signature()],
        ProviderId::Cursor => cursor::credential_watch_snapshot(),
    }
}

/// The app's one HTTP agent, shared by the pollers and the updater.
pub(crate) static HTTP_AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .tls_config(tls)
        .build()
        .into()
});

type HttpResponse = ureq::http::Response<ureq::Body>;

fn get_header_f64(response: &HttpResponse, name: &str) -> f64 {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn get_header_i64(response: &HttpResponse, name: &str) -> Option<i64> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
}

fn non_empty_environment(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Unpadded base64; `url_safe` selects `-_` instead of `+/` for 62 and 63.
fn base64_decode(input: &str, url_safe: bool) -> Option<Vec<u8>> {
    let (value_62, value_63) = if url_safe { (b'-', b'_') } else { (b'+', b'/') };
    if input.len() % 4 == 1 {
        return None;
    }
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            _ if byte == value_62 => 62,
            _ if byte == value_63 => 63,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    let padding_mask = (1u32 << bits).saturating_sub(1);
    (buffer & padding_mask == 0).then_some(output)
}

/// The first of `names` that starts (`--version`), else the first path
/// `where.exe` reports for any of them.
fn resolve_cli(names: &[&str]) -> Option<String> {
    if let Some(name) = names.iter().find(|name| {
        Command::new(name)
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }) {
        return Some(name.to_string());
    }
    names.iter().find_map(|name| {
        let output = Command::new("where.exe")
            .arg(name)
            .creation_flags(CREATE_NO_WINDOW.0)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string)
    })
}

/// A command for a resolved CLI path; `.cmd` shims only run through cmd.exe.
fn cli_command(path: &str) -> Command {
    if path.to_lowercase().ends_with(".cmd") {
        let mut command = Command::new("cmd.exe");
        command.arg("/c").arg(path);
        command
    } else {
        Command::new(path)
    }
}

/// Run a CLI token refresh hidden and silent, killing it after 30 seconds.
fn run_refresh(mut command: Command, what: &str) {
    command
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            diagnose::log_error(&format!("unable to spawn {what}"), error);
            return;
        }
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) if start.elapsed() > Duration::from_secs(30) => {
                let _ = child.kill();
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

fn unix_to_system_time(unix_secs: Option<i64>) -> Option<SystemTime> {
    let secs = unix_secs?;
    if secs < 0 {
        return None;
    }
    Some(UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// Parse an ISO 8601 timestamp string into a SystemTime.
fn parse_iso8601(s: Option<&str>) -> Option<SystemTime> {
    let unix_secs = parse_datetime_to_unix(s?)?;
    UNIX_EPOCH.checked_add(Duration::from_secs(unix_secs))
}

/// Minimal datetime parser — avoids pulling in chrono/time crates.
fn parse_datetime_to_unix(s: &str) -> Option<u64> {
    let (datetime, offset_seconds) = split_timezone(s)?;
    let datetime = match datetime.split_once('.') {
        Some((base, fraction))
            if !fraction.is_empty() && fraction.bytes().all(|b| b.is_ascii_digit()) =>
        {
            base
        }
        Some(_) => return None,
        None => datetime,
    };
    let bytes = datetime.as_bytes();
    if bytes.len() != 19
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }

    let year = parse_digits(&bytes[0..4])?;
    let month = parse_digits(&bytes[5..7])?;
    let day = parse_digits(&bytes[8..10])?;
    let hour = parse_digits(&bytes[11..13])?;
    let minute = parse_digits(&bytes[14..16])?;
    let second = parse_digits(&bytes[17..19])?;
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    let days = (1970..year)
        .map(|year| if is_leap(year) { 366 } else { 365 })
        .sum::<u64>()
        + (1..month)
            .map(|month| days_in_month(year, month))
            .sum::<u64>()
        + day
        - 1;

    let local_seconds = days
        .checked_mul(86_400)?
        .checked_add(hour * 3_600 + minute * 60 + second)?;
    u64::try_from(
        i64::try_from(local_seconds)
            .ok()?
            .checked_sub(offset_seconds)?,
    )
    .ok()
}

fn split_timezone(s: &str) -> Option<(&str, i64)> {
    if let Some(datetime) = s.strip_suffix('Z') {
        return Some((datetime, 0));
    }

    if s.len() >= 25 {
        let offset_start = s.len() - 6;
        let offset = &s.as_bytes()[offset_start..];
        if matches!(offset[0], b'+' | b'-') && offset[3] == b':' {
            let hours = parse_digits(&offset[1..3])?;
            let minutes = parse_digits(&offset[4..6])?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            let seconds = i64::try_from(hours * 3_600 + minutes * 60).ok()?;
            return Some((
                &s[..offset_start],
                if offset[0] == b'+' { seconds } else { -seconds },
            ));
        }
    }

    Some((s, 0))
}

fn parse_digits(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    bytes.iter().try_fold(0_u64, |value, byte| {
        value.checked_mul(10)?.checked_add(u64::from(byte - b'0'))
    })
}

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    }
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Calculate how long until the display text would change
pub fn time_until_display_change(resets_at: Option<SystemTime>) -> Option<Duration> {
    let reset = resets_at?;
    let remaining = reset.duration_since(SystemTime::now()).ok()?;
    Some(time_until_display_change_from_secs(remaining.as_secs()))
}

/// The text counts whole days, hours or minutes: it changes one second after
/// the next boundary of the largest unit that fits.
fn time_until_display_change_from_secs(total_secs: u64) -> Duration {
    let unit = [86_400, 3_600, 60]
        .into_iter()
        .find(|unit| total_secs >= *unit)
        .unwrap_or(1);
    Duration::from_secs(total_secs % unit + 1)
}

/// Returns true if either section has reached "now" (reset time has passed).
pub fn is_past_reset(data: &UsageData) -> bool {
    if data.stale {
        return false;
    }
    let now = SystemTime::now();
    let past = |s: &UsageSection| matches!(s.resets_at, Some(t) if now.duration_since(t).is_ok());
    past(&data.session)
        || past(&data.weekly)
        || data
            .scoped
            .iter()
            .any(|limit| matches!(limit.resets_at, Some(t) if now.duration_since(t).is_ok()))
}

pub fn app_is_past_reset(data: &AppUsageData) -> bool {
    data.all_usage().any(is_past_reset)
}

#[cfg(test)]
mod tests;
