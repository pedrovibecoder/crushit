//! The alarm that marks the end of something.
//!
//! Played by the system rather than by the webview. A session ends precisely
//! when the developer has walked away and the popup is closed, and a hidden
//! webview is exactly what the OS is entitled to suspend — so the one moment
//! the sound matters is the one moment the browser cannot be relied on for it.

use crate::db::{self, Db};
use tauri::{AppHandle, Manager};

/// Bundled with macOS since forever, and short enough to repeat.
#[cfg(target_os = "macos")]
const ALARM: &str = "/System/Library/Sounds/Glass.aiff";

/// How many times the alarm repeats. One chime is missable from another room.
#[cfg(target_os = "macos")]
const REPEATS: usize = 3;

fn enabled(app: &AppHandle) -> bool {
    app.try_state::<Db>()
        .and_then(|db| db::get_settings(&db.conn()).ok())
        .map(|settings| settings.sounds)
        .unwrap_or(true)
}

/// Sounds the alarm, unless the developer has turned sounds off.
///
/// Returns immediately: playback happens on its own thread so the timer that
/// asked for it keeps ticking.
#[cfg(target_os = "macos")]
pub fn alarm(app: &AppHandle) {
    if !enabled(app) || !std::path::Path::new(ALARM).exists() {
        return;
    }
    std::thread::spawn(|| {
        for _ in 0..REPEATS {
            // `afplay` runs until the sound finishes, which is what spaces the
            // repeats out; a failure here is not worth reporting anywhere.
            let played = std::process::Command::new("afplay").arg(ALARM).status();
            if played.is_err() {
                break;
            }
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn alarm(app: &AppHandle) {
    let _ = enabled(app);
}
