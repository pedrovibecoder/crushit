pub mod agent;
mod analysis;
pub mod block;
pub mod claude;
pub mod codex;
mod commands;
pub mod criteria;
mod db;
pub mod desktop;
mod error;
pub mod execution;
mod focus;
pub mod import;
pub mod models;
mod notify;
pub mod plan;
mod popup;
pub mod rest;
pub mod run;
pub mod verify;
pub mod repo;
mod shortcut;
pub mod slack;
mod sound;
mod tray;

use analysis::AnalysisState;
use execution::ExecutionState;
use verify::VerificationState;
use codex::CodexClient;
use commands::FocusState;
use db::Db;
use focus::{FocusSnapshot, FocusStatus, FocusTimer};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tray::MenuBarState;

/// How often the timer thread re-evaluates the clock.
const TICK: Duration = Duration::from_millis(250);
/// How long the menu bar keeps showing "Done" after a session ends.
const DONE_LINGER: Duration = Duration::from_secs(8);
/// A running session writes its progress this often, so a crash loses little.
const PERSIST_EVERY: Duration = Duration::from_secs(15);
/// Longer than any analysis is allowed to take, with room to spare. Past this
/// a run is presumed dead so the menu bar recovers on its own.
const ANALYSIS_STALL_SECONDS: i64 = 15 * 60;
/// The same backstop for an implementation run, which is allowed longer.
const RUN_STALL_SECONDS: i64 = 35 * 60;
/// The tray reads one setting; re-querying it four times a second would hold
/// the database lock against the UI for no benefit.
const SETTINGS_REFRESH: Duration = Duration::from_secs(2);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, pressed, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed
                        && shortcut::matches(pressed)
                    {
                        let _ = popup::toggle(app, None);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::list_projects,
            commands::inspect_directory,
            commands::add_project,
            commands::set_active_project,
            commands::remove_project,
            commands::refresh_project,
            commands::list_categories,
            commands::create_category,
            commands::update_category,
            commands::delete_category,
            commands::completed_tasks,
            commands::list_tasks,
            commands::create_task,
            commands::update_task,
            commands::delete_task,
            commands::reorder_tasks,
            commands::set_criterion_met,
            commands::focus_snapshot,
            commands::rest_snapshot,
            commands::start_rest,
            commands::stop_rest,
            commands::start_focus,
            commands::pause_focus,
            commands::resume_focus,
            commands::stop_focus,
            commands::performance_stats,
            commands::get_settings,
            commands::update_settings,
            commands::check_focus_block,
            commands::hide_popup,
            commands::open_desktop_window,
            commands::hide_desktop_window,
            commands::desktop_window_open,
            commands::resize_popup,
            commands::quit_app,
            commands::popup_shortcut,
            commands::connect_slack,
            commands::slack_account,
            commands::slack_waiting,
            commands::draft_slack_reply,
            commands::send_slack_reply,
            commands::agent_status,
            commands::agent_models,
            commands::latest_goal,
            commands::analysis_snapshot,
            commands::start_analysis,
            commands::import_tasks,
            commands::cancel_analysis,
            commands::accept_plan,
            commands::discard_plan,
            commands::execution_snapshot,
            commands::task_has_thread,
            commands::repo_changes,
            commands::file_diff,
            commands::verification_snapshot,
            commands::suggest_criteria,
            commands::start_verification,
            commands::cancel_verification,
            commands::start_execution,
            commands::stop_execution,
            commands::respond_to_approval,
            commands::clear_execution,
        ])
        .setup(|app| {
            // The app lives in the menu bar: no dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            // Still the old name: renaming the app must not lose the projects
            // and tasks already in it.
            let database = Db::open(&data_dir.join("blitzit.sqlite3"))?;

            let mut timer = FocusTimer::default();
            focus::restore_from_db(&database, &mut timer)?;

            app.manage(database);
            app.manage(FocusState(Mutex::new(timer)));
            app.manage(popup::PopupAnchor::default());
            app.manage(rest::RestState::default());
            app.manage(CodexClient::default());
            app.manage(AnalysisState::default());
            app.manage(shortcut::ShortcutState::default());
            app.manage(agent::StatusCache::default());
            app.manage(ExecutionState::default());
            app.manage(VerificationState::default());
            app.manage(block::BlockState::default());

            build_tray(app.handle())?;
            shortcut::register(app.handle());
            popup::prewarm(app.handle());
            desktop::restore(app.handle());
            spawn_timer_thread(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            let is_desktop = window.label() == desktop::WINDOW_LABEL;
            match event {
                // Closing either surface returns to the menu bar rather than
                // quitting, and never stops a running task.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    if is_desktop {
                        let _ = desktop::hide(window.app_handle());
                    } else {
                        let _ = window.hide();
                    }
                }
                // Only the popup behaves like a menu-bar extra.
                WindowEvent::Focused(false) => {
                    if !is_desktop && should_hide_on_blur(window.app_handle()) {
                        let _ = window.hide();
                    }
                }
                // Remember where the developer put the window.
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    if is_desktop && desktop::is_open(window.app_handle()) {
                        desktop::remember(window.app_handle(), true);
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start Crushit");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            _app.state::<CodexClient>().shutdown();
        }
        if let RunEvent::ExitRequested { code, api, .. } = event {
            // Only an explicit Quit exits; window closes leave the tray running.
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}

fn should_hide_on_blur(app: &AppHandle) -> bool {
    // Keeping the popup open in development leaves devtools usable.
    if cfg!(debug_assertions) {
        return false;
    }
    app.try_state::<Db>()
        .and_then(|db| db::get_settings(&db.conn()).ok())
        .map(|settings| settings.hide_popup_on_blur)
        .unwrap_or(true)
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Crushit", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Crushit", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    let builder = TrayIconBuilder::with_id(tray::TRAY_ID)
        .icon(icon)
        .tooltip("Crushit")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                let _ = popup::show(app, None);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let _ = popup::toggle(tray.app_handle(), Some(rect));
            }
        });

    #[cfg(target_os = "macos")]
    let builder = builder.icon_as_template(true);

    builder.build(app)?;
    Ok(())
}

/// Advances the timer one tick, closing out a session that has just expired.
fn advance_timer(app: &AppHandle) -> (FocusSnapshot, bool) {
    let focus_state = app.state::<FocusState>();
    let mut timer = focus_state.timer();

    let mut just_finished = false;
    if timer.has_expired() {
        timer.finish();
        just_finished = true;
        let snapshot = timer.snapshot();
        if let Some(session_id) = snapshot.session_id {
            let database = app.state::<Db>();
            let _ = db::close_focus_session(
                &database.conn(),
                session_id,
                snapshot.elapsed_seconds,
                true,
            );
        }
    }
    (timer.snapshot(), just_finished)
}

fn persist_progress(app: &AppHandle, snapshot: &FocusSnapshot) {
    if let (FocusStatus::Running, Some(session_id)) = (snapshot.status, snapshot.session_id) {
        let database = app.state::<Db>();
        let _ = db::record_focus_progress(&database.conn(), session_id, snapshot.elapsed_seconds);
    }
}

fn menu_bar_state(
    app: &AppHandle,
    snapshot: &FocusSnapshot,
    showing_done: bool,
    show_timer: bool,
) -> MenuBarState {
    let execution = app.try_state::<ExecutionState>();
    // An agent blocked on a question needs answering before anything else is
    // worth showing, so it outranks even the focus clock.
    if execution
        .as_ref()
        .map(|state| state.is_awaiting_approval())
        .unwrap_or(false)
    {
        return MenuBarState::AwaitingApproval;
    }

    // A break outranks the rest: while it is running there is nothing else the
    // menu bar could usefully say, and the countdown is the point of taking it.
    if show_timer {
        if let Some(rest) = app.try_state::<rest::RestState>() {
            let resting = rest.snapshot();
            if resting.status == rest::RestStatus::Running {
                return MenuBarState::Rest {
                    remaining_seconds: resting.remaining_seconds,
                };
            }
        }
    }

    // Otherwise a focus session is the developer's own clock and comes first.
    let working = app
        .try_state::<AnalysisState>()
        .map(|state| state.is_running())
        .unwrap_or(false)
        || app
            .try_state::<VerificationState>()
            .map(|state| state.is_running())
            .unwrap_or(false)
        || execution.map(|state| state.is_active()).unwrap_or(false);

    match snapshot.status {
        FocusStatus::Idle if working => MenuBarState::Coding,
        FocusStatus::Idle => MenuBarState::Idle,
        FocusStatus::Finished if showing_done => MenuBarState::Done,
        FocusStatus::Finished if working => MenuBarState::Coding,
        FocusStatus::Finished => MenuBarState::Idle,
        FocusStatus::Running | FocusStatus::Paused => {
            if show_timer {
                MenuBarState::Focus {
                    remaining_seconds: snapshot.remaining_seconds,
                }
            } else {
                MenuBarState::Idle
            }
        }
    }
}

/// Owns the clock so a closed popup — or no popup at all — never stops a session.
fn spawn_timer_thread(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_snapshot: Option<FocusSnapshot> = None;
        let mut last_rest: Option<rest::RestSnapshot> = None;
        let mut last_state: Option<MenuBarState> = None;
        let mut finished_at: Option<Instant> = None;
        let mut last_persist = Instant::now();
        let mut show_timer = true;
        let mut settings_read = Instant::now() - SETTINGS_REFRESH;

        loop {
            std::thread::sleep(TICK);
            let (snapshot, just_finished) = advance_timer(&app);

            // Recover from a worker that never reported a terminal state.
            if let Some(stalled) = app
                .try_state::<AnalysisState>()
                .and_then(|state| state.fail_if_stalled(ANALYSIS_STALL_SECONDS))
            {
                let _ = app.emit("codex:analysis", &stalled);
                let _ = app.emit("goals:changed", ());
            }
            if let Some(stalled) = app
                .try_state::<ExecutionState>()
                .and_then(|state| state.fail_if_stalled(RUN_STALL_SECONDS))
            {
                let _ = app.emit("execution:changed", &stalled);
            }
            if let Some(stalled) = app
                .try_state::<VerificationState>()
                .and_then(|state| state.fail_if_stalled(ANALYSIS_STALL_SECONDS))
            {
                let _ = app.emit("verification:changed", &stalled);
            }

            // A break runs on the same clock as everything else, and its
            // countdown is emitted the same way: only when it has moved.
            let rest_state = app.state::<rest::RestState>();
            if let Some(rested) = rest_state.finish_if_over() {
                let _ = app.emit("rest:changed", &rested);
                notify::send(&app, notify::Event::BreakFinished);
                sound::alarm(&app);
                last_rest = Some(rested);
            } else {
                let rested = rest_state.snapshot();
                if last_rest.as_ref() != Some(&rested) {
                    let _ = app.emit("rest:changed", &rested);
                    last_rest = Some(rested);
                }
            }

            if just_finished {
                finished_at = Some(Instant::now());
                let _ = app.emit("focus:finished", &snapshot);
                sound::alarm(&app);
                // The popup is very likely closed when a session runs out.
                let title = snapshot
                    .task_id
                    .and_then(|id| {
                        let db = app.try_state::<Db>()?;
                        let conn = db.conn();
                        db::get_task(&conn, id).ok().map(|task| task.title)
                    })
                    .unwrap_or_else(|| "Your task".to_string());
                notify::send(&app, notify::Event::FocusFinished { task: &title });
            }
            if snapshot.status == FocusStatus::Idle {
                finished_at = None;
            }

            // Sites are shut only while the clock is actually running; the
            // sweep paces itself, so calling it every tick is not a cost.
            block::sweep(&app, &snapshot);

            if last_persist.elapsed() >= PERSIST_EVERY {
                persist_progress(&app, &snapshot);
                last_persist = Instant::now();
            }

            if last_snapshot.as_ref() != Some(&snapshot) {
                let _ = app.emit("focus:tick", &snapshot);
                last_snapshot = Some(snapshot.clone());
            }

            if settings_read.elapsed() >= SETTINGS_REFRESH {
                show_timer = app
                    .try_state::<Db>()
                    .and_then(|db| db::get_settings(&db.conn()).ok())
                    .map(|settings| settings.show_timer_in_menu_bar)
                    .unwrap_or(true);
                settings_read = Instant::now();
            }

            let showing_done = finished_at
                .map(|at| at.elapsed() < DONE_LINGER)
                .unwrap_or(false);
            let state = menu_bar_state(&app, &snapshot, showing_done, show_timer);
            if last_state.as_ref() != Some(&state) {
                tray::render(&app, &state);
                last_state = Some(state);
            }
        }
    });
}
