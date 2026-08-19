//! The coding agent Blitzit talks to, and the plumbing both agents share.
//!
//! The PRD scopes V1 to Codex; Claude Code was added on request, so the two
//! are kept behind one small surface rather than threaded through the app.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    #[default]
    Codex,
    ClaudeCode,
}

impl Agent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Agent::Codex => "codex",
            Agent::ClaudeCode => "claude-code",
        }
    }

    /// How the agent is named in the interface.
    pub fn label(&self) -> &'static str {
        match self {
            Agent::Codex => "Codex",
            Agent::ClaudeCode => "Claude Code",
        }
    }

    pub fn parse_lenient(raw: &str) -> Self {
        match raw {
            "claude-code" => Agent::ClaudeCode,
            _ => Agent::Codex,
        }
    }

    pub const ALL: [Agent; 2] = [Agent::Codex, Agent::ClaudeCode];
}

impl rusqlite::ToSql for Agent {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub agent: Agent,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub signed_in: bool,
    /// How the agent is authenticated, e.g. "ChatGPT" or "claude.ai".
    pub auth_mode: Option<String>,
    /// Why detection or the login check failed, for the UI to show verbatim.
    pub problem: Option<String>,
}

// ------------------------------------------------------------- discovery

/// A CLI version, ordered so the newest install can be preferred.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Version {
    numbers: (u64, u64, u64),
    /// `None` outranks `Some`: 0.148.0 is newer than 0.148.0-alpha.15.
    prerelease: Option<String>,
}

impl Version {
    /// Parses a version out of output like `codex-cli 0.148.0-alpha.15` or
    /// `2.1.170 (Claude Code)`.
    pub fn parse(text: &str) -> Option<Self> {
        text.split_whitespace().find_map(Self::parse_token)
    }

    fn parse_token(token: &str) -> Option<Self> {
        let raw = token.trim().trim_start_matches('v');
        let (core, prerelease) = match raw.split_once('-') {
            Some((core, rest)) => (core, Some(rest.to_string())),
            None => (raw, None),
        };
        if !core.contains('.') {
            return None;
        }
        let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
        let numbers = (
            parts.next()??,
            parts.next().flatten().unwrap_or(0),
            parts.next().flatten().unwrap_or(0),
        );
        Some(Version { numbers, prerelease })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        self.numbers.cmp(&other.numbers).then_with(|| {
            match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (major, minor, patch) = self.numbers;
        write!(f, "{major}.{minor}.{patch}")?;
        match &self.prerelease {
            Some(tag) => write!(f, "-{tag}"),
            None => Ok(()),
        }
    }
}

pub fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// A GUI app inherits a minimal `PATH`, so ask the login shell where a CLI is.
pub fn ask_login_shell(command: &str) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let output = Command::new(&shell)
        .args(["-lic", &format!("command -v {command}")])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let candidate = PathBuf::from(text.lines().next()?.trim());
    is_executable(&candidate).then_some(candidate)
}

pub fn read_version(binary: &Path) -> Option<Version> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Version::parse(String::from_utf8_lossy(&output.stdout).trim())
}

/// Picks the highest-versioned binary from a candidate list, so a stale copy
/// earlier on `PATH` cannot shadow a newer install elsewhere.
pub fn newest(paths: Vec<PathBuf>) -> Option<(PathBuf, Version)> {
    paths
        .into_iter()
        .filter_map(|path| read_version(&path).map(|version| (path, version)))
        .max_by(|(_, left), (_, right)| left.cmp(right))
}

/// Resolves a binary, preferring an explicit override over discovery.
pub fn resolve(override_path: Option<&str>, candidates: Vec<PathBuf>) -> Option<PathBuf> {
    if let Some(raw) = override_path.map(str::trim).filter(|value| !value.is_empty()) {
        let candidate = PathBuf::from(raw);
        return is_executable(&candidate).then_some(candidate);
    }
    newest(candidates).map(|(path, _)| path)
}

// ------------------------------------------------------------------ cache

/// How long a detection result stays good for.
pub const STATUS_TTL: Duration = Duration::from_secs(60);

/// Detection is expensive: resolving a binary asks the login shell where it is
/// (~0.5s because the profile is sourced), then runs `--version` on every
/// candidate, then asks the CLI whether it is signed in. For Claude Code that
/// adds up to well over a second, which is far too slow to repeat every time a
/// screen opens.
#[derive(Default)]
pub struct StatusCache {
    entries: Mutex<HashMap<String, (Instant, AgentStatus)>>,
}

impl StatusCache {
    fn key(agent: Agent, override_path: Option<&str>) -> String {
        format!("{}|{}", agent.as_str(), override_path.unwrap_or(""))
    }

    /// Returns a cached status, probing only when stale or forced.
    pub fn get_or_probe(
        &self,
        agent: Agent,
        override_path: Option<&str>,
        force: bool,
        probe: impl FnOnce() -> AgentStatus,
    ) -> AgentStatus {
        let key = Self::key(agent, override_path);
        if !force {
            let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((at, status)) = entries.get(&key) {
                if at.elapsed() < STATUS_TTL {
                    return status.clone();
                }
            }
        }
        let status = probe();
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, (Instant::now(), status.clone()));
        status
    }

    /// The binary a fresh probe already resolved, if there is one.
    pub fn cached_path(&self, agent: Agent, override_path: Option<&str>) -> Option<PathBuf> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let (at, status) = entries.get(&Self::key(agent, override_path))?;
        if at.elapsed() >= STATUS_TTL || !status.installed {
            return None;
        }
        status.path.as_ref().map(PathBuf::from)
    }

    pub fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

// --------------------------------------------------------------- dispatch

pub fn find_binary(agent: Agent, override_path: Option<&str>) -> Option<PathBuf> {
    match agent {
        Agent::Codex => crate::codex::detect::find_binary(override_path),
        Agent::ClaudeCode => crate::claude::detect::find_binary(override_path),
    }
}

pub fn status(agent: Agent, override_path: Option<&str>) -> AgentStatus {
    match agent {
        Agent::Codex => crate::codex::detect::status(override_path),
        Agent::ClaudeCode => crate::claude::detect::status(override_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_numerically_not_lexically() {
        let older = Version::parse("codex-cli 0.132.0").unwrap();
        let newer = Version::parse("codex-cli 0.148.0-alpha.15").unwrap();
        assert!(newer > older, "0.148.0-alpha.15 should beat 0.132.0");
    }

    #[test]
    fn a_release_outranks_its_own_prerelease() {
        assert!(Version::parse("0.148.0").unwrap() > Version::parse("0.148.0-alpha.15").unwrap());
    }

    #[test]
    fn a_version_is_found_wherever_it_sits_in_the_line() {
        // Codex prints "codex-cli 0.148.0"; Claude prints "2.1.170 (Claude Code)".
        assert_eq!(Version::parse("codex-cli 0.148.0").unwrap().to_string(), "0.148.0");
        assert_eq!(Version::parse("2.1.170 (Claude Code)").unwrap().to_string(), "2.1.170");
    }

    #[test]
    fn junk_output_is_not_mistaken_for_a_version() {
        assert!(Version::parse("command not found").is_none());
        assert!(Version::parse("").is_none());
    }

    #[test]
    fn an_override_that_does_not_exist_is_ignored_rather_than_used() {
        assert_eq!(resolve(Some("/nonexistent/agent"), vec![]), None);
    }

    fn a_status(installed: bool) -> AgentStatus {
        AgentStatus {
            agent: Agent::Codex,
            installed,
            path: Some("/bin/codex".into()),
            ..Default::default()
        }
    }

    #[test]
    fn a_repeat_lookup_does_not_probe_again() {
        let cache = StatusCache::default();
        let probes = std::cell::Cell::new(0);
        let mut probe = || {
            probes.set(probes.get() + 1);
            a_status(true)
        };
        cache.get_or_probe(Agent::Codex, None, false, &mut probe);
        cache.get_or_probe(Agent::Codex, None, false, &mut probe);
        assert_eq!(probes.get(), 1, "the second lookup should be served from cache");
    }

    #[test]
    fn forcing_re_probes_even_when_fresh() {
        let cache = StatusCache::default();
        let probes = std::cell::Cell::new(0);
        let mut probe = || {
            probes.set(probes.get() + 1);
            a_status(true)
        };
        cache.get_or_probe(Agent::Codex, None, false, &mut probe);
        cache.get_or_probe(Agent::Codex, None, true, &mut probe);
        assert_eq!(probes.get(), 2);
    }

    #[test]
    fn each_agent_and_override_is_cached_separately() {
        let cache = StatusCache::default();
        let probes = std::cell::Cell::new(0);
        let mut probe = || {
            probes.set(probes.get() + 1);
            a_status(true)
        };
        cache.get_or_probe(Agent::Codex, None, false, &mut probe);
        cache.get_or_probe(Agent::ClaudeCode, None, false, &mut probe);
        cache.get_or_probe(Agent::Codex, Some("/custom"), false, &mut probe);
        assert_eq!(probes.get(), 3);
    }

    #[test]
    fn a_cached_path_is_only_offered_for_an_installed_agent() {
        let cache = StatusCache::default();
        cache.get_or_probe(Agent::Codex, None, false, || a_status(false));
        assert_eq!(cache.cached_path(Agent::Codex, None), None);

        cache.clear();
        cache.get_or_probe(Agent::Codex, None, false, || a_status(true));
        assert_eq!(
            cache.cached_path(Agent::Codex, None),
            Some(PathBuf::from("/bin/codex"))
        );
    }

    #[test]
    fn clearing_forces_the_next_lookup_to_probe() {
        let cache = StatusCache::default();
        let probes = std::cell::Cell::new(0);
        let mut probe = || {
            probes.set(probes.get() + 1);
            a_status(true)
        };
        cache.get_or_probe(Agent::Codex, None, false, &mut probe);
        cache.clear();
        cache.get_or_probe(Agent::Codex, None, false, &mut probe);
        assert_eq!(probes.get(), 2);
    }

    #[test]
    fn agent_names_round_trip() {
        for agent in Agent::ALL {
            assert_eq!(Agent::parse_lenient(agent.as_str()), agent);
        }
        assert_eq!(Agent::parse_lenient("something-else"), Agent::Codex);
    }
}
