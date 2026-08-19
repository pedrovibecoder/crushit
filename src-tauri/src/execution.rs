//! Backend-owned state for a task being implemented by a coding agent.
//!
//! Like analysis, this lives outside the webview so closing the popup never
//! interrupts a run, and reopening shows it still going.

use crate::agent::Agent;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Enough activity to see what happened without turning the popup into a log.
const MAX_ACTIVITY: usize = 40;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStatus {
    #[default]
    Idle,
    Running,
    /// The agent asked permission and is blocked until the developer answers.
    AwaitingApproval,
    Finished,
    Failed,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub done: bool,
}

/// A permission request, described for a human rather than for the protocol.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: i64,
    pub title: String,
    pub command: Option<String>,
    pub reason: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSnapshot {
    pub status: ExecutionStatus,
    pub task_id: Option<i64>,
    pub task_title: String,
    pub agent: Agent,
    pub thread_id: Option<String>,
    pub activity: Vec<ActivityItem>,
    /// Repository-relative paths the agent reported touching.
    pub changed_files: Vec<String>,
    pub approval: Option<ApprovalRequest>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub started_at: i64,
}

#[derive(Default)]
struct Execution {
    snapshot: ExecutionSnapshot,
}

/// The cancel flag lives outside the mutex and is shared, so a worker can hand
/// a clone to a thread whose only job is to stop a child process.
#[derive(Default)]
pub struct ExecutionState {
    inner: Mutex<Execution>,
    cancelled: Arc<AtomicBool>,
}

impl ExecutionState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Execution> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A handle the worker can carry; it stays valid across `clear`.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    pub fn snapshot(&self) -> ExecutionSnapshot {
        self.lock().snapshot.clone()
    }

    /// True while a run is in flight, including while it waits on an approval.
    pub fn is_active(&self) -> bool {
        matches!(
            self.lock().snapshot.status,
            ExecutionStatus::Running | ExecutionStatus::AwaitingApproval
        )
    }

    pub fn is_awaiting_approval(&self) -> bool {
        self.lock().snapshot.status == ExecutionStatus::AwaitingApproval
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn task_id(&self) -> Option<i64> {
        self.lock().snapshot.task_id
    }

    pub fn begin(&self, task_id: i64, title: String, agent: Agent) -> ExecutionSnapshot {
        self.cancelled.store(false, Ordering::SeqCst);
        let mut execution = self.lock();
        execution.snapshot = ExecutionSnapshot {
            status: ExecutionStatus::Running,
            task_id: Some(task_id),
            task_title: title,
            agent,
            started_at: crate::models::now(),
            ..Default::default()
        };
        execution.snapshot.clone()
    }

    pub fn set_thread(&self, thread_id: String) -> ExecutionSnapshot {
        let mut execution = self.lock();
        execution.snapshot.thread_id = Some(thread_id);
        execution.snapshot.clone()
    }

    pub fn step_started(&self, id: String, label: String, kind: &str) -> ExecutionSnapshot {
        let mut execution = self.lock();
        let activity = &mut execution.snapshot.activity;
        if let Some(existing) = activity.iter_mut().find(|item| item.id == id) {
            existing.label = label;
        } else {
            activity.push(ActivityItem {
                id,
                label,
                kind: kind.to_string(),
                done: false,
            });
            if activity.len() > MAX_ACTIVITY {
                activity.remove(0);
            }
        }
        execution.snapshot.clone()
    }

    pub fn step_finished(&self, id: &str) -> ExecutionSnapshot {
        let mut execution = self.lock();
        if let Some(item) = execution
            .snapshot
            .activity
            .iter_mut()
            .find(|item| item.id == id)
        {
            item.done = true;
        }
        execution.snapshot.clone()
    }

    pub fn file_changed(&self, path: String) -> ExecutionSnapshot {
        let mut execution = self.lock();
        if !execution.snapshot.changed_files.contains(&path) {
            execution.snapshot.changed_files.push(path);
        }
        execution.snapshot.clone()
    }

    pub fn awaiting(&self, approval: ApprovalRequest) -> ExecutionSnapshot {
        let mut execution = self.lock();
        execution.snapshot.status = ExecutionStatus::AwaitingApproval;
        execution.snapshot.approval = Some(approval);
        execution.snapshot.clone()
    }

    /// Clears the request and returns to running, whichever way it was answered.
    pub fn approval_resolved(&self) -> ExecutionSnapshot {
        let mut execution = self.lock();
        execution.snapshot.approval = None;
        if execution.snapshot.status == ExecutionStatus::AwaitingApproval {
            execution.snapshot.status = ExecutionStatus::Running;
        }
        execution.snapshot.clone()
    }

    pub fn succeeded(&self, summary: Option<String>) -> ExecutionSnapshot {
        let mut execution = self.lock();
        execution.snapshot.status = ExecutionStatus::Finished;
        execution.snapshot.summary = summary;
        execution.snapshot.approval = None;
        execution.snapshot.error = None;
        for item in &mut execution.snapshot.activity {
            item.done = true;
        }
        execution.snapshot.clone()
    }

    pub fn failed(&self, error: String) -> ExecutionSnapshot {
        let mut execution = self.lock();
        execution.snapshot.status = ExecutionStatus::Failed;
        execution.snapshot.error = Some(error);
        execution.snapshot.approval = None;
        execution.snapshot.clone()
    }

    /// Asks the worker to stop. Shared, so a killer thread sees it too.
    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn clear(&self) -> ExecutionSnapshot {
        self.cancelled.store(false, Ordering::SeqCst);
        let mut execution = self.lock();
        *execution = Execution::default();
        execution.snapshot.clone()
    }

    /// Backstop for a worker that dies or wedges, mirroring analysis.
    pub fn fail_if_stalled(&self, max_seconds: i64) -> Option<ExecutionSnapshot> {
        let mut execution = self.lock();
        // A run parked on an approval is waiting on a person, not stuck.
        if execution.snapshot.status != ExecutionStatus::Running {
            return None;
        }
        if crate::models::now() - execution.snapshot.started_at < max_seconds {
            return None;
        }
        execution.snapshot.status = ExecutionStatus::Failed;
        execution.snapshot.error = Some("The run stopped responding and was abandoned.".into());
        Some(execution.snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approval() -> ApprovalRequest {
        ApprovalRequest {
            id: 4,
            title: "Run command?".into(),
            command: Some("npm install pdf-lib".into()),
            reason: Some("Required for PDF generation.".into()),
        }
    }

    #[test]
    fn a_fresh_state_is_idle() {
        let state = ExecutionState::default();
        assert_eq!(state.snapshot().status, ExecutionStatus::Idle);
        assert!(!state.is_active());
    }

    #[test]
    fn waiting_on_an_approval_still_counts_as_an_active_run() {
        let state = ExecutionState::default();
        state.begin(1, "PDF service".into(), Agent::Codex);
        state.awaiting(approval());
        assert!(state.is_active());
        assert!(state.is_awaiting_approval());
        assert_eq!(state.snapshot().approval.unwrap().id, 4);
    }

    #[test]
    fn resolving_an_approval_returns_the_run_to_running() {
        let state = ExecutionState::default();
        state.begin(1, "PDF service".into(), Agent::Codex);
        state.awaiting(approval());
        let snapshot = state.approval_resolved();
        assert_eq!(snapshot.status, ExecutionStatus::Running);
        assert!(snapshot.approval.is_none());
    }

    #[test]
    fn changed_files_are_recorded_once_each() {
        let state = ExecutionState::default();
        state.begin(1, "PDF service".into(), Agent::Codex);
        state.file_changed("src/pdf.ts".into());
        state.file_changed("src/pdf.ts".into());
        state.file_changed("src/api.ts".into());
        assert_eq!(state.snapshot().changed_files.len(), 2);
    }

    #[test]
    fn activity_is_keyed_by_id_and_stays_bounded() {
        let state = ExecutionState::default();
        state.begin(1, "PDF service".into(), Agent::Codex);
        for index in 0..(MAX_ACTIVITY + 6) {
            state.step_started(index.to_string(), format!("step {index}"), "command");
        }
        state.step_started("0".into(), "renamed".into(), "command");
        let snapshot = state.snapshot();
        assert!(snapshot.activity.len() <= MAX_ACTIVITY);
    }

    #[test]
    fn a_run_parked_on_approval_is_never_treated_as_stalled() {
        let state = ExecutionState::default();
        state.begin(1, "PDF service".into(), Agent::Codex);
        state.awaiting(approval());
        assert!(state.fail_if_stalled(0).is_none());
    }

    #[test]
    fn a_wedged_run_is_failed_so_the_menu_bar_recovers() {
        let state = ExecutionState::default();
        state.begin(1, "PDF service".into(), Agent::Codex);
        let snapshot = state.fail_if_stalled(0).expect("should fail");
        assert_eq!(snapshot.status, ExecutionStatus::Failed);
        assert!(state.fail_if_stalled(0).is_none(), "only reported once");
    }
}
