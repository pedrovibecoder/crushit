//! Placement and visibility of the menu-bar popup window.

use crate::error::Result;
use std::sync::Mutex;
use tauri::{AppHandle, LogicalPosition, Manager, PhysicalPosition, Rect, WebviewWindow};

pub const POPUP_LABEL: &str = "popup";

/// The top-left the popup was last placed at, in logical pixels.
///
/// macOS anchors a resize to the window's bottom edge, so growing or shrinking
/// the popup would walk it away from the menu bar. Remembering where it was
/// put lets the resize restore the top edge.
#[derive(Default)]
pub struct PopupAnchor(pub Mutex<Option<(f64, f64)>>);

impl PopupAnchor {
    fn store(&self, position: (f64, f64)) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(position);
    }

    fn read(&self) -> Option<(f64, f64)> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Gap in logical pixels between the menu bar and the top of the popup.
const GAP: f64 = 6.0;
/// Room left below the popup so it never runs to the bottom of the screen.
const BOTTOM_MARGIN: f64 = 24.0;
/// Used when the screen cannot be measured.
const FALLBACK_MAX_HEIGHT: f64 = 620.0;
/// Keeps the popup off the very edge of the screen.
const SCREEN_MARGIN: f64 = 8.0;

pub fn window(app: &AppHandle) -> Result<WebviewWindow> {
    app.get_webview_window(POPUP_LABEL)
        .ok_or(crate::error::Error::NotFound("popup window"))
}

fn anchor_center_and_bottom(rect: &Rect, scale: f64) -> (f64, f64) {
    let (x, y) = match rect.position {
        tauri::Position::Physical(p) => (p.x as f64 / scale, p.y as f64 / scale),
        tauri::Position::Logical(p) => (p.x, p.y),
    };
    let (width, height) = match rect.size {
        tauri::Size::Physical(s) => (s.width as f64 / scale, s.height as f64 / scale),
        tauri::Size::Logical(s) => (s.width, s.height),
    };
    (x + width / 2.0, y + height)
}

/// Places the popup under the tray icon, clamped to the visible screen.
pub fn position_under(app: &AppHandle, win: &WebviewWindow, anchor: Option<Rect>) -> Result<()> {
    let scale = win.scale_factor()?;
    let size = win.outer_size()?.to_logical::<f64>(scale);

    let monitor = win.current_monitor()?.or(win.primary_monitor()?);
    let (screen_x, screen_y, screen_w) = match &monitor {
        Some(m) => {
            let pos = m.position().to_logical::<f64>(scale);
            let dims = m.size().to_logical::<f64>(scale);
            (pos.x, pos.y, dims.width)
        }
        None => (0.0, 0.0, size.width),
    };

    let (center_x, bottom_y) = match anchor {
        Some(rect) => anchor_center_and_bottom(&rect, scale),
        // Without a tray rect (keyboard or menu invocation) fall back to the
        // top-right corner, where the menu-bar extras live.
        None => (screen_x + screen_w - size.width / 2.0 - SCREEN_MARGIN, screen_y),
    };

    let min_x = screen_x + SCREEN_MARGIN;
    let max_x = screen_x + screen_w - size.width - SCREEN_MARGIN;
    let x = (center_x - size.width / 2.0).clamp(min_x, max_x.max(min_x));
    let y = bottom_y + GAP;

    app.state::<PopupAnchor>().store((x, y));
    win.set_position(LogicalPosition::new(x, y))?;
    Ok(())
}

/// Restores the remembered top-left, undoing macOS's bottom-anchored resize.
pub fn reapply_anchor(app: &AppHandle, win: &WebviewWindow) -> Result<()> {
    if let Some((x, y)) = app.state::<PopupAnchor>().read() {
        win.set_position(LogicalPosition::new(x, y))?;
    }
    Ok(())
}

/// The tallest the popup may be on the screen it is currently on.
pub fn max_height(win: &WebviewWindow) -> f64 {
    let Ok(scale) = win.scale_factor() else {
        return FALLBACK_MAX_HEIGHT;
    };
    let monitor = win
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| win.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return FALLBACK_MAX_HEIGHT;
    };
    let screen = monitor.size().to_logical::<f64>(scale).height;
    // The popup hangs from just under the menu bar, so that offset is lost.
    let top = anchored_top(win).unwrap_or(GAP);
    (screen - top - BOTTOM_MARGIN).max(240.0)
}

/// Where the popup's top edge currently sits, if it has been placed.
fn anchored_top(win: &WebviewWindow) -> Option<f64> {
    let scale = win.scale_factor().ok()?;
    let position = win.outer_position().ok()?.to_logical::<f64>(scale);
    (position.y > 0.0).then_some(position.y)
}

pub fn show(app: &AppHandle, anchor: Option<Rect>) -> Result<()> {
    let win = window(app)?;
    position_under(app, &win, anchor)?;
    win.show()?;
    win.set_focus()?;
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<()> {
    window(app)?.hide()?;
    Ok(())
}

pub fn toggle(app: &AppHandle, anchor: Option<Rect>) -> Result<()> {
    let win = window(app)?;
    if win.is_visible().unwrap_or(false) {
        win.hide()?;
        Ok(())
    } else {
        show(app, anchor)
    }
}

/// Used on startup so the first open is not the first paint.
pub fn prewarm(app: &AppHandle) {
    if let Ok(win) = window(app) {
        let _ = win.set_position(PhysicalPosition::new(-10_000, -10_000));
    }
}
