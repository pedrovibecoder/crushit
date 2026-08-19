//! A global hotkey for opening the popup.
//!
//! macOS hides menu-bar items it cannot fit — on a laptop with a notch and a
//! busy menu bar the tray icon may never be drawn, even though it exists. The
//! hotkey is the way back into the app when that happens.

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

/// Tried in order; the first one the system accepts wins.
const CANDIDATES: &[(Modifiers, Code, &str)] = &[
    (
        Modifiers::CONTROL.union(Modifiers::ALT),
        Code::KeyB,
        "⌃⌥B",
    ),
    (
        Modifiers::CONTROL.union(Modifiers::SHIFT),
        Code::KeyB,
        "⌃⇧B",
    ),
    (
        Modifiers::CONTROL
            .union(Modifiers::ALT)
            .union(Modifiers::SHIFT),
        Code::KeyB,
        "⌃⌥⇧B",
    ),
];

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutInfo {
    /// The combination that is live, e.g. "⌃⌥B".
    pub combo: Option<String>,
    /// Why no combination could be registered.
    pub problem: Option<String>,
}

#[derive(Default)]
pub struct ShortcutState(pub Mutex<ShortcutInfo>);

impl ShortcutState {
    pub fn get(&self) -> ShortcutInfo {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set(&self, info: ShortcutInfo) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = info;
    }
}

pub fn matches(shortcut: &Shortcut) -> bool {
    CANDIDATES
        .iter()
        .any(|(modifiers, code, _)| shortcut.matches(*modifiers, *code))
}

/// Registers the first combination the system will accept.
pub fn register(app: &AppHandle) {
    let state = app.state::<ShortcutState>();
    let mut last_error = None;

    for (modifiers, code, label) in CANDIDATES {
        let shortcut = Shortcut::new(Some(*modifiers), *code);
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                state.set(ShortcutInfo {
                    combo: Some((*label).to_string()),
                    problem: None,
                });
                return;
            }
            // Usually means another app already owns the combination.
            Err(error) => last_error = Some(error.to_string()),
        }
    }

    let problem = last_error.unwrap_or_else(|| "no shortcut could be registered".to_string());
    eprintln!("blitzit: no popup shortcut could be registered: {problem}");
    state.set(ShortcutInfo { combo: None, problem: Some(problem) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_candidate_is_recognised_by_the_handler() {
        for (modifiers, code, _) in CANDIDATES {
            assert!(matches(&Shortcut::new(Some(*modifiers), *code)));
        }
    }

    #[test]
    fn an_unrelated_combination_is_not_ours() {
        assert!(!matches(&Shortcut::new(Some(Modifiers::META), Code::KeyB)));
        assert!(!matches(&Shortcut::new(
            Some(Modifiers::CONTROL | Modifiers::ALT),
            Code::KeyK
        )));
    }

    #[test]
    fn the_candidates_are_distinct() {
        let labels: std::collections::BTreeSet<_> =
            CANDIDATES.iter().map(|(_, _, label)| *label).collect();
        assert_eq!(labels.len(), CANDIDATES.len());
    }
}
