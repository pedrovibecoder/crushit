//! Driving one read-only planning run through the Claude Code CLI.
//!
//! Claude Code has no long-lived server, so a run is one `claude -p`
//! subprocess streaming newline-delimited JSON on stdout.
//!
//! Read-only is enforced by refusing every tool that can write. Plan mode
//! alone is **not** enough: it blocks the editing tools but still allows
//! `Bash`, and a shell can write anywhere. Analysis therefore inspects the
//! repository with `Read`, `Glob` and `Grep` only.

use crate::error::{Error, Result};
use crate::plan::{self, AnalysisEvent, AnalysisOutcome};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// A hard stop on the conversation. Without it a run that gets into a retry
/// loop keeps emitting events until the wall-clock timeout, which reads to the
/// user as "took too long" rather than "gave up".
const MAX_TURNS: &str = "40";

/// Every tool that can change something on disk. `Bash` is on this list
/// deliberately: plan mode permits it, and a shell can write anywhere.
const DISALLOWED_WRITES: &[&str] = &[
    "Write",
    "Edit",
    "NotebookEdit",
    "Bash",
    "BashOutput",
    "KillShell",
];

/// Tools that need a person. There is nobody to answer them in a headless
/// run, so allowing them only burns turns.
const DISALLOWED_INTERACTIVE: &[&str] = &["AskUserQuestion"];

/// A readable label for a tool the agent just started using.
fn label_for_tool(name: &str, input: &Value) -> String {
    let field = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or("");
    let short = |text: &str| -> String { text.chars().take(48).collect() };

    match name {
        "Read" => {
            let path = field("file_path");
            let file = Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "a file".to_string());
            format!("Reading {file}")
        }
        "Bash" => format!("Running {}", short(field("command"))),
        "Grep" => format!("Searching for {}", short(field("pattern"))),
        "Glob" => "Finding files".to_string(),
        "WebSearch" | "WebFetch" => "Searching the web".to_string(),
        "Task" | "Agent" => "Exploring the repository".to_string(),
        "TodoWrite" => "Organising the work".to_string(),
        other => format!("Using {other}"),
    }
}

/// The agent's final answer, preferring a schema-validated object when the run
/// produced one.
fn answer_text(result: &Value) -> Result<String> {
    if let Some(structured) = result.get("structured_output").filter(|v| !v.is_null()) {
        return Ok(structured.to_string());
    }
    result
        .get("result")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::invalid("Claude Code finished without answering."))
}

fn describe_failure(result: &Value) -> String {
    let message = result
        .get("result")
        .and_then(Value::as_str)
        .map(plan::unwrap_error_body)
        .unwrap_or_default();
    let subtype = result.get("subtype").and_then(Value::as_str).unwrap_or("");
    match subtype {
        "error_max_turns" => {
            "Claude Code gave up before finishing the plan. Try a more specific goal."
                .into()
        }
        "error_during_execution" if message.is_empty() => {
            "Claude Code stopped part-way through the analysis.".into()
        }
        _ if message.is_empty() => "Claude Code stopped without saying why.".into(),
        _ => message,
    }
}

/// Runs one read-only planning run end to end. Blocking: call it on a worker
/// thread so closing the popup cannot interrupt it.
pub fn run_analysis(
    binary: &Path,
    project_path: &str,
    request: &plan::PlanRequest,
    model: Option<&str>,
    on_event: impl FnMut(AnalysisEvent),
    cancelled: Arc<AtomicBool>,
) -> Result<AnalysisOutcome> {
    let (text, session) = run_read_only(
        binary,
        project_path,
        &request.spelled_out(),
        model,
        on_event,
        cancelled,
        "Claude Code took too long to analyse this project. \
         Try a more specific goal, or a faster model in Settings.",
    )?;
    Ok(AnalysisOutcome {
        plan: request.parse(&text)?,
        thread_id: session,
    })
}

/// One read-only run, returning the agent's final answer and its session id.
/// Shared by planning and verification.
pub fn run_read_only(
    binary: &Path,
    project_path: &str,
    prompt: &str,
    model: Option<&str>,
    mut on_event: impl FnMut(AnalysisEvent),
    cancelled: Arc<AtomicBool>,
    timeout_message: &str,
) -> Result<(String, String)> {
    let mut command = Command::new(binary);
    command
        .current_dir(project_path)
        .arg("-p")
        // The shape goes in the prompt rather than through `--json-schema`.
        // The schema is enforced by a tool call, and when a payload trips its
        // input limit the model retries with fewer fields — which then fails
        // the schema's own `required` list, and it never converges.
        .arg(prompt)
        .args(["--output-format", "stream-json", "--verbose"])
        .args(["--permission-mode", "plan"])
        .args(["--max-turns", MAX_TURNS])
        .arg("--disallowedTools")
        .args(DISALLOWED_WRITES)
        .args(DISALLOWED_INTERACTIVE)
        // Without a closed stdin the CLI waits on it before starting.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    let child = Arc::new(Mutex::new(child));
    let finished = crate::run::spawn_canceller(child.clone(), cancelled.clone());

    let deadline = Instant::now() + ANALYSIS_TIMEOUT;
    let mut session_id = String::new();
    let mut outcome: Option<Result<(String, String)>> = None;

    for line in BufReader::new(stdout).lines() {
        if cancelled.load(Ordering::SeqCst) {
            finished.store(true, Ordering::SeqCst);
            let mut child = child.lock().unwrap_or_else(|e| e.into_inner());
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::invalid("Analysis cancelled."));
        }
        if Instant::now() >= deadline {
            finished.store(true, Ordering::SeqCst);
            let mut child = child.lock().unwrap_or_else(|e| e.into_inner());
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::invalid(timeout_message));
        }

        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            // The CLI occasionally prints a plain warning line; ignore it.
            continue;
        };

        match message.get("type").and_then(Value::as_str) {
            Some("system") => {
                if let Some(id) = message.get("session_id").and_then(Value::as_str) {
                    session_id = id.to_string();
                }
            }
            // Assistant turns carry the tool calls worth showing as progress.
            Some("assistant") => {
                let blocks = message
                    .get("message")
                    .and_then(|inner| inner.get("content"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let (Some(id), Some(name)) = (
                        block.get("id").and_then(Value::as_str),
                        block.get("name").and_then(Value::as_str),
                    ) else {
                        continue;
                    };
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    on_event(AnalysisEvent::StepStarted {
                        id: id.to_string(),
                        label: label_for_tool(name, &input),
                    });
                }
            }
            // Tool results come back as a user turn.
            Some("user") => {
                let blocks = message
                    .get("message")
                    .and_then(|inner| inner.get("content"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for block in blocks {
                    if let Some(id) = block.get("tool_use_id").and_then(Value::as_str) {
                        on_event(AnalysisEvent::StepFinished { id: id.to_string() });
                    }
                }
            }
            Some("result") => {
                if let Some(id) = message.get("session_id").and_then(Value::as_str) {
                    session_id = id.to_string();
                }
                let failed = message.get("is_error").and_then(Value::as_bool) == Some(true)
                    || message.get("subtype").and_then(Value::as_str) != Some("success");
                outcome = Some(if failed {
                    Err(Error::invalid(describe_failure(&message)))
                } else {
                    answer_text(&message).map(|text| (text, session_id.clone()))
                });
                // The result is the last thing worth reading. Waiting for the
                // stream to close can hang: the CLI starts helper processes
                // that inherit stdout, so the pipe can stay open after the CLI
                // itself has gone.
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
                "Claude Code exited without producing a plan (status {code})."
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_labels_read_as_progress_not_as_api_calls() {
        assert_eq!(
            label_for_tool("Read", &json!({ "file_path": "/repo/src/models/invoice.ts" })),
            "Reading invoice.ts"
        );
        assert_eq!(
            label_for_tool("Bash", &json!({ "command": "rg --files" })),
            "Running rg --files"
        );
        assert_eq!(label_for_tool("Glob", &json!({})), "Finding files");
    }

    #[test]
    fn an_unknown_tool_still_gets_a_readable_label() {
        assert_eq!(label_for_tool("SomethingNew", &json!({})), "Using SomethingNew");
    }

    #[test]
    fn a_long_command_is_truncated_rather_than_flooding_the_list() {
        let label = label_for_tool("Bash", &json!({ "command": "x".repeat(200) }));
        assert!(label.chars().count() <= "Running ".len() + 48);
    }

    #[test]
    fn the_schema_validated_object_is_preferred_over_the_prose() {
        let result = json!({
            "structured_output": { "summary": "s" },
            "result": "some prose that is not the answer"
        });
        let text = answer_text(&result).unwrap();
        assert!(text.contains("\"summary\""));
        assert!(!text.contains("prose"));
    }

    #[test]
    fn prose_is_used_when_no_structured_output_came_back() {
        let result = json!({ "structured_output": Value::Null, "result": "the answer" });
        assert_eq!(answer_text(&result).unwrap(), "the answer");
    }

    #[test]
    fn a_failed_run_is_explained_in_a_sentence() {
        let hit_limit = json!({ "subtype": "error_max_turns", "is_error": true, "result": "" });
        assert!(describe_failure(&hit_limit).contains("gave up"));

        let silent = json!({ "subtype": "error_during_execution", "is_error": true });
        assert!(!describe_failure(&silent).is_empty());
    }

    #[test]
    fn every_tool_that_can_write_is_refused() {
        // Bash matters most: plan mode allows it, and a shell can write anywhere.
        for tool in ["Write", "Edit", "NotebookEdit", "Bash"] {
            assert!(DISALLOWED_WRITES.contains(&tool), "{tool} must be refused");
        }
    }

    #[test]
    fn tools_that_need_a_person_are_refused_too() {
        assert!(DISALLOWED_INTERACTIVE.contains(&"AskUserQuestion"));
    }
}
