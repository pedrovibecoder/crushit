//! The break timer.
//!
//! Separate from the focus timer on purpose: a break belongs to the developer
//! rather than to a task, so it records nothing against one and leaves no
//! session behind. It is held in memory only — a break is minutes long, and one
//! that does not survive a restart is not worth a table.

use crate::models::now;
use serde::Serialize;
use std::sync::Mutex;

/// What a break may be set to, in minutes.
pub const CHOICES: [i64; 4] = [5, 10, 20, 30];

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestStatus {
    Idle,
    Running,
    /// The break ran out and is waiting to be dismissed.
    Finished,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RestSnapshot {
    pub status: RestStatus,
    pub minutes: i64,
    pub duration_seconds: i64,
    pub elapsed_seconds: i64,
    pub remaining_seconds: i64,
}

impl Default for RestSnapshot {
    fn default() -> Self {
        Self {
            status: RestStatus::Idle,
            minutes: 0,
            duration_seconds: 0,
            elapsed_seconds: 0,
            remaining_seconds: 0,
        }
    }
}

#[derive(Debug, Default)]
struct Rest {
    status: RestStatus,
    duration_seconds: i64,
    started_at: i64,
}

impl Default for RestStatus {
    fn default() -> Self {
        RestStatus::Idle
    }
}

impl Rest {
    fn elapsed(&self) -> i64 {
        match self.status {
            RestStatus::Idle => 0,
            RestStatus::Running => (now() - self.started_at).clamp(0, self.duration_seconds),
            RestStatus::Finished => self.duration_seconds,
        }
    }

    fn snapshot(&self) -> RestSnapshot {
        let elapsed = self.elapsed();
        RestSnapshot {
            status: self.status,
            minutes: self.duration_seconds / 60,
            duration_seconds: self.duration_seconds,
            elapsed_seconds: elapsed,
            remaining_seconds: (self.duration_seconds - elapsed).max(0),
        }
    }
}

#[derive(Default)]
pub struct RestState(Mutex<Rest>);

impl RestState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Rest> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub fn snapshot(&self) -> RestSnapshot {
        self.lock().snapshot()
    }

    pub fn is_running(&self) -> bool {
        self.lock().status == RestStatus::Running
    }

    /// Starts a break, replacing one already under way.
    pub fn begin(&self, minutes: i64) -> RestSnapshot {
        let minutes = minutes.clamp(1, 120);
        let mut rest = self.lock();
        rest.status = RestStatus::Running;
        rest.duration_seconds = minutes * 60;
        rest.started_at = now();
        rest.snapshot()
    }

    /// Back to work, whether the break ran out or was cut short.
    pub fn stop(&self) -> RestSnapshot {
        let mut rest = self.lock();
        *rest = Rest::default();
        rest.snapshot()
    }

    /// Moves a break that has run out into `Finished`, once.
    pub fn finish_if_over(&self) -> Option<RestSnapshot> {
        let mut rest = self.lock();
        if rest.status != RestStatus::Running || rest.elapsed() < rest.duration_seconds {
            return None;
        }
        rest.status = RestStatus::Finished;
        Some(rest.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_break_counts_down_from_the_minutes_asked_for() {
        let state = RestState::default();
        let snapshot = state.begin(10);
        assert_eq!(snapshot.status, RestStatus::Running);
        assert_eq!(snapshot.duration_seconds, 600);
        assert_eq!(snapshot.remaining_seconds, 600);
        assert_eq!(snapshot.minutes, 10);
    }

    #[test]
    fn an_absurd_length_is_brought_back_to_something_sensible() {
        assert_eq!(RestState::default().begin(0).duration_seconds, 60);
        assert_eq!(RestState::default().begin(600).duration_seconds, 120 * 60);
    }

    #[test]
    fn coming_back_early_clears_the_break_entirely() {
        let state = RestState::default();
        state.begin(20);
        let snapshot = state.stop();
        assert_eq!(snapshot.status, RestStatus::Idle);
        assert!(!state.is_running());
    }

    #[test]
    fn a_break_that_runs_out_finishes_once_and_stays_finished() {
        let state = RestState::default();
        {
            let mut rest = state.lock();
            rest.status = RestStatus::Running;
            rest.duration_seconds = 60;
            // Started long enough ago to have run out.
            rest.started_at = now() - 120;
        }
        let finished = state.finish_if_over().expect("should finish");
        assert_eq!(finished.status, RestStatus::Finished);
        assert_eq!(finished.remaining_seconds, 0);
        assert!(state.finish_if_over().is_none(), "only reported once");
    }

    #[test]
    fn an_idle_break_never_finishes() {
        assert!(RestState::default().finish_if_over().is_none());
    }
}
