use crate::analysis::{AnalysisSnapshot, AnalysisState};
use crate::db::{self, Db};
use crate::error::{Error, Result};
use crate::focus::{self, FocusSnapshot, FocusStatus, FocusTimer};
use crate::rest::{RestSnapshot, RestState};
use crate::models::*;
use crate::popup;
use crate::repo;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct FocusState(pub Mutex<FocusTimer>);

impl FocusState {
    pub fn timer(&self) -> std::sync::MutexGuard<'_, FocusTimer> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Everything the popup needs for its first paint, in one round trip.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: Settings,
    pub projects: Vec<Project>,
    pub active_project: Option<Project>,
    pub tasks: Vec<Task>,
    pub categories: Vec<Category>,
    pub focus: FocusSnapshot,
    pub rest: RestSnapshot,
    pub analysis: AnalysisSnapshot,
    pub goal: Option<Goal>,
    pub execution: ExecutionSnapshot,
    pub verification: VerificationSnapshot,
}

fn notify_tasks_changed(app: &AppHandle) {
    let _ = app.emit("tasks:changed", ());
}

fn resolve_path(raw: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(Error::invalid("project path must be absolute"));
    }
    Ok(path.canonicalize().unwrap_or(path))
}

// ------------------------------------------------------------------ startup

#[tauri::command]
pub fn bootstrap(
    db: State<'_, Db>,
    focus_state: State<'_, FocusState>,
    rest: State<'_, RestState>,
    analysis: State<'_, AnalysisState>,
    execution: State<'_, ExecutionState>,
    verification: State<'_, VerificationState>,
) -> Result<Bootstrap> {
    let conn = db.conn();
    let settings = db::get_settings(&conn)?;
    let projects = db::list_projects(&conn)?;
    let active_project = match settings.active_project_id {
        Some(id) => db::find_project(&conn, id)?,
        None => None,
    };
    let (tasks, goal) = match &active_project {
        Some(project) => (
            db::list_tasks(&conn, project.id)?,
            db::latest_goal(&conn, project.id)?,
        ),
        None => (Vec::new(), None),
    };
    Ok(Bootstrap {
        verification: verification.snapshot(),
        execution: execution.snapshot(),
        analysis: analysis.snapshot(),
        goal,
        settings,
        projects,
        active_project,
        tasks,
        categories: db::list_categories(&conn)?,
        focus: focus_state.timer().snapshot(),
        rest: rest.snapshot(),
    })
}

// ----------------------------------------------------------------- projects

#[tauri::command]
pub fn list_projects(db: State<'_, Db>) -> Result<Vec<Project>> {
    db::list_projects(&db.conn())
}

/// Looks at a directory without saving it, for the "Detected" confirmation step.
#[tauri::command]
pub fn inspect_directory(db: State<'_, Db>, path: String) -> Result<ProjectInspection> {
    let path = resolve_path(&path)?;
    if !path.is_dir() {
        return Ok(ProjectInspection {
            name: repo::display_name(&path),
            path: path.to_string_lossy().to_string(),
            exists: false,
            is_git: false,
            branch: None,
            manifests: Vec::new(),
            already_added: false,
        });
    }

    // Selecting a subdirectory of a repository records the repository itself.
    let root = repo::repo_root(&path)
        .map(PathBuf::from)
        .unwrap_or_else(|| path.clone());
    let path_text = root.to_string_lossy().to_string();
    let already_added = db::find_project_by_path(&db.conn(), &path_text)?.is_some();

    Ok(ProjectInspection {
        name: repo::display_name(&root),
        path: path_text,
        exists: true,
        is_git: repo::is_git_repo(&root),
        branch: repo::current_branch(&root),
        manifests: repo::manifests(&root),
        already_added,
    })
}

#[tauri::command]
pub fn add_project(db: State<'_, Db>, path: String) -> Result<Project> {
    let path = resolve_path(&path)?;
    if !path.is_dir() {
        return Err(Error::invalid("that directory does not exist"));
    }
    let root = repo::repo_root(&path)
        .map(PathBuf::from)
        .unwrap_or_else(|| path.clone());

    let conn = db.conn();
    let project = db::upsert_project(
        &conn,
        &root.to_string_lossy(),
        &repo::display_name(&root),
        repo::is_git_repo(&root),
        repo::current_branch(&root).as_deref(),
    )?;
    db::update_settings(
        &conn,
        &SettingsPatch {
            active_project_id: Some(Some(project.id)),
            ..Default::default()
        },
    )?;
    Ok(project)
}

#[tauri::command]
pub fn set_active_project(db: State<'_, Db>, project_id: Option<i64>) -> Result<Settings> {
    db::update_settings(
        &db.conn(),
        &SettingsPatch {
            active_project_id: Some(project_id),
            ..Default::default()
        },
    )
}

#[tauri::command]
pub fn remove_project(
    app: AppHandle,
    db: State<'_, Db>,
    focus_state: State<'_, FocusState>,
    project_id: i64,
) -> Result<Vec<Project>> {
    // A running timer would otherwise point at a task that no longer exists.
    let mut timer = focus_state.timer();
    let conn = db.conn();
    if let Some(task_id) = timer.task_id() {
        let belongs = db::get_task(&conn, task_id)
            .map(|task| task.project_id == project_id)
            .unwrap_or(true);
        if belongs {
            if let Some((session_id, elapsed)) = timer.clear() {
                db::close_focus_session(&conn, session_id, elapsed, false)?;
            }
        }
    }
    db::delete_project(&conn, project_id)?;
    let settings = db::get_settings(&conn)?;
    if settings.active_project_id.is_none() {
        // `get_settings` already drops a dangling id; clear the stored row too.
        db::update_settings(
            &conn,
            &SettingsPatch {
                active_project_id: Some(None),
                ..Default::default()
            },
        )?;
    }
    let projects = db::list_projects(&conn)?;
    drop(conn);
    drop(timer);
    notify_tasks_changed(&app);
    Ok(projects)
}

/// Re-reads git state for a project that may have changed branch on disk.
#[tauri::command]
pub fn refresh_project(db: State<'_, Db>, project_id: i64) -> Result<Project> {
    let conn = db.conn();
    let project = db::find_project(&conn, project_id)?.ok_or(Error::NotFound("project"))?;
    let path = Path::new(&project.path);
    db::upsert_project(
        &conn,
        &project.path,
        &project.name,
        repo::is_git_repo(path),
        repo::current_branch(path).as_deref(),
    )
}

// --------------------------------------------------------------- categories

fn notify_categories_changed(app: &AppHandle) {
    let _ = app.emit("categories:changed", ());
}

#[tauri::command]
pub fn list_categories(db: State<'_, Db>) -> Result<Vec<Category>> {
    db::list_categories(&db.conn())
}

#[tauri::command]
pub fn create_category(
    app: AppHandle,
    db: State<'_, Db>,
    label: String,
    color: String,
) -> Result<Vec<Category>> {
    let conn = db.conn();
    db::create_category(&conn, &label, &color)?;
    let categories = db::list_categories(&conn)?;
    drop(conn);
    notify_categories_changed(&app);
    Ok(categories)
}

#[tauri::command]
pub fn update_category(
    app: AppHandle,
    db: State<'_, Db>,
    category_id: i64,
    patch: CategoryPatch,
) -> Result<Vec<Category>> {
    let conn = db.conn();
    db::update_category(&conn, category_id, &patch)?;
    let categories = db::list_categories(&conn)?;
    drop(conn);
    notify_categories_changed(&app);
    Ok(categories)
}

/// Tasks filed under the deleted category move to the one left at the top, so
/// the open task list changes too.
#[tauri::command]
pub fn delete_category(
    app: AppHandle,
    db: State<'_, Db>,
    category_id: i64,
) -> Result<Vec<Category>> {
    let categories = db::delete_category(&db.conn(), category_id)?;
    notify_categories_changed(&app);
    notify_tasks_changed(&app);
    Ok(categories)
}

/// Finished work for the history graph. `days` bounds how far back it reaches.
#[tauri::command]
pub fn completed_tasks(db: State<'_, Db>, project_id: i64, days: i64) -> Result<Vec<CompletedTask>> {
    let days = days.clamp(1, 1830);
    db::completed_since(&db.conn(), project_id, now() - days * 86_400)
}

// -------------------------------------------------------------------- tasks

#[tauri::command]
pub fn list_tasks(db: State<'_, Db>, project_id: i64) -> Result<Vec<Task>> {
    db::list_tasks(&db.conn(), project_id)
}

#[tauri::command]
pub fn create_task(db: State<'_, Db>, input: NewTask) -> Result<Task> {
    // No broadcast: the caller receives the new task and applies it directly.
    db::create_task(&db.conn(), &input)
}

#[tauri::command]
pub fn update_task(db: State<'_, Db>, task_id: i64, patch: TaskPatch) -> Result<Task> {
    db::update_task(&db.conn(), task_id, &patch)
}

#[tauri::command]
pub fn delete_task(
    app: AppHandle,
    db: State<'_, Db>,
    focus_state: State<'_, FocusState>,
    task_id: i64,
) -> Result<()> {
    let mut timer = focus_state.timer();
    let conn = db.conn();
    if timer.task_id() == Some(task_id) {
        if let Some((session_id, elapsed)) = timer.clear() {
            db::close_focus_session(&conn, session_id, elapsed, false)?;
        }
    }
    db::delete_task(&conn, task_id)?;
    drop(conn);
    drop(timer);
    notify_tasks_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn reorder_tasks(
    db: State<'_, Db>,
    project_id: i64,
    ordered_ids: Vec<i64>,
) -> Result<Vec<Task>> {
    let conn = db.conn();
    db::reorder_tasks(&conn, project_id, &ordered_ids)?;
    db::list_tasks(&conn, project_id)
}

#[tauri::command]
pub fn set_criterion_met(
    db: State<'_, Db>,
    criterion_id: i64,
    is_met: bool,
) -> Result<Task> {
    let conn = db.conn();
    let task_id = db::set_criterion_met(&conn, criterion_id, is_met)?;
    db::get_task(&conn, task_id)
}

// -------------------------------------------------------------------- focus

#[tauri::command]
pub fn focus_snapshot(focus_state: State<'_, FocusState>) -> FocusSnapshot {
    focus_state.timer().snapshot()
}

#[tauri::command]
pub fn start_focus(
    app: AppHandle,
    db: State<'_, Db>,
    focus_state: State<'_, FocusState>,
    task_id: i64,
    duration_minutes: Option<i64>,
) -> Result<FocusSnapshot> {
    let mut timer = focus_state.timer();
    focus::require_idle_or_same_task(&timer, task_id)?;

    let conn = db.conn();
    let task = db::get_task(&conn, task_id)?;

    // Resuming the same task keeps its existing session rather than opening one.
    if timer.task_id() == Some(task_id) && timer.status() == FocusStatus::Paused {
        timer.resume();
    } else {
        let minutes = duration_minutes
            .filter(|m| *m > 0)
            .unwrap_or_else(|| db::get_settings(&conn).map(|s| s.focus_minutes).unwrap_or(25));
        let session_id = db::open_focus_session(&conn, task_id, minutes * 60)?;
        timer.start(session_id, task_id, minutes * 60);
    }

    if task.status != TaskStatus::InProgress {
        db::update_task(
            &conn,
            task_id,
            &TaskPatch {
                status: Some(TaskStatus::InProgress),
                ..Default::default()
            },
        )?;
    }

    let snapshot = timer.snapshot();
    drop(conn);
    drop(timer);
    notify_tasks_changed(&app);
    let _ = app.emit("focus:changed", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn pause_focus(
    app: AppHandle,
    db: State<'_, Db>,
    focus_state: State<'_, FocusState>,
) -> Result<FocusSnapshot> {
    let mut timer = focus_state.timer();
    timer.pause();
    let snapshot = timer.snapshot();
    if let Some(session_id) = snapshot.session_id {
        db::record_focus_progress(&db.conn(), session_id, snapshot.elapsed_seconds)?;
    }
    drop(timer);
    let _ = app.emit("focus:changed", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn resume_focus(
    app: AppHandle,
    focus_state: State<'_, FocusState>,
) -> Result<FocusSnapshot> {
    let mut timer = focus_state.timer();
    timer.resume();
    let snapshot = timer.snapshot();
    drop(timer);
    let _ = app.emit("focus:changed", &snapshot);
    Ok(snapshot)
}

/// Ends the session. The task keeps whatever status it already had.
#[tauri::command]
pub fn stop_focus(
    app: AppHandle,
    db: State<'_, Db>,
    focus_state: State<'_, FocusState>,
) -> Result<FocusSnapshot> {
    let mut timer = focus_state.timer();
    let was_finished = timer.status() == FocusStatus::Finished;
    if let Some((session_id, elapsed)) = timer.clear() {
        // A session the ticker already closed on expiry must not be reopened.
        if !was_finished {
            db::close_focus_session(&db.conn(), session_id, elapsed, false)?;
        }
    }
    let snapshot = timer.snapshot();
    drop(timer);
    notify_tasks_changed(&app);
    let _ = app.emit("focus:changed", &snapshot);
    Ok(snapshot)
}

// ----------------------------------------------------------------- settings

/// A small read on how the work is going, for the desktop window.
// -------------------------------------------------------------------- break

#[tauri::command]
pub fn rest_snapshot(rest: State<'_, RestState>) -> RestSnapshot {
    rest.snapshot()
}

/// Starts a break. A focus session running underneath is paused rather than
/// left counting: the point of saying you are tired is that you have stopped.
#[tauri::command]
pub fn start_rest(
    app: AppHandle,
    focus_state: State<'_, FocusState>,
    rest: State<'_, RestState>,
    minutes: i64,
) -> Result<RestSnapshot> {
    {
        let mut timer = focus_state.timer();
        if timer.snapshot().status == FocusStatus::Running {
            timer.pause();
            let _ = app.emit("focus:changed", &timer.snapshot());
        }
    }
    let snapshot = rest.begin(minutes);
    let _ = app.emit("rest:changed", &snapshot);
    Ok(snapshot)
}

/// Ends a break, whether it ran out or was cut short.
#[tauri::command]
pub fn stop_rest(app: AppHandle, rest: State<'_, RestState>) -> RestSnapshot {
    let snapshot = rest.stop();
    let _ = app.emit("rest:changed", &snapshot);
    snapshot
}

#[tauri::command]
pub fn performance_stats(db: State<'_, Db>) -> Result<db::Stats> {
    db::stats(&db.conn())
}

#[tauri::command]
pub fn get_settings(db: State<'_, Db>) -> Result<Settings> {
    db::get_settings(&db.conn())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    patch: SettingsPatch,
) -> Result<Settings> {
    // A changed binary path invalidates whatever detection last concluded.
    if patch.codex_path.is_some() || patch.claude_path.is_some() {
        cache.clear();
    }
    if let Some(wanted) = patch.launch_at_login {
        apply_autostart(&app, wanted);
    }
    db::update_settings(&db.conn(), &patch)
}

/// Registers or removes the login item. Reported back through the settings
/// read, so a refusal by the system shows up rather than being assumed.
pub fn apply_autostart(app: &AppHandle, wanted: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let _ = if wanted {
        manager.enable()
    } else {
        manager.disable()
    };
}

// ------------------------------------------------------------------- window

#[tauri::command]
pub fn hide_popup(app: AppHandle) -> Result<()> {
    popup::hide(&app)
}

/// Raises the desktop window — the "open in a real window" action.
#[tauri::command]
pub fn open_desktop_window(app: AppHandle, view: Option<String>) -> Result<()> {
    // Opening the window is a deliberate switch of surface, so the popup gets
    // out of the way rather than hovering over it.
    let _ = popup::hide(&app);
    crate::desktop::show(&app)?;
    // The desktop window keeps its own idea of which screen it is on, so being
    // sent somewhere has to be asked for rather than assumed.
    if let Some(view) = view.filter(|view| !view.trim().is_empty()) {
        let _ = app.emit_to(crate::desktop::WINDOW_LABEL, "desktop:navigate", view);
    }
    Ok(())
}

/// Returns the application to the menu bar without quitting.
#[tauri::command]
pub fn hide_desktop_window(app: AppHandle) -> Result<()> {
    crate::desktop::hide(&app)
}

#[tauri::command]
pub fn desktop_window_open(app: AppHandle) -> bool {
    crate::desktop::is_open(&app)
}

/// Grows or shrinks the popup to match the rendered content height.
///
/// The ceiling comes from the screen rather than a constant: the panel clips
/// its own overflow, so a window shorter than its content puts the footer out
/// of reach entirely.
#[tauri::command]
pub fn resize_popup(app: AppHandle, height: f64) -> Result<()> {
    let win = popup::window(&app)?;
    let scale = win.scale_factor()?;
    let current = win.outer_size()?.to_logical::<f64>(scale);
    let target = height.clamp(240.0, popup::max_height(&win));
    if (current.height - target).abs() < 1.0 {
        return Ok(());
    }
    win.set_size(tauri::LogicalSize::new(current.width, target))?;
    // macOS keeps the bottom edge fixed when resizing, so put the top back.
    popup::reapply_anchor(&app, &win)?;
    Ok(())
}

/// The hotkey that opens the popup when the menu-bar icon is out of reach.
#[tauri::command]
pub fn popup_shortcut(state: State<'_, crate::shortcut::ShortcutState>) -> crate::shortcut::ShortcutInfo {
    state.get()
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

// -------------------------------------------------------------------- agent

use crate::agent::{self, Agent, AgentStatus};
use crate::codex::CodexClient;
use crate::plan::AnalysisEvent;

/// A model the agent can be pointed at, for the Settings picker.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentModel {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

/// Claude Code has no model-list command, so its well-known aliases are used.
fn claude_models() -> Vec<AgentModel> {
    [
        ("opus", "Opus", "Deepest reasoning"),
        ("sonnet", "Sonnet", "Balanced for everyday work"),
        ("haiku", "Haiku", "Fastest"),
    ]
    .into_iter()
    .map(|(id, name, description)| AgentModel {
        id: id.to_string(),
        display_name: name.to_string(),
        description: Some(description.to_string()),
        is_default: false,
    })
    .collect()
}

fn configured_path(settings: &Settings, agent: Agent) -> Option<String> {
    match agent {
        Agent::Codex => settings.codex_path.clone(),
        Agent::ClaudeCode => settings.claude_path.clone(),
    }
}

fn configured_model(settings: &Settings, agent: Agent) -> Option<String> {
    match agent {
        Agent::Codex => settings.codex_model.clone(),
        Agent::ClaudeCode => settings.claude_model.clone(),
    }
}

fn agent_binary(db: &Db, cache: &agent::StatusCache, agent: Agent) -> Result<std::path::PathBuf> {
    let settings = db::get_settings(&db.conn())?;
    let configured = configured_path(&settings, agent);
    // Resolving from scratch spawns a login shell and a `--version` per
    // candidate; reuse what the last status probe already worked out.
    cache
        .cached_path(agent, configured.as_deref())
        .or_else(|| agent::find_binary(agent, configured.as_deref()))
        .ok_or_else(|| {
            Error::invalid(format!(
                "{} not found. Install it, or set its path in Settings.",
                agent.label()
            ))
        })
}

/// Reports what Crushit found for one agent: binary, version, sign-in state.
///
/// Served from cache unless `force` is set, because probing costs well over a
/// second and every screen that shows agent state would otherwise pay it.
#[tauri::command]
pub fn agent_status(
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    agent: Agent,
    force: Option<bool>,
) -> Result<AgentStatus> {
    let settings = db::get_settings(&db.conn())?;
    let configured = configured_path(&settings, agent);
    Ok(cache.get_or_probe(agent, configured.as_deref(), force.unwrap_or(false), || {
        agent::status(agent, configured.as_deref())
    }))
}

#[tauri::command]
pub fn agent_models(
    app: AppHandle,
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    client: State<'_, CodexClient>,
    agent: Agent,
) -> Result<Vec<AgentModel>> {
    if agent == Agent::ClaudeCode {
        return Ok(claude_models());
    }

    let binary = agent_binary(&db, &cache, agent)?;
    client.ensure_started(&binary, &app.package_info().version.to_string())?;
    let response = client.request(
        "model/list",
        serde_json::json!({}),
        std::time::Duration::from_secs(30),
    )?;

    let rows = response
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(rows
        .into_iter()
        .filter(|model| model.get("hidden").and_then(|v| v.as_bool()) != Some(true))
        .filter_map(|model| {
            let id = model.get("id")?.as_str()?.to_string();
            Some(AgentModel {
                display_name: model
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                description: model
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                is_default: model.get("isDefault").and_then(|v| v.as_bool()) == Some(true),
                id,
            })
        })
        .collect())
}

// -------------------------------------------------------------------- goals

#[tauri::command]
pub fn latest_goal(db: State<'_, Db>, project_id: i64) -> Result<Option<Goal>> {
    db::latest_goal(&db.conn(), project_id)
}

#[tauri::command]
pub fn analysis_snapshot(analysis: State<'_, AnalysisState>) -> AnalysisSnapshot {
    analysis.snapshot()
}

/// Starts a read-only planning run with the selected agent. Returns as soon as
/// the work is handed to a worker thread, so the popup can be closed while the
/// agent reads the repository.
#[tauri::command]
pub fn start_analysis(
    app: AppHandle,
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    analysis: State<'_, AnalysisState>,
    project_id: i64,
    prompt: String,
) -> Result<Goal> {
    if analysis.is_running() {
        return Err(Error::invalid("an analysis is already running"));
    }

    let (goal, project, chosen, model, categories) = {
        let conn = db.conn();
        let settings = db::get_settings(&conn)?;
        let chosen = settings.agent;
        let project = db::find_project(&conn, project_id)?.ok_or(Error::NotFound("project"))?;
        let goal = db::create_goal(&conn, project_id, &prompt, chosen)?;
        let goal = db::set_goal_status(&conn, goal.id, GoalStatus::Analyzing)?;
        // The agent files its tasks under the developer's own categories, so
        // the list it may choose from is read at the moment the run starts.
        let categories = db::category_slugs(&conn)?;
        (goal, project, chosen, configured_model(&settings, chosen), categories)
    };
    let request = crate::plan::PlanRequest::goal(&goal.prompt, categories);
    let binary = agent_binary(&db, &cache, chosen)?;

    let snapshot = analysis.begin(goal.id, project_id, goal.title.clone());
    let _ = app.emit("codex:analysis", &snapshot);

    spawn_analysis_worker(app, goal.clone(), project.path, chosen, binary, model, request);
    Ok(goal)
}

/// Turns a file dropped on the window into a plan waiting to be accepted.
///
/// This is the same machinery as `start_analysis` — one read-only agent turn
/// producing a plan — pointed at a staged copy of the dropped file instead of
/// at the repository.
#[tauri::command]
pub fn import_tasks(
    app: AppHandle,
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    analysis: State<'_, AnalysisState>,
    project_id: i64,
    path: String,
) -> Result<Goal> {
    if analysis.is_running() {
        return Err(Error::invalid("an analysis is already running"));
    }

    let source = PathBuf::from(&path);
    let name = crate::import::display_name(&source);
    let staged = crate::import::stage(
        &source,
        &app.path()
            .app_data_dir()
            .map_err(|_| Error::invalid("could not find somewhere to stage the file"))?,
    )?;
    // The turn is rooted at the staging directory, which holds the copy and
    // nothing else, so a read-only agent can reach the file and no further.
    let work_dir = staged
        .parent()
        .ok_or_else(|| Error::invalid("could not stage that file"))?
        .to_string_lossy()
        .to_string();

    let (goal, chosen, model, categories) = {
        let conn = db.conn();
        let settings = db::get_settings(&conn)?;
        let chosen = settings.agent;
        db::find_project(&conn, project_id)?.ok_or(Error::NotFound("project"))?;
        let goal = db::create_goal(&conn, project_id, &format!("Import tasks from {name}"), chosen)?;
        let goal = db::set_goal_status(&conn, goal.id, GoalStatus::Analyzing)?;
        let categories = db::category_slugs(&conn)?;
        (goal, chosen, configured_model(&settings, chosen), categories)
    };
    let binary = agent_binary(&db, &cache, chosen)?;
    let request = crate::import::request(
        &staged.file_name().unwrap_or_default().to_string_lossy(),
        categories,
    );

    let snapshot = analysis.begin(goal.id, project_id, goal.title.clone());
    let _ = app.emit("codex:analysis", &snapshot);

    spawn_analysis_worker(app, goal.clone(), work_dir, chosen, binary, model, request);
    Ok(goal)
}

fn spawn_analysis_worker(
    app: AppHandle,
    goal: Goal,
    project_path: String,
    chosen: Agent,
    binary: std::path::PathBuf,
    model: Option<String>,
    request: crate::plan::PlanRequest,
) {
    std::thread::spawn(move || {
        let analysis = app.state::<AnalysisState>();
        let database = app.state::<Db>();

        // If this worker unwinds, the guard resolves the run on the way out so
        // the UI and the menu bar cannot be left showing an analysis forever.
        let mut guard = RunGuard {
            analysis: &analysis,
            app: &app,
            resolved: false,
        };

        let mut on_event = |event: AnalysisEvent| {
            let snapshot = match event {
                AnalysisEvent::StepStarted { id, label } => analysis.step_started(id, label),
                AnalysisEvent::StepFinished { id } => analysis.step_finished(&id),
            };
            let _ = app.emit("codex:analysis", &snapshot);
        };
        let cancel_flag = analysis.cancel_flag();
        let is_cancelled = || analysis.is_cancelled();

        let outcome = match chosen {
            Agent::Codex => {
                let client = app.state::<CodexClient>();
                let version = app.package_info().version.to_string();
                client.ensure_started(&binary, &version).and_then(|()| {
                    crate::codex::planning::run_analysis(
                        &client,
                        &project_path,
                        &request,
                        model.as_deref(),
                        &mut on_event,
                        is_cancelled,
                    )
                })
            }
            Agent::ClaudeCode => crate::claude::planning::run_analysis(
                &binary,
                &project_path,
                &request,
                model.as_deref(),
                &mut on_event,
                cancel_flag,
            ),
        };

        // A cancelled run is not a failure: put the goal back to a draft and
        // leave the screen clean.
        if analysis.is_cancelled() {
            guard.resolved = true;
            let _ = db::set_goal_status(&database.conn(), goal.id, GoalStatus::Draft);
            let snapshot = analysis.clear();
            let _ = app.emit("codex:analysis", &snapshot);
            let _ = app.emit("goals:changed", ());
            return;
        }

        let snapshot = match outcome {
            Ok(result) => {
                let tasks = result.plan.tasks.len();
                let stored =
                    db::set_goal_plan(&database.conn(), goal.id, &result.thread_id, &result.plan);
                match stored {
                    Ok(_) => {
                        crate::notify::send(
                            &app,
                            crate::notify::Event::PlanReady {
                                goal: &goal.title,
                                tasks,
                            },
                        );
                        analysis.succeeded()
                    }
                    Err(error) => analysis.failed(error.to_string()),
                }
            }
            Err(error) => {
                let message = error.to_string();
                let _ = db::set_goal_failed(&database.conn(), goal.id, &message);
                analysis.failed(message)
            }
        };
        guard.resolved = true;
        let _ = app.emit("codex:analysis", &snapshot);
        let _ = app.emit("goals:changed", ());
    });
}

/// Marks an unfinished run failed when the worker leaves by any path.
struct RunGuard<'a> {
    analysis: &'a AnalysisState,
    app: &'a AppHandle,
    resolved: bool,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if self.resolved {
            return;
        }
        let snapshot = self.analysis.failed("The analysis stopped unexpectedly.".into());
        let _ = self.app.emit("codex:analysis", &snapshot);
        let _ = self.app.emit("goals:changed", ());
    }
}

#[tauri::command]
pub fn cancel_analysis(app: AppHandle, analysis: State<'_, AnalysisState>) -> AnalysisSnapshot {
    if analysis.is_running() {
        analysis.request_cancel();
        return analysis.snapshot();
    }
    // Nothing running: just clear a finished or failed run off the screen.
    let snapshot = analysis.clear();
    let _ = app.emit("codex:analysis", &snapshot);
    snapshot
}

/// Writes the plan out as tasks. This is the confirmation step the PRD requires
/// before anything the agent proposed becomes real work.
#[tauri::command]
pub fn accept_plan(
    app: AppHandle,
    db: State<'_, Db>,
    analysis: State<'_, AnalysisState>,
    goal_id: i64,
) -> Result<Vec<Task>> {
    let conn = db.conn();
    let goal = db::get_goal(&conn, goal_id)?;
    let tasks = db::create_tasks_from_plan(&conn, &goal)?;
    drop(conn);

    let snapshot = analysis.clear();
    let _ = app.emit("codex:analysis", &snapshot);
    notify_tasks_changed(&app);
    let _ = app.emit("goals:changed", ());
    Ok(tasks)
}

#[tauri::command]
pub fn discard_plan(
    app: AppHandle,
    db: State<'_, Db>,
    analysis: State<'_, AnalysisState>,
    goal_id: i64,
) -> Result<()> {
    db::delete_goal(&db.conn(), goal_id)?;
    let snapshot = analysis.clear();
    let _ = app.emit("codex:analysis", &snapshot);
    let _ = app.emit("goals:changed", ());
    Ok(())
}

// ---------------------------------------------------------------- execution

use crate::execution::{ExecutionSnapshot, ExecutionState};
use crate::run::{RunEvent, RunOutcome};

#[tauri::command]
pub fn execution_snapshot(execution: State<'_, ExecutionState>) -> ExecutionSnapshot {
    execution.snapshot()
}

/// Whether this task already has a conversation with the selected agent, which
/// is what makes "Continue" meaningful rather than a second cold start.
#[tauri::command]
pub fn task_has_thread(db: State<'_, Db>, task_id: i64) -> Result<bool> {
    let conn = db.conn();
    let agent = db::get_settings(&conn)?.agent;
    Ok(db::task_thread(&conn, task_id, agent)?.is_some())
}

/// Hands a task to the selected agent **with write access**.
///
/// The confirmation the PRD requires happens in the UI before this is called;
/// reaching here is the explicit action that enables writes.
#[tauri::command]
pub fn start_execution(
    app: AppHandle,
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    execution: State<'_, ExecutionState>,
    task_id: i64,
    resume: Option<bool>,
) -> Result<Task> {
    if execution.is_active() {
        return Err(Error::invalid("a task is already running"));
    }

    let (task, project, chosen, model, thread) = {
        let conn = db.conn();
        let settings = db::get_settings(&conn)?;
        let chosen = settings.agent;
        let task = db::get_task(&conn, task_id)?;
        let project =
            db::find_project(&conn, task.project_id)?.ok_or(Error::NotFound("project"))?;
        // Continuing reuses the task's own conversation; starting fresh does not.
        let thread = match resume.unwrap_or(false) {
            true => db::task_thread(&conn, task_id, chosen)?,
            false => None,
        };
        let task = db::update_task(
            &conn,
            task_id,
            &TaskPatch {
                status: Some(TaskStatus::InProgress),
                ..Default::default()
            },
        )?;
        (task, project, chosen, configured_model(&settings, chosen), thread)
    };
    let binary = agent_binary(&db, &cache, chosen)?;

    let snapshot = execution.begin(task.id, task.title.clone(), chosen);
    let _ = app.emit("execution:changed", &snapshot);
    notify_tasks_changed(&app);

    spawn_execution_worker(app, task.clone(), project.path, chosen, binary, model, thread);
    Ok(task)
}

fn spawn_execution_worker(
    app: AppHandle,
    task: Task,
    project_path: String,
    chosen: Agent,
    binary: std::path::PathBuf,
    model: Option<String>,
    resume_thread: Option<String>,
) {
    std::thread::spawn(move || {
        let execution = app.state::<ExecutionState>();
        let database = app.state::<Db>();
        let mut guard = ExecutionGuard {
            execution: &execution,
            app: &app,
            resolved: false,
        };

        let mut on_event = |event: RunEvent| {
            let snapshot = match event {
                RunEvent::Thread(id) => {
                    let _ = db::save_task_thread(
                        &database.conn(),
                        task.id,
                        chosen,
                        &id,
                        "running",
                    );
                    execution.set_thread(id)
                }
                RunEvent::Started { id, label, kind } => execution.step_started(id, label, kind),
                RunEvent::Finished { id } => execution.step_finished(&id),
                RunEvent::FileChanged(path) => execution.file_changed(path),
                RunEvent::Approval(request) => {
                    // Blocked work is the one thing that cannot wait to be seen.
                    crate::notify::send(
                        &app,
                        crate::notify::Event::ApprovalNeeded { task: &task.title },
                    );
                    execution.awaiting(request)
                }
                RunEvent::ApprovalResolved => execution.approval_resolved(),
                // The closing message is kept for the summary, not the list.
                RunEvent::Message(_) => execution.snapshot(),
            };
            let _ = app.emit("execution:changed", &snapshot);
        };

        let outcome: Result<RunOutcome> = match chosen {
            Agent::Codex => {
                let client = app.state::<CodexClient>();
                let version = app.package_info().version.to_string();
                client.ensure_started(&binary, &version).and_then(|()| {
                    crate::codex::execution::run_task(
                        &client,
                        &project_path,
                        &task,
                        resume_thread.as_deref(),
                        model.as_deref(),
                        &mut on_event,
                        || execution.is_cancelled(),
                        || execution.is_awaiting_approval(),
                    )
                })
            }
            Agent::ClaudeCode => crate::claude::execution::run_task(
                &binary,
                &project_path,
                &task,
                resume_thread.as_deref(),
                model.as_deref(),
                &mut on_event,
                execution.cancel_flag(),
            ),
        };

        let cancelled = execution.is_cancelled();
        let snapshot = match outcome {
            Ok(result) => {
                let conn = database.conn();
                if !result.thread_id.is_empty() {
                    let status = if cancelled { "stopped" } else { "finished" };
                    let _ =
                        db::save_task_thread(&conn, task.id, chosen, &result.thread_id, status);
                }
                // Stopping leaves the task in progress; finishing asks for review.
                // Neither completes it — that stays the developer's call.
                if !cancelled {
                    let _ = db::update_task(
                        &conn,
                        task.id,
                        &TaskPatch {
                            status: Some(TaskStatus::NeedsReview),
                            ..Default::default()
                        },
                    );
                }
                drop(conn);
                execution.succeeded(result.summary)
            }
            Err(error) => execution.failed(error.to_string()),
        };

        if !cancelled {
            let event = match snapshot.error.is_some() {
                true => crate::notify::Event::RunFailed { task: &task.title },
                false => crate::notify::Event::RunFinished {
                    task: &task.title,
                    changed: snapshot.changed_files.len(),
                },
            };
            crate::notify::send(&app, event);
        }

        guard.resolved = true;
        let _ = app.emit("execution:changed", &snapshot);
        notify_tasks_changed(&app);
    });
}

/// Marks an unfinished run failed when the worker leaves by any path.
struct ExecutionGuard<'a> {
    execution: &'a ExecutionState,
    app: &'a AppHandle,
    resolved: bool,
}

impl Drop for ExecutionGuard<'_> {
    fn drop(&mut self) {
        if self.resolved {
            return;
        }
        let snapshot = self.execution.failed("The run stopped unexpectedly.".into());
        let _ = self.app.emit("execution:changed", &snapshot);
    }
}

/// Interrupts the run. The conversation and the code already written are kept.
#[tauri::command]
pub fn stop_execution(
    app: AppHandle,
    client: State<'_, CodexClient>,
    execution: State<'_, ExecutionState>,
) -> ExecutionSnapshot {
    if execution.is_active() {
        // Nothing may be left waiting on a person who has walked away.
        client.decline_all_pending();
        execution.request_cancel();
        return execution.snapshot();
    }
    let snapshot = execution.clear();
    let _ = app.emit("execution:changed", &snapshot);
    snapshot
}

/// Answers the agent's permission request. Only ever what the user chose.
#[tauri::command]
pub fn respond_to_approval(
    app: AppHandle,
    client: State<'_, CodexClient>,
    execution: State<'_, ExecutionState>,
    approve: bool,
) -> Result<ExecutionSnapshot> {
    let approval = execution
        .snapshot()
        .approval
        .ok_or_else(|| Error::invalid("nothing is waiting for approval"))?;
    client.answer_approval(approval.id, if approve { "accept" } else { "decline" })?;
    let snapshot = execution.approval_resolved();
    let _ = app.emit("execution:changed", &snapshot);
    Ok(snapshot)
}

/// Dismisses a finished or failed run from the screen.
#[tauri::command]
pub fn clear_execution(app: AppHandle, execution: State<'_, ExecutionState>) -> ExecutionSnapshot {
    let snapshot = execution.clear();
    let _ = app.emit("execution:changed", &snapshot);
    snapshot
}

// ------------------------------------------------------- changes and review

use crate::repo::RepoChanges;
use crate::verify::{self, VerificationSnapshot, VerificationState};

/// What the working tree has changed, for the review step.
#[tauri::command]
pub fn repo_changes(db: State<'_, Db>) -> Result<RepoChanges> {
    let conn = db.conn();
    let settings = db::get_settings(&conn)?;
    let Some(project_id) = settings.active_project_id else {
        return Ok(RepoChanges::default());
    };
    let project = db::find_project(&conn, project_id)?.ok_or(Error::NotFound("project"))?;
    drop(conn);
    Ok(crate::repo::changes(std::path::Path::new(&project.path)))
}

/// The unified diff for one changed file.
#[tauri::command]
pub fn file_diff(db: State<'_, Db>, path: String) -> Result<String> {
    let conn = db.conn();
    let settings = db::get_settings(&conn)?;
    let project_id = settings
        .active_project_id
        .ok_or_else(|| Error::invalid("no project is selected"))?;
    let project = db::find_project(&conn, project_id)?.ok_or(Error::NotFound("project"))?;
    drop(conn);
    crate::repo::file_diff(std::path::Path::new(&project.path), &path)
        .ok_or_else(|| Error::invalid("that file has no diff to show"))
}

#[tauri::command]
pub fn verification_snapshot(state: State<'_, VerificationState>) -> VerificationSnapshot {
    state.snapshot()
}

/// Checks the work against the task's criteria. Read-only, like planning.
/// Asks the agent for acceptance criteria for a task written by hand.
///
/// Blocking, unlike the other agent work: it is one short read-only question,
/// and the screen that asked has nothing to show until the answer arrives.
#[tauri::command]
pub async fn suggest_criteria(
    app: AppHandle,
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    task_id: i64,
) -> Result<Vec<String>> {
    let (task, project, chosen, model) = {
        let conn = db.conn();
        let settings = db::get_settings(&conn)?;
        let task = db::get_task(&conn, task_id)?;
        let project =
            db::find_project(&conn, task.project_id)?.ok_or(Error::NotFound("project"))?;
        (
            task,
            project,
            settings.agent,
            configured_model(&settings, settings.agent),
        )
    };
    let binary = agent_binary(&db, &cache, chosen)?;

    let answer = match chosen {
        Agent::Codex => {
            let client = app.state::<CodexClient>();
            client.ensure_started(&binary, &app.package_info().version.to_string())?;
            crate::codex::planning::run_read_only_turn(
                &client,
                &project.path,
                &crate::criteria::prompt(&task),
                crate::criteria::output_schema(),
                model.as_deref(),
                |_| {},
                || false,
                "Codex took too long to suggest acceptance criteria.",
            )
        }
        Agent::ClaudeCode => crate::claude::planning::run_read_only(
            &binary,
            &project.path,
            &format!(
                "{}{}",
                crate::criteria::prompt(&task),
                crate::criteria::json_instruction()
            ),
            model.as_deref(),
            |_| {},
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            "Claude Code took too long to suggest acceptance criteria.",
        ),
    };

    crate::criteria::parse(&answer?.0)
}

#[tauri::command]
pub fn start_verification(
    app: AppHandle,
    db: State<'_, Db>,
    cache: State<'_, agent::StatusCache>,
    state: State<'_, VerificationState>,
    task_id: i64,
) -> Result<VerificationSnapshot> {
    if state.is_running() {
        return Err(Error::invalid("a verification is already running"));
    }

    let (task, project, chosen, model) = {
        let conn = db.conn();
        let settings = db::get_settings(&conn)?;
        let task = db::get_task(&conn, task_id)?;
        if task.criteria.is_empty() {
            return Err(Error::invalid(
                "add at least one acceptance criterion before verifying",
            ));
        }
        let project =
            db::find_project(&conn, task.project_id)?.ok_or(Error::NotFound("project"))?;
        (
            task,
            project,
            settings.agent,
            configured_model(&settings, settings.agent),
        )
    };
    let binary = agent_binary(&db, &cache, chosen)?;

    let snapshot = state.begin(task.id, task.title.clone());
    let _ = app.emit("verification:changed", &snapshot);
    spawn_verification_worker(app, task, project.path, chosen, binary, model);
    Ok(snapshot)
}

fn spawn_verification_worker(
    app: AppHandle,
    task: Task,
    project_path: String,
    chosen: Agent,
    binary: std::path::PathBuf,
    model: Option<String>,
) {
    std::thread::spawn(move || {
        let state = app.state::<VerificationState>();
        let database = app.state::<Db>();
        let mut guard = VerificationGuard {
            state: &state,
            app: &app,
            resolved: false,
        };

        let mut on_event = |event: AnalysisEvent| {
            let snapshot = match event {
                AnalysisEvent::StepStarted { id, label } => state.step_started(id, label),
                AnalysisEvent::StepFinished { id } => state.step_finished(&id),
            };
            let _ = app.emit("verification:changed", &snapshot);
        };

        let answer: Result<(String, String)> = match chosen {
            Agent::Codex => {
                let client = app.state::<CodexClient>();
                let version = app.package_info().version.to_string();
                client.ensure_started(&binary, &version).and_then(|()| {
                    crate::codex::planning::run_read_only_turn(
                        &client,
                        &project_path,
                        &verify::prompt(&task),
                        verify::output_schema(),
                        model.as_deref(),
                        &mut on_event,
                        || state.is_cancelled(),
                        "Codex took too long to verify this task.",
                    )
                })
            }
            Agent::ClaudeCode => crate::claude::planning::run_read_only(
                &binary,
                &project_path,
                &format!("{}{}", verify::prompt(&task), verify::json_instruction()),
                model.as_deref(),
                &mut on_event,
                state.cancel_flag(),
                "Claude Code took too long to verify this task.",
            ),
        };

        if state.is_cancelled() {
            guard.resolved = true;
            let snapshot = state.clear();
            let _ = app.emit("verification:changed", &snapshot);
            return;
        }

        let snapshot = match answer.and_then(|(text, _)| verify::parse(&text, &task)) {
            Ok(result) => {
                let _ = db::save_verification(&database.conn(), task.id, &result);
                state.succeeded(result)
            }
            Err(error) => state.failed(error.to_string()),
        };
        guard.resolved = true;
        let _ = app.emit("verification:changed", &snapshot);
    });
}

struct VerificationGuard<'a> {
    state: &'a VerificationState,
    app: &'a AppHandle,
    resolved: bool,
}

impl Drop for VerificationGuard<'_> {
    fn drop(&mut self) {
        if self.resolved {
            return;
        }
        let snapshot = self.state.failed("Verification stopped unexpectedly.".into());
        let _ = self.app.emit("verification:changed", &snapshot);
    }
}

#[tauri::command]
pub fn cancel_verification(
    app: AppHandle,
    state: State<'_, VerificationState>,
) -> VerificationSnapshot {
    if state.is_running() {
        state.request_cancel();
        return state.snapshot();
    }
    let snapshot = state.clear();
    let _ = app.emit("verification:changed", &snapshot);
    snapshot
}
