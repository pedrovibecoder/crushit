pub mod agent;
mod analysis;
pub mod claude;
pub mod codex;
mod commands;
mod db;
mod error;
mod focus;
mod models;
pub mod plan;
mod popup;
mod repo;
mod shortcut;
mod tray;

use analysis::AnalysisState;
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
/// The tray reads one setting; re-querying it four times a second would hold
/// the database lock against the UI for no benefit.
const SETTINGS_REFRESH: Duration = Duration::from_secs(2);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            commands::list_tasks,
            commands::create_task,
            commands::update_task,
            commands::delete_task,
            commands::reorder_tasks,
            commands::set_criterion_met,
            commands::focus_snapshot,
            commands::start_focus,
            commands::pause_focus,
            commands::resume_focus,
            commands::stop_focus,
            commands::get_settings,
            commands::update_settings,
            commands::hide_popup,
            commands::resize_popup,
            commands::quit_app,
            commands::popup_shortcut,
            commands::agent_status,
            commands::agent_models,
            commands::latest_goal,
            commands::analysis_snapshot,
            commands::start_analysis,
            commands::cancel_analysis,
            commands::accept_plan,
            commands::discard_plan,
        ])
        .setup(|app| {
            // The app lives in the menu bar: no dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let database = Db::open(&data_dir.join("blitzit.sqlite3"))?;

            let mut timer = FocusTimer::default();
            focus::restore_from_db(&database, &mut timer)?;

            app.manage(database);
            app.manage(FocusState(Mutex::new(timer)));
            app.manage(popup::PopupAnchor::default());
            app.manage(CodexClient::default());
            app.manage(AnalysisState::default());
            app.manage(shortcut::ShortcutState::default());
            app.manage(agent::StatusCache::default());

            build_tray(app.handle())?;
            shortcut::register(app.handle());
            popup::prewarm(app.handle());
            spawn_timer_thread(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the popup must never quit the app or end a session.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) => {
                if should_hide_on_blur(window.app_handle()) {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("failed to start Blitzit");

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
    let open = MenuItem::with_id(app, "open", "Open Blitzit", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Blitzit", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    let builder = TrayIconBuilder::with_id(tray::TRAY_ID)
        .icon(icon)
        .tooltip("Blitzit")
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
    // A focus session is the developer's own clock, so it outranks Codex.
    let analysing = app
        .try_state::<AnalysisState>()
        .map(|state| state.is_running())
        .unwrap_or(false);

    match snapshot.status {
        FocusStatus::Idle if analysing => MenuBarState::Coding,
        FocusStatus::Idle => MenuBarState::Idle,
        FocusStatus::Finished if showing_done => MenuBarState::Done,
        FocusStatus::Finished if analysing => MenuBarState::Coding,
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

            if just_finished {
                finished_at = Some(Instant::now());
                let _ = app.emit("focus:finished", &snapshot);
            }
            if snapshot.status == FocusStatus::Idle {
                finished_at = None;
            }

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
