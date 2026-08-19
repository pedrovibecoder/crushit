//! Menu-bar presentation: the states from the PRD's menu-bar spec.

use crate::focus::format_clock;
use tauri::AppHandle;

pub const TRAY_ID: &str = "main";

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // AwaitingApproval lands with Codex execution.
pub enum MenuBarState {
    /// Icon only.
    Idle,
    Focus { remaining_seconds: i64 },
    /// Taking a break, with what is left of it.
    Rest { remaining_seconds: i64 },
    /// Codex is working: analysing a repository, or later, writing code.
    Coding,
    AwaitingApproval,
    Done,
}

impl MenuBarState {
    /// The text shown next to the icon, or `None` for the bare icon.
    pub fn title(&self) -> Option<String> {
        match self {
            MenuBarState::Idle => None,
            MenuBarState::Focus { remaining_seconds } => Some(format_clock(*remaining_seconds)),
            MenuBarState::Rest { remaining_seconds } => {
                Some(format!("☕ {}", format_clock(*remaining_seconds)))
            }
            MenuBarState::Coding => Some("Coding…".to_string()),
            MenuBarState::AwaitingApproval => Some("Approval".to_string()),
            MenuBarState::Done => Some("Done".to_string()),
        }
    }
}

pub fn render(app: &AppHandle, state: &MenuBarState) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(state.title());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_break_reads_as_a_break_rather_than_as_more_work() {
        let title = MenuBarState::Rest {
            remaining_seconds: 300,
        }
        .title()
        .expect("a break has a title");
        assert!(title.contains("5:00"), "{title}");
        assert!(title.starts_with('☕'), "{title}");
    }

    #[test]
    fn idle_shows_no_title_and_focus_shows_the_clock() {
        assert_eq!(MenuBarState::Idle.title(), None);
        assert_eq!(
            MenuBarState::Focus {
                remaining_seconds: 1471
            }
            .title()
            .as_deref(),
            Some("24:31")
        );
    }
}
