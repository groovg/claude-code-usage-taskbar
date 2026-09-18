//! Context-window usage of the newest local Claude Code session.
//!
//! Claude Code appends every session to `<config dir>/projects/<cwd slug>/<session id>.jsonl`.
//! Each assistant line carries the API `usage` of that turn; the input and
//! cache tokens of the last one are the context the next turn starts from.
//! This reads a file tail and nothing else: no network, no CLI.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::models::ContextSection;

/// Enough to hold the last assistant line even after a large tool result.
const TAIL_BYTES: u64 = 256 * 1024;
const DEFAULT_WINDOW: u64 = 200_000;
const LONG_WINDOW: u64 = 1_000_000;

/// The last parse, keyed by transcript path and mtime: the window asks every
/// five seconds, the file changes only when a turn completes.
static LAST: Mutex<Option<(PathBuf, SystemTime, ContextSection)>> = Mutex::new(None);

pub(super) fn read() -> Option<ContextSection> {
    let config = config_directory()?;
    let (transcript, updated_at) = newest_transcript(&config.join("projects"))?;
    if let Ok(last) = LAST.lock() {
        if let Some((path, modified, section)) = last.as_ref() {
            if *path == transcript && *modified == updated_at {
                return Some(section.clone());
            }
        }
    }
    let tail = read_tail(&transcript, TAIL_BYTES)?;
    let (mut section, cwd) = context_from_transcript_tail(&tail)?;
    // ponytail: the window is cached with the reading; a settings edit shows
    // up after the next turn rather than immediately.
    section.window = context_window(&config, cwd.as_deref());
    section.percentage = (section.tokens as f64 / section.window as f64 * 100.0).clamp(0.0, 100.0);
    section.updated_at = Some(updated_at);
    if let Ok(mut last) = LAST.lock() {
        *last = Some((transcript, updated_at, section.clone()));
    }
    Some(section)
}

fn config_directory() -> Option<PathBuf> {
    crate::accounts::environment_directory(crate::providers::ProviderId::Claude)
        .or_else(|| dirs::home_dir().map(|home| home.join(".claude")))
}

/// The most recently written `*.jsonl` directly under any project directory.
/// Sub-agent transcripts live in nested directories and are not walked, so a
/// tool call's side conversation never masquerades as the main session.
fn newest_transcript(projects: &Path) -> Option<(PathBuf, SystemTime)> {
    std::fs::read_dir(projects)
        .ok()?
        .flatten()
        .filter(|project| project.path().is_dir())
        .flat_map(|project| std::fs::read_dir(project.path()).ok().into_iter().flatten())
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let is_transcript = path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with("agent-"));
            let modified = entry.metadata().ok()?.modified().ok()?;
            is_transcript.then_some((path, modified))
        })
        .max_by_key(|(_, modified)| *modified)
}

fn read_tail(path: &Path, bytes: u64) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(bytes)))
        .ok()?;
    let mut buffer = Vec::with_capacity(length.min(bytes) as usize);
    file.read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// The last complete assistant turn, searched from the end, with the working
/// directory it ran in. Skipped: a line still being written (fails to parse),
/// a line that merely quotes the marker inside a tool result, and the
/// synthetic zero-usage entries Claude Code writes on API errors and
/// interrupts, which say nothing about the context.
fn context_from_transcript_tail(tail: &str) -> Option<(ContextSection, Option<PathBuf>)> {
    tail.rsplit('\n')
        .filter(|line| line.contains("\"type\":\"assistant\""))
        .find_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            if value.get("type")?.as_str()? != "assistant" {
                return None;
            }
            let message = value.get("message")?;
            let usage = message.get("usage")?;
            let count = |key: &str| usage.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
            let tokens = count("input_tokens")
                + count("cache_creation_input_tokens")
                + count("cache_read_input_tokens");
            let model = message
                .get("model")
                .and_then(|model| model.as_str())
                .map(str::to_string);
            if tokens == 0 || model.as_deref() == Some("<synthetic>") {
                return None;
            }
            let cwd = value
                .get("cwd")
                .and_then(|cwd| cwd.as_str())
                .filter(|cwd| !cwd.is_empty());
            let project = cwd
                .and_then(|cwd| cwd.rsplit(['\\', '/']).find(|part| !part.is_empty()))
                .map(str::to_string);
            Some((
                ContextSection {
                    tokens,
                    model,
                    project,
                    ..Default::default()
                },
                cwd.map(PathBuf::from),
            ))
        })
}

/// Claude Code marks the long-context variant with a `[1m]` suffix on the
/// `model` setting. The session's project settings win over the user's, in
/// Claude Code's own order: `.claude/settings.local.json`, then
/// `.claude/settings.json`, then the user settings in the config directory.
fn context_window(config: &Path, cwd: Option<&Path>) -> u64 {
    let project = cwd.map(|cwd| cwd.join(".claude"));
    let candidates = project
        .iter()
        .flat_map(|dir| [dir.join("settings.local.json"), dir.join("settings.json")])
        .chain(std::iter::once(config.join("settings.json")));
    let model = candidates
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .find_map(|settings| model_from_settings(&settings));
    window_for_model(model.as_deref())
}

fn model_from_settings(settings: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(settings)
        .ok()?
        .get("model")?
        .as_str()
        .map(str::to_string)
}

fn window_for_model(model: Option<&str>) -> u64 {
    if model.is_some_and(|model| model.contains("[1m]")) {
        LONG_WINDOW
    } else {
        DEFAULT_WINDOW
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAIL: &str = concat!(
        r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
        "\n",
        r#"{"type":"assistant","cwd":"C:\\Users\\me\\work\\my-app","message":{"model":"claude-fable-5-1","usage":{"input_tokens":32,"cache_creation_input_tokens":2480,"cache_read_input_tokens":56847,"output_tokens":3676}}}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"log says \"type\":\"assistant\" here"}]}}"#,
        "\n",
        r#"{"type":"assistant","message":{"model":"claude-fable-5-1","usage":{"input_tokens":10,"cache_creation"#,
    );

    #[test]
    fn reads_the_last_complete_assistant_turn() {
        let (section, cwd) = context_from_transcript_tail(TAIL).expect("an assistant line");
        assert_eq!(section.tokens, 32 + 2480 + 56847);
        assert_eq!(section.model.as_deref(), Some("claude-fable-5-1"));
        assert_eq!(section.project.as_deref(), Some("my-app"));
        assert_eq!(cwd, Some(PathBuf::from(r"C:\Users\me\work\my-app")));
        assert!(context_from_transcript_tail("{\"type\":\"user\"}\n").is_none());
    }

    #[test]
    fn synthetic_error_turns_do_not_reset_the_context() {
        let tail = format!(
            "{TAIL}\n{}\n",
            r#"{"type":"assistant","message":{"model":"<synthetic>","usage":{"input_tokens":0,"output_tokens":0}}}"#
        );
        let (section, _) = context_from_transcript_tail(&tail).expect("the real turn before it");
        assert_eq!(section.tokens, 32 + 2480 + 56847);
    }

    #[test]
    fn the_long_context_marker_selects_the_million_token_window() {
        assert_eq!(
            window_for_model(
                model_from_settings(r#"{"model": "claude-fable-5-1[1m]"}"#).as_deref()
            ),
            LONG_WINDOW
        );
        assert_eq!(window_for_model(Some("claude-fable-5-1")), DEFAULT_WINDOW);
        // Only the model key counts, not a `[1m]` elsewhere in the file.
        assert_eq!(
            model_from_settings(r#"{"model": "claude-opus-5", "env": {"X": "[1m]"}}"#).as_deref(),
            Some("claude-opus-5")
        );
        assert_eq!(model_from_settings(""), None);
        assert_eq!(window_for_model(None), DEFAULT_WINDOW);
    }

    #[test]
    fn project_settings_outrank_user_settings_for_the_window() {
        let root = std::env::temp_dir().join(format!(
            "claude-window-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = root.join("config");
        let project = root.join("project");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(project.join(".claude")).unwrap();
        std::fs::write(
            config.join("settings.json"),
            r#"{"model": "claude-fable-5-1"}"#,
        )
        .unwrap();
        // No project settings: the user's plain model, standard window.
        assert_eq!(context_window(&config, Some(&project)), DEFAULT_WINDOW);
        // A project file without a model key falls through to the user's.
        std::fs::write(
            project.join(".claude").join("settings.json"),
            r#"{"permissions": {}}"#,
        )
        .unwrap();
        assert_eq!(context_window(&config, Some(&project)), DEFAULT_WINDOW);
        // The local project file wins.
        std::fs::write(
            project.join(".claude").join("settings.local.json"),
            r#"{"model": "claude-fable-5-1[1m]"}"#,
        )
        .unwrap();
        assert_eq!(context_window(&config, Some(&project)), LONG_WINDOW);
        assert_eq!(context_window(&config, None), DEFAULT_WINDOW);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_newest_top_level_transcript_wins_and_agents_are_skipped() {
        let root = std::env::temp_dir().join(format!(
            "claude-context-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project = root.join("c--work");
        std::fs::create_dir_all(project.join("nested")).unwrap();
        let old = project.join("old.jsonl");
        let new = project.join("new.jsonl");
        std::fs::write(&old, "x").unwrap();
        std::fs::write(&new, "y").unwrap();
        let agent = project.join("agent-1.jsonl");
        std::fs::write(&agent, "z").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(60);
        for (path, offset) in [(&old, 0), (&new, 30), (&agent, 60)] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(later + std::time::Duration::from_secs(offset))
                .unwrap();
        }
        assert_eq!(newest_transcript(&root).map(|(path, _)| path), Some(new));
        assert!(read_tail(&old, 4).is_some_and(|tail| tail == "x"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
