//! Finding the Claude Code CLI and asking it about its own state.
//!
//! Crushit never reads Claude credentials; it only asks the CLI whether it is
//! signed in and reports the answer.

use crate::agent::{self, Agent, AgentStatus};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories a Claude Code install commonly ends up in.
const COMMON_DIRS: &[&str] = &[
    ".local/bin",
    ".claude/local",
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    ".bun/bin",
    ".volta/bin",
];

fn candidates() -> Vec<PathBuf> {
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let mut found: Vec<PathBuf> = Vec::new();

    if let Some(path) = agent::ask_login_shell("claude") {
        found.push(path);
    }
    for dir in COMMON_DIRS {
        let base = match (dir.starts_with('/'), &home) {
            (true, _) => PathBuf::from(dir),
            (false, Some(home)) => home.join(dir),
            (false, None) => continue,
        };
        let path = base.join("claude");
        if agent::is_executable(&path) {
            found.push(path);
        }
    }

    found.sort();
    found.dedup();
    found
}

pub fn find_binary(override_path: Option<&str>) -> Option<PathBuf> {
    agent::resolve(override_path, candidates())
}

/// Reads how Claude is authenticated without ever touching the token itself.
fn login_status(binary: &Path) -> (bool, Option<String>, Option<String>) {
    let output = match Command::new(binary).args(["auth", "status"]).output() {
        Ok(output) => output,
        Err(error) => return (false, None, Some(error.to_string())),
    };
    let text = String::from_utf8_lossy(&output.stdout);

    // Recent builds answer with JSON; older ones print a sentence.
    if let Ok(value) = serde_json::from_str::<Value>(text.trim()) {
        let signed_in = value.get("loggedIn").and_then(Value::as_bool).unwrap_or(false);
        let mode = value
            .get("authMethod")
            .and_then(Value::as_str)
            .map(str::to_string);
        return (signed_in, mode.filter(|_| signed_in), None);
    }

    let lower = text.to_lowercase();
    let signed_in = output.status.success() && !lower.contains("not logged in");
    (signed_in, signed_in.then(|| "Claude".to_string()), None)
}

pub fn status(override_path: Option<&str>) -> AgentStatus {
    let base = AgentStatus {
        agent: Agent::ClaudeCode,
        ..Default::default()
    };

    let Some(binary) = find_binary(override_path) else {
        return AgentStatus {
            problem: Some(
                "Claude Code CLI not found. Install it, or set its path in Settings.".into(),
            ),
            ..base
        };
    };

    let Some(version) = agent::read_version(&binary) else {
        return AgentStatus {
            path: Some(binary.to_string_lossy().to_string()),
            problem: Some("Found a claude binary but it did not report a version.".into()),
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
        let status = status(Some("/nonexistent/claude"));
        assert!(!status.installed);
        assert!(status.problem.is_some());
        assert_eq!(status.agent, Agent::ClaudeCode);
    }
}
