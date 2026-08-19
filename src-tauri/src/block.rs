//! Shutting the door on the sites that eat a focus session.
//!
//! There is no way to reach inside a browser from outside it, so this asks
//! politely and often: once a second the running browsers are asked what
//! their tabs are showing, and any tab sitting on a blocked site is sent to a
//! page that says why. It is a door, not a wall — someone who means to get
//! around it can, and that is the right trade for a feature that must never
//! be able to lock a person out of their own machine.

use crate::db::{self, Db};
use crate::focus::{FocusSnapshot, FocusStatus};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// What the list starts as, for someone who turns this on without editing it.
/// Deliberately short. A developer looks things up on YouTube and Reddit all
/// day, so shutting those by default would teach people to turn the whole
/// feature off; the list is theirs to extend.
pub const DEFAULT_SITES: &[&str] = &[
    "instagram.com",
    "facebook.com",
    "x.com",
    "twitter.com",
    "tiktok.com",
];

/// How often the browsers are asked what they are showing. Every tick would
/// mean four `osascript` processes a second for no gain: nobody reads a feed
/// in under a second, and the reply itself costs tens of milliseconds.
const SWEEP_EVERY: Duration = Duration::from_millis(900);

/// Once macOS has refused the Apple Event, asking again immediately only
/// burns processes — the answer will not change until the user changes it.
const RETRY_AFTER_DENIAL: Duration = Duration::from_secs(60);

/// The browsers that can be asked, and the dialect each one answers in.
/// Chromium's dictionary calls it the `active tab` of a window; Safari's calls
/// the same thing the `current tab`.
const BROWSERS: &[(&str, Tab)] = &[
    ("Google Chrome", Tab::Active),
    ("Brave Browser", Tab::Active),
    ("Microsoft Edge", Tab::Active),
    ("Chromium", Tab::Active),
    ("Safari", Tab::Current),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Active,
    Current,
}

impl Tab {
    fn phrase(self) -> &'static str {
        match self {
            Tab::Active => "active tab",
            Tab::Current => "current tab",
        }
    }
}

/// Whether macOS will let the app talk to a browser at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    /// Never asked, or no browser was open to ask.
    #[default]
    Unknown,
    Granted,
    /// The user said no, or has not answered the system prompt yet.
    Denied,
}

#[derive(Default)]
struct Inner {
    last_sweep: Option<Instant>,
    denied_at: Option<Instant>,
    permission: Permission,
}

/// Held by the app so the timer thread can pace itself between ticks.
#[derive(Default)]
pub struct BlockState {
    inner: Mutex<Inner>,
    /// True while a sweep is out talking to the browsers. A slow reply must
    /// not stack up sweeps behind it.
    busy: AtomicBool,
}

impl BlockState {
    /// True when enough time has passed to ask the browsers again.
    fn due(&self) -> bool {
        let mut inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(denied) = inner.denied_at {
            if denied.elapsed() < RETRY_AFTER_DENIAL {
                return false;
            }
        }
        let due = inner
            .last_sweep
            .map(|at| at.elapsed() >= SWEEP_EVERY)
            .unwrap_or(true);
        if due {
            inner.last_sweep = Some(Instant::now());
        }
        due
    }

    fn record(&self, permission: Permission) {
        let mut inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(poisoned) => poisoned.into_inner(),
        };
        inner.permission = permission;
        inner.denied_at = match permission {
            Permission::Denied => Some(Instant::now()),
            _ => None,
        };
    }

    /// Claims the right to run a sweep, if one is not already out.
    fn claim(&self) -> bool {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn release(&self) {
        self.busy.store(false, Ordering::Release);
    }

    pub fn permission(&self) -> Permission {
        match self.inner.lock() {
            Ok(inner) => inner.permission,
            Err(poisoned) => poisoned.into_inner().permission,
        }
    }
}

// -------------------------------------------------------------- the list

/// Reads the list as a person would write it: one per line, or separated by
/// commas or spaces, with or without the parts of a URL that are not the site.
pub fn parse_sites(raw: &str) -> Vec<String> {
    let mut sites: Vec<String> = Vec::new();
    for piece in raw.split(|c: char| c == ',' || c == '\n' || c == ';' || c.is_whitespace()) {
        if let Some(site) = normalise(piece) {
            if !sites.contains(&site) {
                sites.push(site);
            }
        }
    }
    sites
}

/// `https://www.Instagram.com/explore` and `instagram.com` are the same site.
fn normalise(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').to_ascii_lowercase();
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&trimmed);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .trim_matches('.');
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host.to_string())
}

/// The host a URL points at, or nothing for a URL with no host to speak of.
pub fn host_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return None;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Which site on the list a URL belongs to, subdomains included: blocking
/// `facebook.com` has to catch `m.facebook.com`, or it blocks nothing at all.
pub fn blocked_by<'a>(url: &str, sites: &'a [String]) -> Option<&'a str> {
    let host = host_of(url)?;
    sites
        .iter()
        .find(|site| host == site.as_str() || host.ends_with(&format!(".{site}")))
        .map(|site| site.as_str())
}

// --------------------------------------------------------- talking to macOS

/// One tab, as the browser described it.
#[derive(Debug, PartialEq)]
struct OpenTab {
    browser: String,
    window: String,
    url: String,
}

/// AppleScript has no escape for a quote inside a literal beyond a backslash.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Asks every browser that happens to be running what its windows are showing.
///
/// `application "X" is running` is answered by AppleScript itself rather than
/// by the application, so a browser that is closed stays closed — asking must
/// never be the thing that opens Chrome.
fn read_script() -> String {
    // `tab` and `linefeed` cannot be used by name here: inside a `tell` block
    // a browser's own dictionary owns the word `tab`, and the script stops
    // compiling. Held as variables, they are unambiguous everywhere.
    let mut script = String::from(
        "set out to \"\"\nset sep to (ASCII character 9)\nset nl to (ASCII character 10)\n",
    );
    for (browser, tab) in BROWSERS.iter().filter(|(name, _)| installed(name)) {
        script.push_str(&format!(
            "if application {name} is running then\n\
             \ttry\n\
             \t\ttell application {name}\n\
             \t\t\trepeat with w in windows\n\
             \t\t\t\ttry\n\
             \t\t\t\t\tset out to out & {name} & sep & ((id of w) as text) & sep & (URL of {phrase} of w) & nl\n\
             \t\t\t\tend try\n\
             \t\t\tend repeat\n\
             \t\tend tell\n\
             \tend try\n\
             end if\n",
            name = quote(browser),
            phrase = tab.phrase(),
        ));
    }
    script.push_str("return out\n");
    script
}

/// Sends one window's front tab somewhere else.
fn navigate_script(browser: &str, window: &str, url: &str) -> String {
    let phrase = BROWSERS
        .iter()
        .find(|(name, _)| *name == browser)
        .map(|(_, tab)| tab.phrase())
        .unwrap_or(Tab::Active.phrase());
    format!(
        "if application {name} is running then\n\
         \ttell application {name}\n\
         \t\trepeat with w in windows\n\
         \t\t\ttry\n\
         \t\t\t\tif ((id of w) as text) is {window} then set URL of {phrase} of w to {url}\n\
         \t\t\tend try\n\
         \t\tend repeat\n\
         \tend tell\n\
         end if\n",
        name = quote(browser),
        window = quote(window),
        url = quote(url),
    )
}

/// Where macOS keeps applications. A browser that is not in one of these is
/// not installed as far as this feature is concerned.
const APP_DIRS: &[&str] = &["/Applications", "/System/Applications"];

/// AppleScript resolves an application's vocabulary when the script is
/// compiled, not when it runs — so naming a browser that is not installed
/// fails the whole script, taking the installed ones down with it. Only the
/// browsers actually on the machine are named.
fn installed(browser: &str) -> bool {
    let bundle = format!("{browser}.app");
    APP_DIRS
        .iter()
        .map(std::path::PathBuf::from)
        .chain(
            std::env::var_os("HOME")
                .map(|home| std::path::PathBuf::from(home).join("Applications")),
        )
        .any(|dir| dir.join(&bundle).exists())
}

fn parse_tabs(output: &str) -> Vec<OpenTab> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let browser = parts.next()?.trim();
            let window = parts.next()?.trim();
            let url = parts.next()?.trim();
            (!browser.is_empty() && !url.is_empty()).then(|| OpenTab {
                browser: browser.to_string(),
                window: window.to_string(),
                url: url.to_string(),
            })
        })
        .collect()
}

/// `-1743` is macOS saying the user has not allowed this app to drive that one.
fn is_refusal(stderr: &str) -> bool {
    stderr.contains("-1743") || stderr.contains("Not authorized to send Apple events")
}

#[cfg(target_os = "macos")]
fn osascript(script: &str) -> std::result::Result<String, Permission> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|_| Permission::Unknown)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_refusal(&stderr) {
        return Err(Permission::Denied);
    }
    if !output.status.success() {
        return Err(Permission::Unknown);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(target_os = "macos"))]
fn osascript(_script: &str) -> std::result::Result<String, Permission> {
    Err(Permission::Unknown)
}

// ------------------------------------------------------------- the sweep

/// The page a blocked tab is sent to, written out fresh so it can name the
/// task you are supposed to be on.
const BLOCK_PAGE: &str = include_str!("blocked.html");

fn block_page_url(app: &AppHandle, task: &str, remaining: i64) -> Option<String> {
    let dir = app.path().app_cache_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("blocked.html");
    let page = BLOCK_PAGE
        .replace("{{task}}", &escape_html(task))
        .replace("{{remaining}}", &crate::focus::format_clock(remaining));
    std::fs::write(&path, page).ok()?;
    Some(format!("file://{}", path.to_string_lossy()))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// What the running session is called, for the page to say out loud.
fn task_title(app: &AppHandle, snapshot: &FocusSnapshot) -> String {
    snapshot
        .task_id
        .and_then(|id| {
            let db = app.try_state::<Db>()?;
            let conn = db.conn();
            db::get_task(&conn, id).ok().map(|task| task.title)
        })
        .unwrap_or_else(|| "your task".to_string())
}

/// Called from the timer thread on every tick; does nothing unless a session
/// is actually running and the developer asked for this.
///
/// The talking happens on a thread of its own. The timer thread also draws the
/// menu-bar clock, and `osascript` takes long enough that doing it there would
/// show up as a countdown that stutters once a second.
pub fn sweep(app: &AppHandle, snapshot: &FocusSnapshot) {
    if snapshot.status != FocusStatus::Running {
        return;
    }
    let Some(state) = app.try_state::<BlockState>() else {
        return;
    };
    let settings = app
        .try_state::<Db>()
        .and_then(|db| db::get_settings(&db.conn()).ok());
    let Some(settings) = settings else { return };
    if !settings.focus_block_enabled || settings.focus_block_sites.is_empty() {
        return;
    }
    if !state.due() || !state.claim() {
        return;
    }

    // Read before leaving the tick: both need the database, and the sweep
    // thread should not be holding that lock while it waits on a browser.
    let title = task_title(app, snapshot);
    let remaining = snapshot.remaining_seconds;
    let sites = settings.focus_block_sites.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        run_sweep(&app, &sites, &title, remaining);
        if let Some(state) = app.try_state::<BlockState>() {
            state.release();
        }
    });
}

fn run_sweep(app: &AppHandle, sites: &[String], title: &str, remaining: i64) {
    let Some(state) = app.try_state::<BlockState>() else {
        return;
    };
    let script = read_script();
    if !script.contains("tell application") {
        return;
    }
    let tabs = match osascript(&script) {
        Ok(output) => {
            state.record(Permission::Granted);
            parse_tabs(&output)
        }
        Err(permission) => {
            state.record(permission);
            return;
        }
    };

    let hits: Vec<&OpenTab> = tabs
        .iter()
        .filter(|tab| blocked_by(&tab.url, sites).is_some())
        .collect();
    if hits.is_empty() {
        return;
    }

    let Some(url) = block_page_url(app, title, remaining) else {
        return;
    };
    for tab in hits {
        let _ = osascript(&navigate_script(&tab.browser, &tab.window, &url));
    }
}

/// Asks the browsers one question, only to find out whether macOS allows it.
/// Used by the settings screen, where "it is not working" needs an answer.
pub fn probe() -> Permission {
    let script = read_script();
    if !script.contains("tell application") {
        return Permission::Unknown;
    }
    match osascript(&script) {
        Ok(_) => Permission::Granted,
        Err(permission) => permission,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_is_read_however_it_was_written() {
        let sites = parse_sites("https://www.Instagram.com/explore, facebook.com\nx.com");
        assert_eq!(sites, vec!["instagram.com", "facebook.com", "x.com"]);
    }

    #[test]
    fn nonsense_and_repeats_are_dropped() {
        assert_eq!(parse_sites("  , notasite, ,\n"), Vec::<String>::new());
        assert_eq!(parse_sites("x.com, www.x.com"), vec!["x.com"]);
    }

    #[test]
    fn a_host_is_read_off_a_url() {
        assert_eq!(host_of("https://www.facebook.com/feed").as_deref(), Some("facebook.com"));
        assert_eq!(host_of("http://m.facebook.com:8080/").as_deref(), Some("m.facebook.com"));
        assert_eq!(host_of("file:///tmp/blocked.html"), None);
        assert_eq!(host_of("about:blank"), None);
    }

    #[test]
    fn subdomains_of_a_blocked_site_are_blocked_too() {
        let sites = parse_sites("facebook.com");
        assert_eq!(blocked_by("https://m.facebook.com/", &sites), Some("facebook.com"));
        assert_eq!(blocked_by("https://facebook.com/x", &sites), Some("facebook.com"));
    }

    #[test]
    fn a_site_that_merely_ends_the_same_way_is_not_blocked() {
        let sites = parse_sites("x.com");
        assert_eq!(blocked_by("https://notx.com/", &sites), None);
        assert_eq!(blocked_by("https://example.com/x.com", &sites), None);
    }

    #[test]
    fn the_block_page_itself_is_never_blocked() {
        let sites = parse_sites("instagram.com");
        assert_eq!(blocked_by("file:///Users/me/blocked.html", &sites), None);
    }

    #[test]
    fn tabs_are_read_back_off_the_reply() {
        let tabs = parse_tabs("Google Chrome\t1\thttps://x.com/\nSafari\t7\thttps://a.b/c\n\n");
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].browser, "Google Chrome");
        assert_eq!(tabs[1].url, "https://a.b/c");
    }

    #[test]
    fn a_quote_in_a_url_cannot_end_the_script() {
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert!(navigate_script("Safari", "1", "file:///x\"y").contains("\\\""));
    }

    #[test]
    fn only_installed_browsers_are_named() {
        // Safari ships with macOS, so it is always there to find; a browser
        // invented for this test never is.
        assert!(installed("Safari"));
        assert!(!installed("Browser That Does Not Exist"));
        assert!(!read_script().contains("Browser That Does Not Exist"));
    }

    #[test]
    fn safari_is_asked_for_its_current_tab_and_chrome_for_its_active_one() {
        let script = read_script();
        assert!(script.contains("URL of current tab of w"));
        // Asking must never be what opens a browser.
        assert!(script.contains("is running"));
        // `tab` is a browser's own word inside a `tell` block.
        assert!(!script.contains("& tab &"));
    }

    #[test]
    fn a_refusal_is_told_apart_from_a_failure() {
        assert!(is_refusal("execution error: Not authorized to send Apple events (-1743)"));
        assert!(!is_refusal("execution error: Can't get window 1. (-1728)"));
    }
}


