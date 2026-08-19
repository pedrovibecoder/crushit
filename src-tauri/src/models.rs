use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// Seconds since the unix epoch. Every timestamp in the database uses this.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

macro_rules! string_enum {
    ($name:ident {
        $($(#[doc = $doc:literal])* $variant:ident => $text:literal),+ $(,)?
    }, default = $default:ident) => {
        #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($(#[doc = $doc])* #[serde(rename = $text)] $variant),+
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            pub fn parse(raw: &str) -> Result<Self> {
                match raw {
                    $($text => Ok(Self::$variant),)+
                    other => Err(Error::invalid(format!(
                        "unknown {}: {other}", stringify!($name)
                    ))),
                }
            }

            /// Rows written by an older schema fall back instead of failing the read.
            pub fn parse_lenient(raw: &str) -> Self {
                Self::parse(raw).unwrap_or(Self::$default)
            }
        }

        impl Default for $name {
            fn default() -> Self { Self::$default }
        }

        impl rusqlite::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(self.as_str().into())
            }
        }
    };
}

/// A task category the developer manages themselves. The slug is the stable id
/// stored on tasks; the label and colour are theirs to change at any time.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub slug: String,
    pub label: String,
    /// A hex colour, used for the dot that tells categories apart at a glance.
    pub color: String,
    pub position: i64,
}

/// Absent fields are left unchanged. The slug is never patched: tasks point at
/// it, so renaming a category keeps every task that used it.
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPatch {
    pub label: Option<String>,
    pub color: Option<String>,
}

/// The story-point scale. Fibonacci, because the gaps are the point: the
/// difference between 8 and 13 is a real judgement, between 8 and 9 is noise.
pub const STORY_POINTS: [i64; 6] = [1, 2, 3, 5, 8, 13];

/// Snaps any number onto the scale. Zero and below mean "not estimated".
pub fn nearest_story_points(raw: i64) -> Option<i64> {
    if raw <= 0 {
        return None;
    }
    STORY_POINTS
        .iter()
        .copied()
        .min_by_key(|point| (point - raw).abs())
}

/// Turns a label into a slug that is safe to store and compare. Anything that
/// is not a letter or digit becomes a single dash.
pub fn slugify(label: &str) -> String {
    let mut slug = String::with_capacity(label.len());
    for ch in label.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

string_enum!(TaskStatus {
    Backlog => "backlog",
    Ready => "ready",
    InProgress => "in-progress",
    NeedsReview => "needs-review",
    Completed => "completed",
    Blocked => "blocked",
}, default = Ready);

impl TaskStatus {
    pub fn is_done(&self) -> bool {
        matches!(self, TaskStatus::Completed)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub is_git: bool,
    pub branch: Option<String>,
    pub created_at: i64,
    pub last_opened_at: i64,
}

/// Read-only look at a directory the user is considering, shown before it is saved.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInspection {
    pub path: String,
    pub name: String,
    pub exists: bool,
    pub is_git: bool,
    pub branch: Option<String>,
    pub manifests: Vec<String>,
    pub already_added: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceCriterion {
    pub id: i64,
    pub task_id: i64,
    pub text: String,
    pub is_met: bool,
    pub position: i64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub goal_id: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub status: TaskStatus,
    pub position: i64,
    pub estimate_minutes: Option<i64>,
    /// Relative size on the Fibonacci scale, when one has been estimated.
    pub story_points: Option<i64>,
    pub is_ai_generated: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub criteria: Vec<AcceptanceCriterion>,
    pub files: Vec<String>,
    pub depends_on: Vec<i64>,
    /// Total seconds of focus recorded against this task.
    pub focus_seconds: i64,
    /// How many focus sessions it has taken so far.
    pub focus_sessions: i64,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub status: Option<TaskStatus>,
    pub estimate_minutes: Option<i64>,
    pub story_points: Option<i64>,
    pub goal_id: Option<i64>,
    pub is_ai_generated: Option<bool>,
    pub criteria: Option<Vec<String>>,
    pub files: Option<Vec<String>>,
}

/// One finished task, as the history graph needs it: enough to draw a day and
/// to list what was in it, without the criteria, files and focus totals a full
/// task carries.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CompletedTask {
    pub id: i64,
    pub title: String,
    pub category: String,
    pub story_points: Option<i64>,
    pub completed_at: i64,
}

/// Every field is optional: absent means "leave unchanged".
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub status: Option<TaskStatus>,
    pub estimate_minutes: Option<i64>,
    pub story_points: Option<i64>,
    pub criteria: Option<Vec<String>>,
    pub files: Option<Vec<String>>,
    pub depends_on: Option<Vec<i64>>,
}

string_enum!(Theme {
    Light => "light",
    /// GitHub's Dark Default palette.
    GithubDark => "github-dark",
}, default = Light);

string_enum!(GoalStatus {
    Draft => "draft",
    Analyzing => "analyzing",
    /// A plan came back and is waiting for the developer to accept it.
    Ready => "ready",
    Failed => "failed",
    Accepted => "accepted",
    Discarded => "discarded",
}, default = Draft);

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    /// What the developer typed.
    pub prompt: String,
    pub status: GoalStatus,
    /// Which coding agent produced this plan.
    pub agent: crate::agent::Agent,
    pub codex_thread_id: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub plan: Option<crate::plan::Plan>,
}

impl Goal {
    #[cfg(test)]
    pub fn plan_is_none(&self) -> bool {
        self.plan.is_none()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub focus_minutes: i64,
    pub show_timer_in_menu_bar: bool,
    pub hide_popup_on_blur: bool,
    pub active_project_id: Option<i64>,
    /// Which coding agent planning and execution use.
    pub agent: crate::agent::Agent,
    /// Explicit Codex binary, when discovery cannot find it.
    pub codex_path: Option<String>,
    /// Model override; `None` leaves the choice to the user's Codex config.
    pub codex_model: Option<String>,
    pub claude_path: Option<String>,
    pub claude_model: Option<String>,
    /// Which palette the interface uses.
    pub theme: Theme,
    /// Announce the moments that happen while you are looking elsewhere.
    pub notifications: bool,
    /// Sound the alarm when a session runs out or a task is finished.
    pub sounds: bool,
    pub launch_at_login: bool,
    /// False until the first run has been walked through.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            show_timer_in_menu_bar: true,
            hide_popup_on_blur: true,
            active_project_id: None,
            agent: crate::agent::Agent::Codex,
            codex_path: None,
            codex_model: None,
            claude_path: None,
            claude_model: None,
            theme: Theme::Light,
            notifications: true,
            sounds: true,
            launch_at_login: false,
            onboarded: false,
        }
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub focus_minutes: Option<i64>,
    pub show_timer_in_menu_bar: Option<bool>,
    pub hide_popup_on_blur: Option<bool>,
    pub active_project_id: Option<Option<i64>>,
    pub agent: Option<crate::agent::Agent>,
    pub codex_path: Option<Option<String>>,
    pub codex_model: Option<Option<String>>,
    pub claude_path: Option<Option<String>>,
    pub claude_model: Option<Option<String>>,
    pub theme: Option<Theme>,
    pub notifications: Option<bool>,
    pub sounds: Option<bool>,
    pub launch_at_login: Option<bool>,
    pub onboarded: Option<bool>,
}
