//! Running one task through the Claude Code CLI with write access.
//!
//! Headless Claude Code has no channel for answering a permission prompt, so
//! anything that would ask is refused rather than waved through:
//! `--permission-mode acceptEdits` lets it edit files, and everything else
//! (running commands in particular) is declined. Refusals are surfaced in the
//! activity list instead of disappearing.

use crate::error::{Error, Result};
use crate::models::Task;
use crate::run::{self, RunEvent, RunOutcome};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bounds the conversation so a stuck run ends by itself.
const MAX_TURNS: &str = "80";

/// Tools whose input names a file the agent is writing.
const WRITE_TOOLS: &[&str] = &["Edit", "Write", "NotebookEdit", "MultiEdit"];

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// A readable label plus the kind of work, for the activity list.
fn label_for_tool(name: &str, input: &Value) -> (String, &'static str) {
    let field = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or("");
    match name {
        _ if WRITE_TOOLS.contains(&name) => {
            (format!("Editing {}", file_name(field("file_path"))), "file")
        }
        "Read" => (format!("Reading {}", file_name(field("file_path"))), "read"),
        "Bash" => (format!("Running {}", run::short(field("command"), 44)), "command"),
        "Grep" => (format!("Searching for {}", run::short(field("pattern"), 32)), "read"),
        "Glob" => ("Finding files".to_string(), "read"),
        "WebSearch" | "WebFetch" => ("Searching the web".to_string(), "search"),
        "Task" | "Agent" => ("Exploring the repository".to_string(), "tool"),
        "TodoWrite" => ("Organising the work".to_string(), "tool"),
        other => (format!("Using {other}"), "tool"),
    }
}

fn describe_failure(result: &Value) -> String {
    let message = result
        .get("result")
        .and_then(Value::as_str)
        .map(crate::plan::unwrap_error_body)
        .unwrap_or_default();
    match result.get("subtype").and_then(Value::as_str).unwrap_or("") {
        "error_max_turns" => "Claude Code hit its turn limit before finishing.".into(),
        _ if message.is_empty() => "Claude Code stopped without saying why.".into(),
        _ => message,
    }
}

/// Implements one task. Blocking: run it on a worker thread.
pub fn run_task(
    binary: &Path,
    project_path: &str,
    task: &Task,
    resume_session: Option<&str>,
    model: Option<&str>,
    mut on_event: impl FnMut(RunEvent),
    cancelled: Arc<AtomicBool>,
) -> Result<RunOutcome> {
    let mut command = Command::new(binary);
    command
        .current_dir(project_path)
        .arg("-p")
        .arg(run::task_prompt(task))
        .args(["--output-format", "stream-json", "--verbose"])
        .args(["--permission-mode", "acceptEdits"])
        .args(["--max-turns", MAX_TURNS])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(session) = resume_session.filter(|value| !value.trim().is_empty()) {
        command.args(["--resume", session]);
    }
    if let Some(model) = model.filter(|value| !value.trim().is_empty()) {
        command.args(["--model", model]);
    }

    let mut child = command
        .spawn()
        .map_err(|error| Error::invalid(format!("could not start Claude Code: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::invalid("Claude Code produced no output"))?;
    // Stopping has to reach the process; a quiet agent emits nothing to react to.
    let child = Arc::new(Mutex::new(child));
    let finished = run::spawn_canceller(child.clone(), cancelled.clone());

    let deadline = Instant::now() + Duration::from_secs(run::RUN_TIMEOUT_SECONDS);
    let mut session_id = String::new();
    let mut outcome: Option<Result<RunOutcome>> = None;

    for line in BufReader::new(stdout).lines() {
        if cancelled.load(Ordering::SeqCst) {
            finished.store(true, Ordering::SeqCst);
            let mut child = child.lock().unwrap_or_else(|e| e.into_inner());
            let _ = child.kill();
            let _ = child.wait();
            return Ok(RunOutcome {
                thread_id: session_id,
                summary: None,
            });
        }
        if Instant::now() >= deadline {
            finished.store(true, Ordering::SeqCst);
            let mut child = child.lock().unwrap_or_else(|e| e.into_inner());
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::invalid("Claude Code took too long on this task."));
        }

        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        match message.get("type").and_then(Value::as_str) {
            Some("system") => {
                if let Some(id) = message.get("session_id").and_then(Value::as_str) {
                    if session_id != id {
                        session_id = id.to_string();
                        on_event(RunEvent::Thread(session_id.clone()));
                    }
                }
            }
            Some("assistant") => {
                for block in content_blocks(&message) {
                    match block.get("type").and_then(Value::as_str) {
                        Some("tool_use") => {
                            let (Some(id), Some(name)) = (
                                block.get("id").and_then(Value::as_str),
                                block.get("name").and_then(Value::as_str),
                            ) else {
                                continue;
                            };
                            let input = block.get("input").cloned().unwrap_or(Value::Null);
                            let (label, kind) = label_for_tool(name, &input);
                            on_event(RunEvent::Started {
                                id: id.to_string(),
                                label,
                                kind,
                            });
                            if WRITE_TOOLS.contains(&name) {
                                if let Some(path) =
                                    input.get("file_path").and_then(Value::as_str)
                                {
                                    on_event(RunEvent::FileChanged(run::relative_to(
                                        project_path,
                                        path,
                                    )));
                                }
                            }
                        }
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                if !text.trim().is_empty() {
                                    on_event(RunEvent::Message(text.to_string()));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("user") => {
                for block in content_blocks(&message) {
                    if let Some(id) = block.get("tool_use_id").and_then(Value::as_str) {
                        on_event(RunEvent::Finished { id: id.to_string() });
                    }
                }
            }
            Some("result") => {
                if let Some(id) = message.get("session_id").and_then(Value::as_str) {
                    session_id = id.to_string();
                }
                report_denials(&message, &mut on_event);
                let failed = message.get("is_error").and_then(Value::as_bool) == Some(true)
                    || message.get("subtype").and_then(Value::as_str) != Some("success");
                outcome = Some(if failed {
                    Err(Error::invalid(describe_failure(&message)))
                } else {
                    Ok(RunOutcome {
                        thread_id: session_id.clone(),
                        summary: message
                            .get("result")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    })
                });
                // Helper processes can hold stdout open after the CLI exits.
                break;
            }
            _ => {}
        }
    }

    finished.store(true, Ordering::SeqCst);
    let status = {
        let mut child = child.lock().unwrap_or_else(|e| e.into_inner());
        let _ = child.kill();
        child.wait()
    };
    match outcome {
        Some(outcome) => outcome,
        None => {
            let code = status.ok().and_then(|status| status.code()).unwrap_or(-1);
            Err(Error::invalid(format!(
                "Claude Code exited without finishing (status {code})."
            )))
        }
    }
}

fn content_blocks(message: &Value) -> Vec<Value> {
    message
        .get("message")
        .and_then(|inner| inner.get("content"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Anything the agent was refused is shown rather than quietly dropped.
fn report_denials(result: &Value, on_event: &mut impl FnMut(RunEvent)) {
    let denials = result
        .get("permission_denials")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, denial) in denials.iter().enumerate() {
        let tool = denial
            .get("tool_name")
            .and_then(Value::as_str)
            .unwrap_or("a tool");
        on_event(RunEvent::Started {
            id: format!("denied-{index}"),
            label: format!("Refused: {tool} needs approval"),
            kind: "denied",
        });
        on_event(RunEvent::Finished {
            id: format!("denied-{index}"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_edit_is_labelled_by_file_and_counted_as_a_change() {
        let (label, kind) = label_for_tool("Edit", &json!({ "file_path": "/repo/src/pdf.ts" }));
        assert_eq!(label, "Editing pdf.ts");
        assert_eq!(kind, "file");
        assert!(WRITE_TOOLS.contains(&"Write"));
    }

    #[test]
    fn reading_and_editing_are_told_apart() {
        assert_eq!(label_for_tool("Read", &json!({ "file_path": "/a/b.ts" })).1, "read");
        assert_eq!(label_for_tool("Write", &json!({ "file_path": "/a/b.ts" })).1, "file");
    }

    #[test]
    fn a_refused_tool_becomes_a_visible_activity_line() {
        let result = json!({
            "permission_denials": [{ "tool_name": "Bash" }],
        });
        let mut labels: Vec<String> = Vec::new();
        let mut sink = |event: RunEvent| {
            if let RunEvent::Started { label, .. } = event {
                labels.push(label);
            }
        };
        report_denials(&result, &mut sink);
        assert_eq!(labels, vec!["Refused: Bash needs approval"]);
    }

    #[test]
    fn hitting_the_turn_limit_is_explained() {
        let result = json!({ "subtype": "error_max_turns", "is_error": true, "result": "" });
        assert!(describe_failure(&result).contains("turn limit"));
    }
}
