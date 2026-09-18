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
    let mut section = context_from_transcript_tail(&tail)?;
    let settings = std::fs::read_to_string(config.join("settings.json")).unwrap_or_default();
    section.window = context_window(&settings);
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

/// The last complete assistant turn, searched from the end. Skipped: a line
/// still being written (fails to parse), a line that merely quotes the marker
/// inside a tool result, and the synthetic zero-usage entries Claude Code
/// writes on API errors and interrupts, which say nothing about the context.
fn context_from_transcript_tail(tail: &str) -> Option<ContextSection> {
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
            Some(ContextSection {
                tokens,
                model,
                ..Default::default()
            })
        })
}

/// Claude Code marks the long-context variant with a `[1m]` suffix on the
/// `model` in `settings.json`; anything else runs against the standard window.
fn context_window(settings: &str) -> u64 {
    let long_context = serde_json::from_str::<serde_json::Value>(settings)
        .ok()
        .and_then(|settings| {
            settings
                .get("model")?
                .as_str()
                .map(|model| model.contains("[1m]"))
        })
        .unwrap_or(false);
    if long_context {
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
        r#"{"type":"assistant","message":{"model":"claude-fable-5-1","usage":{"input_tokens":32,"cache_creation_input_tokens":2480,"cache_read_input_tokens":56847,"output_tokens":3676}}}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"log says \"type\":\"assistant\" here"}]}}"#,
        "\n",
        r#"{"type":"assistant","message":{"model":"claude-fable-5-1","usage":{"input_tokens":10,"cache_creation"#,
    );

    #[test]
    fn reads_the_last_complete_assistant_turn() {
        let section = context_from_transcript_tail(TAIL).expect("an assistant line");
        assert_eq!(section.tokens, 32 + 2480 + 56847);
        assert_eq!(section.model.as_deref(), Some("claude-fable-5-1"));
        assert!(context_from_transcript_tail("{\"type\":\"user\"}\n").is_none());
    }

    #[test]
    fn synthetic_error_turns_do_not_reset_the_context() {
        let tail = format!(
            "{TAIL}\n{}\n",
            r#"{"type":"assistant","message":{"model":"<synthetic>","usage":{"input_tokens":0,"output_tokens":0}}}"#
        );
        let section = context_from_transcript_tail(&tail).expect("the real turn before it");
        assert_eq!(section.tokens, 32 + 2480 + 56847);
    }

    #[test]
    fn the_long_context_marker_selects_the_million_token_window() {
        assert_eq!(
            context_window(r#"{"model": "claude-fable-5-1[1m]"}"#),
            LONG_WINDOW
        );
        assert_eq!(
            context_window(r#"{"model": "claude-fable-5-1"}"#),
            DEFAULT_WINDOW
        );
        // Only the model key counts, not a `[1m]` elsewhere in the file.
        assert_eq!(
            context_window(r#"{"model": "claude-opus-5", "env": {"X": "[1m]"}}"#),
            DEFAULT_WINDOW
        );
        assert_eq!(context_window(""), DEFAULT_WINDOW);
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
