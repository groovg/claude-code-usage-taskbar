use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static LOG: OnceLock<Mutex<File>> = OnceLock::new();

/// Open the diagnostic log; a child process appends to its parent's.
pub fn init(append: bool) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join("claude-code-usage-taskbar.log");
    let mut options = OpenOptions::new();
    options.create(true);
    if append {
        options.append(true);
    } else {
        options.write(true).truncate(true);
    }
    let file = options
        .open(&path)
        .map_err(|e| format!("Unable to open diagnostic log file {}: {e}", path.display()))?;

    let _ = LOG.set(Mutex::new(file));

    log(if append {
        "diagnostic logging enabled for child process"
    } else {
        "diagnostic logging enabled"
    });
    Ok(path)
}

pub fn is_enabled() -> bool {
    LOG.get().is_some()
}

pub fn log(message: impl AsRef<str>) {
    let Some(file) = LOG.get() else {
        return;
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    if let Ok(mut file) = file.lock() {
        let _ = writeln!(file, "[{timestamp}] {}", message.as_ref());
        let _ = file.flush();
    }
}

pub fn log_error(context: &str, error: impl std::fmt::Display) {
    log(format!("{context}: {error}"));
}
