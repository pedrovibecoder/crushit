//! What both agents need in order to implement a task: the instruction they
//! are given, and the events a run reports back.

use crate::execution::ApprovalRequest;
use crate::models::Task;

/// A run may take a while, but not forever.
pub const RUN_TIMEOUT_SECONDS: u64 = 30 * 60;

#[derive(Debug)]
pub enum RunEvent {
    /// The agent's conversation handle, stored so the task can be resumed.
    Thread(String),
    Started {
        id: String,
        label: String,
        kind: &'static str,
    },
    Finished {
        id: String,
    },
    FileChanged(String),
    Approval(ApprovalRequest),
    /// The request was answered, one way or the other.
    ApprovalResolved,
    /// The agent's closing message.
    Message(String),
}

pub struct RunOutcome {
    pub thread_id: String,
    pub summary: Option<String>,
}

/// The instruction given to an agent asked to implement one task.
pub fn task_prompt(task: &Task) -> String {
    let mut prompt = format!("Implement this task in the current repository.\n\nTASK\n{}", task.title);

    if let Some(description) = task.description.as_deref().filter(|text| !text.is_empty()) {
        prompt.push_str(&format!("\n\n{description}"));
    }

    if !task.criteria.is_empty() {
        prompt.push_str("\n\nACCEPTANCE CRITERIA");
        for criterion in &task.criteria {
            let mark = if criterion.is_met { "done" } else { "todo" };
            prompt.push_str(&format!("\n- [{mark}] {}", criterion.text));
        }
    }

    if !task.files.is_empty() {
        prompt.push_str("\n\nLIKELY FILES (a starting point, not a limit)");
        for file in &task.files {
            prompt.push_str(&format!("\n- {file}"));
        }
    }

    prompt.push_str(
        "\n\nRules:\n\
         - Change only what this task needs. Leave unrelated code alone.\n\
         - Do not commit, push, or change git history; leave the work in the tree.\n\
         - Follow the conventions already in the repository.\n\
         - Finish with a short summary of what you changed and anything left undone.",
    );
    prompt
}

/// Watches for cancellation and kills the process when it comes.
///
/// A CLI that has gone quiet produces no output, so a loop that only checks
/// for cancellation between lines will not notice one for as long as the agent
/// is thinking. Stopping has to reach the process itself.
///
/// Returns a flag to set once the run is over, which retires the watcher.
pub fn spawn_canceller(
    child: std::sync::Arc<std::sync::Mutex<std::process::Child>>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    use std::sync::atomic::Ordering;

    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done = finished.clone();
    std::thread::spawn(move || {
        while !done.load(Ordering::SeqCst) {
            if cancelled.load(Ordering::SeqCst) {
                let mut child = child.lock().unwrap_or_else(|e| e.into_inner());
                let _ = child.kill();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    finished
}

/// Presents a touched file the way the repository refers to it. Agents report
/// absolute paths, which are too long to read and mean nothing to the user.
pub fn relative_to(project_path: &str, path: &str) -> String {
    let project = std::path::Path::new(project_path);
    let candidate = std::path::Path::new(path);
    candidate
        .strip_prefix(project)
        .map(|rest| rest.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
}

/// Trims a command down to something that fits a one-line activity list.
pub fn short(text: &str, limit: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= limit {
        return flat;
    }
    flat.chars().take(limit).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AcceptanceCriterion, TaskCategory, TaskStatus};

    fn a_task() -> Task {
        Task {
            id: 1,
            project_id: 1,
            goal_id: None,
            title: "Create invoice PDF service".into(),
            description: Some("Render an invoice to a PDF buffer.".into()),
            category: TaskCategory::Backend,
            status: TaskStatus::InProgress,
            position: 0,
            estimate_minutes: Some(45),
            is_ai_generated: true,
            created_at: 0,
            updated_at: 0,
            completed_at: None,
            criteria: vec![
                AcceptanceCriterion {
                    id: 1,
                    task_id: 1,
                    text: "Generates a valid PDF".into(),
                    is_met: true,
                    position: 0,
                },
                AcceptanceCriterion {
                    id: 2,
                    task_id: 1,
                    text: "Handles a missing invoice".into(),
                    is_met: false,
                    position: 1,
                },
            ],
            files: vec!["src/services/invoice.ts".into()],
            depends_on: vec![],
            focus_seconds: 0,
            focus_sessions: 0,
        }
    }

    #[test]
    fn the_prompt_carries_everything_the_agent_needs() {
        let prompt = task_prompt(&a_task());
        assert!(prompt.contains("Create invoice PDF service"));
        assert!(prompt.contains("Render an invoice to a PDF buffer."));
        assert!(prompt.contains("Generates a valid PDF"));
        assert!(prompt.contains("src/services/invoice.ts"));
    }

    #[test]
    fn criteria_carry_whether_they_are_already_met() {
        let prompt = task_prompt(&a_task());
        assert!(prompt.contains("- [done] Generates a valid PDF"));
        assert!(prompt.contains("- [todo] Handles a missing invoice"));
    }

    #[test]
    fn the_agent_is_told_not_to_commit() {
        assert!(task_prompt(&a_task()).contains("Do not commit"));
    }

    #[test]
    fn a_task_with_no_extras_still_produces_a_usable_prompt() {
        let mut task = a_task();
        task.description = None;
        task.criteria.clear();
        task.files.clear();
        let prompt = task_prompt(&task);
        assert!(prompt.contains("Create invoice PDF service"));
        assert!(!prompt.contains("ACCEPTANCE CRITERIA"));
        assert!(!prompt.contains("LIKELY FILES"));
    }

    #[test]
    fn a_touched_file_is_shown_the_way_the_repository_names_it() {
        assert_eq!(
            relative_to("/Users/dev/app", "/Users/dev/app/src/server.js"),
            "src/server.js"
        );
    }

    #[test]
    fn a_path_outside_the_project_is_left_alone() {
        let outside = "/etc/hosts";
        assert_eq!(relative_to("/Users/dev/app", outside), outside);
        // An already-relative path is not mangled either.
        assert_eq!(relative_to("/Users/dev/app", "src/a.ts"), "src/a.ts");
    }

    #[test]
    fn long_commands_are_flattened_and_trimmed() {
        assert_eq!(short("ls   -la\n  /tmp", 40), "ls -la /tmp");
        let long = short(&"x".repeat(100), 10);
        assert_eq!(long.chars().count(), 11, "ten characters plus the ellipsis");
    }
}
