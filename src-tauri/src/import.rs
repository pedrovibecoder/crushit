//! Turning a file someone dropped on the window into a list of tasks.
//!
//! A screenshot of a board, a CSV export, a spreadsheet — they all say the same
//! thing in different shapes, so rather than parse each format here the file is
//! handed to the coding agent, which already knows how to read one. What comes
//! back is an ordinary plan, reviewed and accepted through the same screen a
//! planned goal is.

use crate::error::{Error, Result};
use crate::models::now;
use crate::plan::PlanRequest;
use std::path::{Path, PathBuf};

/// An imported list is someone's whole backlog, not one sitting of work, so it
/// is allowed to be much longer than a plan the agent writes itself.
pub const MAX_IMPORTED_TASKS: usize = 40;

/// What can be dropped. Anything else is refused up front rather than spending
/// an agent turn discovering it cannot be read.
const SUPPORTED: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "heic", // screenshots
    "csv", "tsv", "xlsx", "xls", "numbers", // exports
    "md", "txt", "json", // notes
];

/// True for a file this can make sense of.
pub fn is_supported(path: &Path) -> bool {
    extension(path).is_some_and(|ext| SUPPORTED.contains(&ext.as_str()))
}

fn extension(path: &Path) -> Option<String> {
    Some(path.extension()?.to_string_lossy().to_lowercase())
}

/// The name shown on the goal this import becomes.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "dropped file".to_string())
}

/// Copies the dropped file somewhere the agent is allowed to read.
///
/// The agent runs read-only with its working directory as the sandbox root, and
/// what was dropped is usually nowhere near the project — a screenshot on the
/// desktop, a download. Staging it in its own directory gives the turn a root
/// that holds the file and nothing else.
pub fn stage(source: &Path, root: &Path) -> Result<PathBuf> {
    if !source.is_file() {
        return Err(Error::invalid("that is not a file"));
    }
    if !is_supported(source) {
        return Err(Error::invalid(format!(
            "{} cannot be read as a task list. Drop a screenshot, a CSV, or a spreadsheet.",
            display_name(source)
        )));
    }
    let imports = root.join("imports");
    // Copies are only needed for the length of one turn; clearing the old ones
    // on the way in keeps a year of screenshots from piling up unnoticed.
    sweep(&imports, KEEP_STAGED_FOR_SECONDS);
    let directory = imports.join(now().to_string());
    std::fs::create_dir_all(&directory)?;
    let staged = directory.join(sanitised_name(source));
    std::fs::copy(source, &staged)?;
    Ok(staged)
}

/// How long a staged copy is kept before the next import clears it away.
const KEEP_STAGED_FOR_SECONDS: i64 = 24 * 60 * 60;

/// Removes staged copies older than `max_age`. Best effort: a file that will
/// not delete is not worth failing an import over.
fn sweep(imports: &Path, max_age: i64) {
    let Ok(entries) = std::fs::read_dir(imports) else {
        return;
    };
    let cutoff = now() - max_age;
    for entry in entries.flatten() {
        // Each staged directory is named for the second it was made.
        let stamped = entry
            .file_name()
            .to_string_lossy()
            .parse::<i64>()
            .unwrap_or(i64::MAX);
        if stamped < cutoff {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Keeps the extension — the agent decides how to read the file by its name —
/// while making sure the name itself cannot walk out of the directory.
fn sanitised_name(path: &Path) -> String {
    let name = display_name(path);
    let cleaned: String = name
        .chars()
        .map(|ch| if ch == '/' || ch == '\\' { '-' } else { ch })
        .collect();
    let trimmed = cleaned.trim_matches('.').trim();
    if trimmed.is_empty() {
        "dropped".to_string()
    } else {
        trimmed.to_string()
    }
}

/// What to ask about a staged file.
pub fn request(file_name: &str, categories: Vec<String>) -> PlanRequest {
    PlanRequest {
        prompt: prompt(file_name),
        categories,
        max_tasks: MAX_IMPORTED_TASKS,
    }
}

fn prompt(file_name: &str) -> String {
    let scale = crate::models::STORY_POINTS
        .iter()
        .map(|point| point.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "The file `{file_name}` in your working directory is an existing task list — \
         a screenshot of a board, a spreadsheet export, or notes. Read it and turn it \
         into tasks.\n\n\
         Read the file however suits it: view it if it is an image, and read or convert \
         it if it is a spreadsheet. Do not modify anything, and do not look outside this \
         directory.\n\n\
         Rules:\n\
         - One task per item in the list. Keep the author's own wording in `title`.\n\
         - Do not invent work that is not in the file, and do not merge two items into one.\n\
         - Anything the file says about an item — notes, a description, a column of \
           detail — goes in `description`. Leave it empty rather than padding it.\n\
         - `storyPoints` sizes each task against the others on the Fibonacci scale \
           ({scale}), from what the item says. Estimate rather than leaving it out.\n\
         - `estimateMinutes` is a realistic whole number of minutes, or 0 if the file \
           gives nothing to go on.\n\
         - `acceptanceCriteria` only where the file states them. An empty list is fine.\n\
         - `relevantFiles` only where the list names one. An empty list is fine.\n\
         - `dependsOn` only where the list says one item waits on another.\n\
         - `summary` says what the list is and how many items you found.\n\
         - `existing` records what you could read of the file; `missing` records anything \
           you could not make out, such as a truncated column or unreadable text.\n\
         - At most {MAX_IMPORTED_TASKS} tasks. If the list is longer, take the first \
           {MAX_IMPORTED_TASKS} in the order they appear and say so in `summary`."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_formats_a_task_list_arrives_in_are_accepted() {
        for name in ["board.png", "Backlog.CSV", "sprint.xlsx", "notes.md"] {
            assert!(is_supported(Path::new(name)), "{name} should be supported");
        }
    }

    #[test]
    fn something_that_is_not_a_task_list_is_refused() {
        for name in ["archive.zip", "video.mov", "binary"] {
            assert!(!is_supported(Path::new(name)), "{name} should be refused");
        }
    }

    #[test]
    fn a_staged_name_cannot_walk_out_of_its_directory() {
        assert_eq!(sanitised_name(Path::new("/tmp/../../etc/passwd")), "passwd");
        assert!(!sanitised_name(Path::new("/tmp/a.csv")).contains('/'));
    }

    #[test]
    fn a_dropped_file_is_staged_alone_and_old_copies_are_swept_away() {
        let root = std::env::temp_dir().join(format!("crushit-import-{}", now()));
        let source = root.join("board.csv");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source, "title\nDo the thing\n").unwrap();

        // A copy left behind by an import from long ago.
        let stale = root.join("imports").join("1");
        std::fs::create_dir_all(&stale).unwrap();

        let staged = stage(&source, &root).unwrap();
        assert!(staged.is_file());
        assert_eq!(staged.file_name().unwrap(), "board.csv");
        // Nothing but the copy sits in the directory the agent is given.
        let directory = staged.parent().unwrap();
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
        assert!(!stale.exists(), "the stale copy should have been swept");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn something_that_cannot_be_read_is_refused_before_an_agent_is_spent() {
        let root = std::env::temp_dir().join(format!("crushit-import-bad-{}", now()));
        let source = root.join("archive.zip");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source, "not a task list").unwrap();

        let refused = stage(&source, &root).unwrap_err().to_string();
        assert!(refused.contains("archive.zip"), "{refused}");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_prompt_names_the_file_and_the_scale() {
        let text = prompt("board.png");
        assert!(text.contains("board.png"));
        assert!(text.contains("1, 2, 3, 5, 8, 13"));
        assert!(text.contains("Do not invent work"));
    }

    #[test]
    fn an_import_may_be_longer_than_a_plan_the_agent_writes_itself() {
        assert!(MAX_IMPORTED_TASKS > crate::plan::MAX_TASKS);
        assert_eq!(request("a.csv", vec![]).max_tasks, MAX_IMPORTED_TASKS);
    }
}
