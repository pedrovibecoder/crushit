//! Finding the Codex CLI and asking it about its own state.
//!
//! Blitzit never reads Codex credentials; it only asks the CLI whether it is
//! signed in and reports the answer.

use crate::agent::{self, Agent, AgentStatus};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories a standalone Codex install commonly ends up in.
const COMMON_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    ".local/bin",
    ".codex/bin",
    ".bun/bin",
    ".volta/bin",
];

/// The desktop app ships its own Codex, which is often newer than anything on
/// `PATH` — a stale npm install can easily shadow it.
const BUNDLED: &[&str] = &[
    "/Applications/ChatGPT.app/Contents/Resources/codex",
    "Applications/ChatGPT.app/Contents/Resources/codex",
];

/// Node version managers keep one copy per installed runtime.
fn scan_node_versions() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let nvm = home?.join(".nvm/versions/node");
    let mut versions: Vec<_> = std::fs::read_dir(nvm)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    versions.sort();
    versions
        .into_iter()
        .rev()
        .map(|version| version.join("bin/codex"))
        .find(|candidate| agent::is_executable(candidate))
}

/// Every Codex we can find, in no particular order.
fn candidates() -> Vec<PathBuf> {
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let mut found: Vec<PathBuf> = Vec::new();

    if let Some(path) = agent::ask_login_shell("codex") {
        found.push(path);
    }
    for entry in BUNDLED {
        let path = match (entry.starts_with('/'), &home) {
            (true, _) => PathBuf::from(entry),
            (false, Some(home)) => home.join(entry),
            (false, None) => continue,
        };
        if agent::is_executable(&path) {
            found.push(path);
        }
    }
    for dir in COMMON_DIRS {
        let base = match (dir.starts_with('/'), &home) {
            (true, _) => PathBuf::from(dir),
            (false, Some(home)) => home.join(dir),
            (false, None) => continue,
        };
        let path = base.join("codex");
        if agent::is_executable(&path) {
            found.push(path);
        }
    }
    if let Some(path) = scan_node_versions() {
        found.push(path);
    }

    found.sort();
    found.dedup();
    found
}

pub fn find_binary(override_path: Option<&str>) -> Option<PathBuf> {
    agent::resolve(override_path, candidates())
}

/// Reads how Codex is authenticated without ever touching the token itself.
fn login_status(binary: &Path) -> (bool, Option<String>, Option<String>) {
    let output = match Command::new(binary).args(["login", "status"]).output() {
        Ok(output) => output,
        Err(error) => return (false, None, Some(error.to_string())),
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() || !text.to_lowercase().contains("logged in") {
        return (false, None, None);
    }
    let mode = if text.contains("ChatGPT") {
        "ChatGPT"
    } else if text.to_lowercase().contains("api key") {
        "API key"
    } else {
        "Codex"
    };
    (true, Some(mode.to_string()), None)
}

pub fn status(override_path: Option<&str>) -> AgentStatus {
    let base = AgentStatus {
        agent: Agent::Codex,
        ..Default::default()
    };

    let Some(binary) = find_binary(override_path) else {
        return AgentStatus {
            problem: Some("Codex CLI not found. Install it, or set its path in Settings.".into()),
            ..base
        };
    };

    let Some(version) = agent::read_version(&binary) else {
        return AgentStatus {
            path: Some(binary.to_string_lossy().to_string()),
            problem: Some("Found a codex binary but it did not report a version.".into()),
            ..base
        };
    };

    let (signed_in, auth_mode, problem) = login_status(&binary);
    AgentStatus {
        installed: true,
        path: Some(binary.to_string_lossy().to_string()),
        version: Some(version.to_string()),
        signed_in,
        auth_mode,
        problem,
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_override_falls_through_to_normal_discovery() {
        assert_eq!(find_binary(Some("   ")), find_binary(None));
    }

    #[test]
    fn a_missing_cli_is_reported_rather_than_guessed_at() {
        let status = status(Some("/nonexistent/codex"));
        assert!(!status.installed);
        assert!(status.problem.is_some());
        assert_eq!(status.agent, Agent::Codex);
    }
}
