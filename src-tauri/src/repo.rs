//! Read-only inspection of a directory the user is selecting as a project.

use std::path::Path;
use std::process::Command;

/// Manifests worth surfacing in the "Detected" list, in display order.
const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Gemfile",
    "composer.json",
    "Package.swift",
];

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn is_git_repo(path: &Path) -> bool {
    match git(path, &["rev-parse", "--is-inside-work-tree"]) {
        Some(value) => value == "true",
        // No git on PATH is not proof there is no repository.
        None => path.join(".git").exists(),
    }
}

/// The checked-out branch, or `None` on a detached HEAD or a non-repository.
pub fn current_branch(path: &Path) -> Option<String> {
    git(path, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|name| name != "HEAD")
}

/// The repository root, so selecting a subdirectory still records the whole repo.
pub fn repo_root(path: &Path) -> Option<String> {
    git(path, &["rev-parse", "--show-toplevel"])
}

pub fn manifests(path: &Path) -> Vec<String> {
    MANIFESTS
        .iter()
        .filter(|name| path.join(name).is_file())
        .map(|name| name.to_string())
        .collect()
}

/// One file the working tree has changed relative to HEAD.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    /// "added", "modified", "deleted", "renamed", or "untracked".
    pub state: String,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(serde::Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoChanges {
    pub files: Vec<ChangedFile>,
    pub insertions: u32,
    pub deletions: u32,
    /// False when the directory is not a repository, so the UI can say so.
    pub is_git: bool,
}

/// Turns a porcelain status code into something worth reading.
fn state_from_code(code: &str) -> &'static str {
    let code = code.trim();
    if code == "??" {
        return "untracked";
    }
    // Either side of the code may carry the interesting letter.
    if code.contains('D') {
        "deleted"
    } else if code.contains('R') {
        "renamed"
    } else if code.contains('A') {
        "added"
    } else {
        "modified"
    }
}

/// Line counts per file, from both staged and unstaged changes.
fn numstat(path: &Path, staged: bool) -> Vec<(String, u32, u32)> {
    let mut args = vec!["diff", "--numstat"];
    if staged {
        args.push("--cached");
    }
    let Some(output) = git(path, &args) else {
        return Vec::new();
    };
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let added = parts.next()?;
            let removed = parts.next()?;
            let file = parts.next()?;
            // Binary files report "-" rather than a count.
            Some((
                file.to_string(),
                added.parse().unwrap_or(0),
                removed.parse().unwrap_or(0),
            ))
        })
        .collect()
}

/// What the working tree has changed, for the "files changed" view.
pub fn changes(path: &Path) -> RepoChanges {
    if !is_git_repo(path) {
        return RepoChanges::default();
    }

    let mut counts: std::collections::HashMap<String, (u32, u32)> =
        std::collections::HashMap::new();
    for staged in [false, true] {
        for (file, added, removed) in numstat(path, staged) {
            let entry = counts.entry(file).or_insert((0, 0));
            entry.0 += added;
            entry.1 += removed;
        }
    }

    let listing = git(path, &["status", "--porcelain"]).unwrap_or_default();
    let mut files = Vec::new();
    for line in listing.lines() {
        if line.len() < 4 {
            continue;
        }
        let (code, rest) = line.split_at(2);
        // A rename reads "old -> new"; the new name is what matters here.
        let name = rest.trim();
        let name = name.rsplit(" -> ").next().unwrap_or(name).to_string();
        let (insertions, deletions) = counts.get(&name).copied().unwrap_or((0, 0));
        files.push(ChangedFile {
            state: state_from_code(code).to_string(),
            path: name,
            insertions,
            deletions,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    RepoChanges {
        insertions: files.iter().map(|file| file.insertions).sum(),
        deletions: files.iter().map(|file| file.deletions).sum(),
        files,
        is_git: true,
    }
}

/// The unified diff for one file, for the diff viewer.
pub fn file_diff(path: &Path, file: &str) -> Option<String> {
    // An untracked file has nothing to diff against, so show it as all additions.
    let tracked = git(path, &["ls-files", "--error-unmatch", file]).is_some();
    if !tracked {
        let content = std::fs::read_to_string(path.join(file)).ok()?;
        let body: String = content
            .lines()
            .take(400)
            .map(|line| format!("+{line}\n"))
            .collect();
        return Some(format!("--- /dev/null\n+++ b/{file}\n{body}"));
    }
    git(path, &["diff", "HEAD", "--", file]).or_else(|| Some(String::new()))
}

pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
