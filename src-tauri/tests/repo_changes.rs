//! Change detection against a real repository built for the test. No agent and
//! no network, so this runs with the normal suite.

use crushit_lib::repo;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("git should run");
    assert!(status.success(), "git {args:?} failed");
}

/// A repository with one commit, in a directory unique to this test.
fn a_repository(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("crushit-repo-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/kept.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.join("src/gone.txt"), "bye\n").unwrap();

    git(&dir, &["init", "-q"]);
    git(&dir, &["add", "-A"]);
    git(
        &dir,
        &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "base"],
    );
    dir
}

#[test]
fn a_clean_repository_reports_nothing_changed() {
    let dir = a_repository("clean");
    let changes = repo::changes(&dir);
    assert!(changes.is_git);
    assert!(changes.files.is_empty());
    assert_eq!((changes.insertions, changes.deletions), (0, 0));
}

#[test]
fn every_kind_of_change_is_reported_with_its_line_counts() {
    let dir = a_repository("mixed");
    std::fs::write(dir.join("src/kept.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    std::fs::remove_file(dir.join("src/gone.txt")).unwrap();
    std::fs::write(dir.join("src/new.txt"), "fresh\n").unwrap();

    let changes = repo::changes(&dir);
    let by_path = |name: &str| {
        changes
            .files
            .iter()
            .find(|file| file.path == name)
            .unwrap_or_else(|| panic!("expected {name} in {:?}", changes.files))
            .clone()
    };

    assert_eq!(by_path("src/kept.txt").state, "modified");
    assert_eq!(by_path("src/kept.txt").insertions, 1);
    assert_eq!(by_path("src/gone.txt").state, "deleted");
    assert_eq!(by_path("src/gone.txt").deletions, 1);
    assert_eq!(by_path("src/new.txt").state, "untracked");

    // The totals are what the review header shows.
    assert_eq!(changes.insertions, 1);
    assert_eq!(changes.deletions, 1);
}

#[test]
fn a_staged_change_counts_the_same_as_an_unstaged_one() {
    let dir = a_repository("staged");
    std::fs::write(dir.join("src/kept.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    git(&dir, &["add", "src/kept.txt"]);

    let changes = repo::changes(&dir);
    assert_eq!(changes.files.len(), 1);
    assert_eq!(changes.insertions, 1, "staged lines must still be counted");
}

#[test]
fn a_tracked_file_diffs_against_the_commit() {
    let dir = a_repository("diff");
    std::fs::write(dir.join("src/kept.txt"), "one\ntwo\nthree\nfour\n").unwrap();

    let diff = repo::file_diff(&dir, "src/kept.txt").expect("a diff");
    assert!(diff.contains("+four"), "the added line should appear: {diff}");
    assert!(diff.contains("@@"), "a unified diff has hunk headers");
}

#[test]
fn an_untracked_file_is_shown_as_all_additions() {
    let dir = a_repository("untracked");
    std::fs::write(dir.join("src/new.txt"), "fresh\nlines\n").unwrap();

    let diff = repo::file_diff(&dir, "src/new.txt").expect("a diff");
    assert!(diff.contains("+fresh"));
    assert!(diff.contains("+lines"));
    assert!(diff.contains("/dev/null"), "nothing to compare against");
}

#[test]
fn a_directory_without_git_is_reported_rather_than_erroring() {
    let dir = std::env::temp_dir().join("crushit-repo-test-nogit");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let changes = repo::changes(&dir);
    assert!(!changes.is_git);
    assert!(changes.files.is_empty());
}
