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

pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
