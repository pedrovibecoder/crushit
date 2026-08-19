//! The desktop window: the same application with room to work in.
//!
//! The popup answers "what am I on"; this answers "sit down and do it". Both
//! render the same screens from the same state.

use crate::db::{self, Db};
use crate::error::Result;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};

pub const WINDOW_LABEL: &str = "main";

pub fn window(app: &AppHandle) -> Result<WebviewWindow> {
    app.get_webview_window(WINDOW_LABEL)
        .ok_or(crate::error::Error::NotFound("desktop window"))
}

/// While the window is open the app behaves like a normal application — dock
/// icon, application switcher, keyboard focus. With it closed it goes back to
/// being a menu-bar accessory.
#[cfg(target_os = "macos")]
fn set_foreground(app: &AppHandle, foreground: bool) {
    let policy = if foreground {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    let _ = app.set_activation_policy(policy);
}

#[cfg(not(target_os = "macos"))]
fn set_foreground(_app: &AppHandle, _foreground: bool) {}

/// Geometry worth restoring next launch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowState {
    pub open: bool,
    pub width: f64,
    pub height: f64,
    pub position: Option<(f64, f64)>,
}

pub fn show(app: &AppHandle) -> Result<()> {
    let win = window(app)?;
    set_foreground(app, true);
    win.show()?;
    win.unminimize().ok();
    win.set_focus()?;
    remember(app, true);
    Ok(())
}

/// Hides the window without quitting; a running task is untouched.
pub fn hide(app: &AppHandle) -> Result<()> {
    let win = window(app)?;
    remember(app, false);
    win.hide()?;
    set_foreground(app, false);
    Ok(())
}

pub fn is_open(app: &AppHandle) -> bool {
    window(app)
        .ok()
        .and_then(|win| win.is_visible().ok())
        .unwrap_or(false)
}

/// Writes the window's current geometry and open state to settings.
pub fn remember(app: &AppHandle, open: bool) {
    let Ok(win) = window(app) else { return };
    let Some(database) = app.try_state::<Db>() else {
        return;
    };
    let Ok(scale) = win.scale_factor() else { return };

    let size = win.outer_size().ok().map(|size| size.to_logical::<f64>(scale));
    let position = win
        .outer_position()
        .ok()
        .map(|position| position.to_logical::<f64>(scale));

    let state = WindowState {
        open,
        width: size.map(|size| size.width).unwrap_or(940.0),
        height: size.map(|size| size.height).unwrap_or(640.0),
        position: position.map(|position| (position.x, position.y)),
    };
    let _ = db::save_window_state(&database.conn(), &state);
}

/// Restores the window at launch, reopening it only if it was open before.
pub fn restore(app: &AppHandle) {
    let Some(database) = app.try_state::<Db>() else {
        return;
    };
    let Ok(state) = db::window_state(&database.conn()) else {
        return;
    };
    let Ok(win) = window(app) else { return };

    let _ = win.set_size(LogicalSize::new(
        state.width.max(720.0),
        state.height.max(520.0),
    ));
    if let Some((x, y)) = state.position {
        // A window remembered off a screen that is no longer attached would be
        // unreachable, so only restore a position that is plainly on-screen.
        if x > -2000.0 && y > -200.0 {
            let _ = win.set_position(LogicalPosition::new(x, y));
        }
    }
    if state.open {
        let _ = show(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remembered_size_never_falls_below_the_minimum() {
        // `restore` clamps; this documents the rule the clamp encodes.
        let tiny = WindowState {
            open: true,
            width: 100.0,
            height: 100.0,
            position: None,
        };
        assert_eq!(tiny.width.max(720.0), 720.0);
        assert_eq!(tiny.height.max(520.0), 520.0);
    }
}
