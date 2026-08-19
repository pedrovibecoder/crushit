//! Backend-owned state for an in-flight repository analysis.
//!
//! Like the focus timer, this lives outside the webview so closing the popup
//! never cancels the work and reopening it shows the run still in progress.

use crate::plan::AnalysisStep;
use serde::Serialize;
use std::sync::Mutex;

/// Keeps the progress list short enough to read at a glance.
const MAX_STEPS: usize = 12;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AnalysisStatus {
    #[default]
    Idle,
    Running,
    /// A plan is stored on the goal, waiting for the developer's decision.
    Ready,
    Failed,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSnapshot {
    pub status: AnalysisStatus,
    pub goal_id: Option<i64>,
    pub project_id: Option<i64>,
    pub goal_title: String,
    pub steps: Vec<AnalysisStep>,
    pub error: Option<String>,
    pub started_at: i64,
}

#[derive(Default)]
struct Analysis {
    snapshot: AnalysisSnapshot,
    cancelled: bool,
}

#[derive(Default)]
pub struct AnalysisState(Mutex<Analysis>);

impl AnalysisState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Analysis> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> AnalysisSnapshot {
        self.lock().snapshot.clone()
    }

    pub fn is_running(&self) -> bool {
        self.lock().snapshot.status == AnalysisStatus::Running
    }

    pub fn is_cancelled(&self) -> bool {
        self.lock().cancelled
    }

    pub fn begin(&self, goal_id: i64, project_id: i64, goal_title: String) -> AnalysisSnapshot {
        let mut analysis = self.lock();
        analysis.cancelled = false;
        analysis.snapshot = AnalysisSnapshot {
            status: AnalysisStatus::Running,
            goal_id: Some(goal_id),
            project_id: Some(project_id),
            goal_title,
            steps: Vec::new(),
            error: None,
            started_at: crate::models::now(),
        };
        analysis.snapshot.clone()
    }

    pub fn step_started(&self, id: String, label: String) -> AnalysisSnapshot {
        let mut analysis = self.lock();
        let steps = &mut analysis.snapshot.steps;
        if let Some(existing) = steps.iter_mut().find(|step| step.id == id) {
            existing.label = label;
        } else {
            steps.push(AnalysisStep { id, label, done: false });
            if steps.len() > MAX_STEPS {
                // Drop the oldest finished step rather than the live one.
                if let Some(position) = steps.iter().position(|step| step.done) {
                    steps.remove(position);
                } else {
                    steps.remove(0);
                }
            }
        }
        analysis.snapshot.clone()
    }

    pub fn step_finished(&self, id: &str) -> AnalysisSnapshot {
        let mut analysis = self.lock();
        if let Some(step) = analysis.snapshot.steps.iter_mut().find(|step| step.id == id) {
            step.done = true;
        }
        analysis.snapshot.clone()
    }

    pub fn succeeded(&self) -> AnalysisSnapshot {
        let mut analysis = self.lock();
        analysis.snapshot.status = AnalysisStatus::Ready;
        analysis.snapshot.error = None;
        for step in &mut analysis.snapshot.steps {
            step.done = true;
        }
        analysis.snapshot.clone()
    }

    pub fn failed(&self, error: String) -> AnalysisSnapshot {
        let mut analysis = self.lock();
        analysis.snapshot.status = AnalysisStatus::Failed;
        analysis.snapshot.error = Some(error);
        analysis.snapshot.clone()
    }

    /// Fails a run that has been going far longer than any analysis should.
    ///
    /// The worker normally reaches a terminal state itself. This is the
    /// backstop for a worker that dies or wedges on I/O, so the menu bar can
    /// never be stuck reading "Coding…" forever.
    pub fn fail_if_stalled(&self, max_seconds: i64) -> Option<AnalysisSnapshot> {
        let mut analysis = self.lock();
        if analysis.snapshot.status != AnalysisStatus::Running {
            return None;
        }
        let elapsed = crate::models::now() - analysis.snapshot.started_at;
        if elapsed < max_seconds {
            return None;
        }
        analysis.snapshot.status = AnalysisStatus::Failed;
        analysis.snapshot.error =
            Some("The analysis stopped responding and was abandoned.".to_string());
        Some(analysis.snapshot.clone())
    }

    /// Asks the worker to stop; it notices between notifications.
    pub fn request_cancel(&self) {
        self.lock().cancelled = true;
    }

    pub fn clear(&self) -> AnalysisSnapshot {
        let mut analysis = self.lock();
        *analysis = Analysis::default();
        analysis.snapshot.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_state_is_idle() {
        let state = AnalysisState::default();
        assert_eq!(state.snapshot().status, AnalysisStatus::Idle);
        assert!(!state.is_running());
    }

    #[test]
    fn steps_complete_by_id_and_the_same_id_never_duplicates() {
        let state = AnalysisState::default();
        state.begin(1, 2, "Invoices".into());
        state.step_started("a".into(), "Running ls".into());
        state.step_started("a".into(), "Running ls -la".into());
        let snapshot = state.step_finished("a");
        assert_eq!(snapshot.steps.len(), 1);
        assert_eq!(snapshot.steps[0].label, "Running ls -la");
        assert!(snapshot.steps[0].done);
    }

    #[test]
    fn the_step_list_stays_bounded_and_keeps_the_live_step() {
        let state = AnalysisState::default();
        state.begin(1, 2, "Invoices".into());
        for index in 0..(MAX_STEPS + 4) {
            state.step_started(index.to_string(), format!("step {index}"));
            state.step_finished(&index.to_string());
        }
        state.step_started("live".into(), "still going".into());
        let snapshot = state.snapshot();
        assert!(snapshot.steps.len() <= MAX_STEPS);
        assert!(snapshot.steps.iter().any(|step| step.id == "live"));
    }

    #[test]
    fn finishing_marks_every_step_done() {
        let state = AnalysisState::default();
        state.begin(1, 2, "Invoices".into());
        state.step_started("a".into(), "Running".into());
        let snapshot = state.succeeded();
        assert_eq!(snapshot.status, AnalysisStatus::Ready);
        assert!(snapshot.steps.iter().all(|step| step.done));
    }

    #[test]
    fn a_stalled_run_is_failed_so_the_menu_bar_recovers() {
        let state = AnalysisState::default();
        state.begin(1, 2, "Invoices".into());
        assert!(state.fail_if_stalled(3600).is_none(), "a fresh run is not stalled");

        let snapshot = state.fail_if_stalled(0).expect("a run past its limit fails");
        assert_eq!(snapshot.status, AnalysisStatus::Failed);
        assert!(!state.is_running());
        // Once resolved it must not be reported again on every tick.
        assert!(state.fail_if_stalled(0).is_none());
    }

    #[test]
    fn a_finished_run_is_never_touched_by_the_watchdog() {
        let state = AnalysisState::default();
        state.begin(1, 2, "Invoices".into());
        state.succeeded();
        assert!(state.fail_if_stalled(0).is_none());
        assert_eq!(state.snapshot().status, AnalysisStatus::Ready);
    }

    #[test]
    fn a_new_run_clears_a_previous_failure() {
        let state = AnalysisState::default();
        state.begin(1, 2, "First".into());
        state.failed("usage limit".into());
        let snapshot = state.begin(2, 2, "Second".into());
        assert_eq!(snapshot.status, AnalysisStatus::Running);
        assert_eq!(snapshot.error, None);
        assert!(!state.is_cancelled());
    }
}
