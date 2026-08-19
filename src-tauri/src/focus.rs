//! The focus timer.
//!
//! Timer state lives in the backend rather than the webview so that closing
//! the popup — or the webview being torn down entirely — never stops a
//! running session, and so the menu-bar title stays correct either way.

use crate::db::{self, Db};
use crate::error::{Error, Result};
use crate::models::now;
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FocusStatus {
    Idle,
    Running,
    Paused,
    /// The planned duration elapsed and the session is waiting to be dismissed.
    Finished,
}

/// The shape the frontend and the tray both render from.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FocusSnapshot {
    pub status: FocusStatus,
    pub session_id: Option<i64>,
    pub task_id: Option<i64>,
    pub duration_seconds: i64,
    pub elapsed_seconds: i64,
    pub remaining_seconds: i64,
}

#[derive(Debug)]
pub struct FocusTimer {
    status: FocusStatus,
    session_id: Option<i64>,
    task_id: Option<i64>,
    duration_seconds: i64,
    /// Seconds banked by segments that have already stopped.
    banked_seconds: i64,
    /// When the current running segment began, if one is running.
    segment_started_at: Option<i64>,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            status: FocusStatus::Idle,
            session_id: None,
            task_id: None,
            duration_seconds: 0,
            banked_seconds: 0,
            segment_started_at: None,
        }
    }
}

impl FocusTimer {
    pub fn elapsed_seconds(&self) -> i64 {
        let running = match (self.status, self.segment_started_at) {
            (FocusStatus::Running, Some(started)) => (now() - started).max(0),
            _ => 0,
        };
        self.banked_seconds + running
    }

    pub fn snapshot(&self) -> FocusSnapshot {
        let elapsed = self.elapsed_seconds();
        FocusSnapshot {
            status: self.status,
            session_id: self.session_id,
            task_id: self.task_id,
            duration_seconds: self.duration_seconds,
            elapsed_seconds: elapsed,
            remaining_seconds: (self.duration_seconds - elapsed).max(0),
        }
    }

    pub fn status(&self) -> FocusStatus {
        self.status
    }

    pub fn task_id(&self) -> Option<i64> {
        self.task_id
    }

    /// True once a running timer has reached its planned duration.
    pub fn has_expired(&self) -> bool {
        self.status == FocusStatus::Running && self.elapsed_seconds() >= self.duration_seconds
    }

    pub fn start(&mut self, session_id: i64, task_id: i64, duration_seconds: i64) {
        self.status = FocusStatus::Running;
        self.session_id = Some(session_id);
        self.task_id = Some(task_id);
        self.duration_seconds = duration_seconds;
        self.banked_seconds = 0;
        self.segment_started_at = Some(now());
    }

    /// Rebuilds a session that outlived a restart, held paused so the user
    /// decides whether the time away should count.
    pub fn restore(
        &mut self,
        session_id: i64,
        task_id: i64,
        duration_seconds: i64,
        elapsed_seconds: i64,
    ) {
        self.status = FocusStatus::Paused;
        self.session_id = Some(session_id);
        self.task_id = Some(task_id);
        self.duration_seconds = duration_seconds;
        self.banked_seconds = elapsed_seconds.clamp(0, duration_seconds);
        self.segment_started_at = None;
    }

    pub fn pause(&mut self) {
        if self.status != FocusStatus::Running {
            return;
        }
        self.banked_seconds = self.elapsed_seconds();
        self.segment_started_at = None;
        self.status = FocusStatus::Paused;
    }

    pub fn resume(&mut self) {
        if self.status != FocusStatus::Paused {
            return;
        }
        self.segment_started_at = Some(now());
        self.status = FocusStatus::Running;
    }

    pub fn finish(&mut self) {
        self.banked_seconds = self.elapsed_seconds().min(self.duration_seconds);
        self.segment_started_at = None;
        self.status = FocusStatus::Finished;
    }

    /// Clears the timer and reports the session that was closed out.
    pub fn clear(&mut self) -> Option<(i64, i64)> {
        let closed = self.session_id.map(|id| (id, self.elapsed_seconds()));
        *self = Self::default();
        closed
    }
}

/// Formats remaining time the way the menu bar shows it: `24:31`, or `1:04:12`.
pub fn format_clock(seconds: i64) -> String {
    let total = seconds.max(0);
    let (hours, minutes, secs) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

/// Restores an interrupted session at launch and discards any older ones.
pub fn restore_from_db(db: &Db, timer: &mut FocusTimer) -> Result<()> {
    let conn = db.conn();
    let open = db::latest_open_focus_session(&conn)?;
    if let Some((session_id, task_id, planned_seconds, elapsed_seconds)) = open {
        timer.restore(session_id, task_id, planned_seconds, elapsed_seconds);
        db::close_stale_focus_sessions(&conn, Some(session_id))?;
    } else {
        db::close_stale_focus_sessions(&conn, None)?;
    }
    Ok(())
}

pub fn require_idle_or_same_task(timer: &FocusTimer, task_id: i64) -> Result<()> {
    match timer.status() {
        FocusStatus::Idle => Ok(()),
        _ if timer.task_id() == Some(task_id) => Ok(()),
        _ => Err(Error::invalid(
            "another task is already in focus — stop it first",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_formats_minutes_and_hours() {
        assert_eq!(format_clock(0), "0:00");
        assert_eq!(format_clock(1471), "24:31");
        assert_eq!(format_clock(3852), "1:04:12");
        assert_eq!(format_clock(-5), "0:00");
    }

    #[test]
    fn pausing_banks_elapsed_time_and_stops_the_clock() {
        let mut timer = FocusTimer::default();
        timer.start(1, 7, 1500);
        timer.pause();
        let paused = timer.snapshot();
        assert_eq!(paused.status, FocusStatus::Paused);
        assert_eq!(timer.snapshot().elapsed_seconds, paused.elapsed_seconds);
    }

    #[test]
    fn remaining_never_goes_negative() {
        let mut timer = FocusTimer::default();
        timer.start(1, 7, 10);
        timer.banked_seconds = 999;
        assert_eq!(timer.snapshot().remaining_seconds, 0);
        assert!(timer.has_expired());
    }

    #[test]
    fn restore_holds_the_session_paused() {
        let mut timer = FocusTimer::default();
        timer.restore(3, 9, 1500, 600);
        let snapshot = timer.snapshot();
        assert_eq!(snapshot.status, FocusStatus::Paused);
        assert_eq!(snapshot.elapsed_seconds, 600);
        assert_eq!(snapshot.remaining_seconds, 900);
    }

    #[test]
    fn clear_reports_the_closed_session_and_resets() {
        let mut timer = FocusTimer::default();
        timer.start(4, 2, 60);
        let closed = timer.clear();
        assert_eq!(closed.map(|(id, _)| id), Some(4));
        assert_eq!(timer.snapshot().status, FocusStatus::Idle);
    }

    #[test]
    fn a_second_task_cannot_take_focus() {
        let mut timer = FocusTimer::default();
        timer.start(1, 7, 60);
        assert!(require_idle_or_same_task(&timer, 7).is_ok());
        assert!(require_idle_or_same_task(&timer, 8).is_err());
    }
}
