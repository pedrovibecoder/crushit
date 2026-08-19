//! System notifications for the moments the developer is not looking.
//!
//! The popup can be closed and the window hidden while work continues, so the
//! few events that need attention are announced. Anything that is visible on
//! screen anyway is not.

use crate::db::{self, Db};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// The events worth interrupting someone for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    /// A focus session ran out.
    FocusFinished { task: &'a str },
    /// The agent is blocked and cannot continue without an answer.
    ApprovalNeeded { task: &'a str },
    /// A task run ended, either way.
    RunFinished { task: &'a str, changed: usize },
    RunFailed { task: &'a str },
    /// A plan is waiting to be accepted.
    PlanReady { goal: &'a str, tasks: usize },
}

impl Event<'_> {
    fn title(&self) -> String {
        match self {
            Event::FocusFinished { .. } => "Focus session finished".into(),
            Event::ApprovalNeeded { .. } => "Waiting for your approval".into(),
            Event::RunFinished { .. } => "Task run finished".into(),
            Event::RunFailed { .. } => "Task run failed".into(),
            Event::PlanReady { .. } => "Plan ready".into(),
        }
    }

    fn body(&self) -> String {
        match self {
            Event::FocusFinished { task } => task.to_string(),
            Event::ApprovalNeeded { task } => format!("{task} — the agent needs an answer"),
            Event::RunFinished { task, changed } => {
                let files = if *changed == 1 { "file" } else { "files" };
                format!("{task} — {changed} {files} changed")
            }
            Event::RunFailed { task } => task.to_string(),
            Event::PlanReady { goal, tasks } => {
                let word = if *tasks == 1 { "task" } else { "tasks" };
                format!("{goal} — {tasks} {word} to review")
            }
        }
    }
}

/// Shows a notification, unless the developer has turned them off.
pub fn send(app: &AppHandle, event: Event<'_>) {
    let enabled = app
        .try_state::<Db>()
        .and_then(|db| db::get_settings(&db.conn()).ok())
        .map(|settings| settings.notifications)
        .unwrap_or(true);
    if !enabled {
        return;
    }

    let _ = app
        .notification()
        .builder()
        .title(event.title())
        .body(event.body())
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_event_says_what_happened_and_to_which_task() {
        let cases = [
            Event::FocusFinished { task: "PDF service" },
            Event::ApprovalNeeded { task: "PDF service" },
            Event::RunFinished {
                task: "PDF service",
                changed: 2,
            },
            Event::RunFailed { task: "PDF service" },
            Event::PlanReady {
                goal: "Invoice downloads",
                tasks: 5,
            },
        ];
        for event in cases {
            assert!(!event.title().is_empty());
            assert!(!event.body().is_empty(), "{event:?} needs a body");
        }
    }

    #[test]
    fn counts_are_pluralised() {
        assert!(Event::RunFinished {
            task: "t",
            changed: 1
        }
        .body()
        .contains("1 file changed"));
        assert!(Event::RunFinished {
            task: "t",
            changed: 3
        }
        .body()
        .contains("3 files changed"));
        assert!(Event::PlanReady {
            goal: "g",
            tasks: 1
        }
        .body()
        .contains("1 task"));
    }

    #[test]
    fn an_approval_reads_as_something_to_act_on() {
        let body = Event::ApprovalNeeded { task: "PDF service" }.body();
        assert!(body.contains("PDF service"));
        assert!(body.contains("needs an answer"));
    }
}
