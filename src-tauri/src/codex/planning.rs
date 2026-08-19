//! Driving one read-only planning turn through the Codex app server.
//!
//! Planning runs read-only: the thread and the turn both pin Codex to a
//! read-only sandbox and the client declines every approval, so analysis can
//! never write to the user's repository.

use super::client::{CodexClient, Notification, APPROVAL_REQUESTED};
use crate::error::{Error, Result};
use crate::plan::{self, AnalysisEvent, AnalysisOutcome};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Ceiling on how long one analysis may run before it is abandoned.
const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// A readable label for an item the agent started working on.
fn label_for(item: &Value) -> Option<(String, String)> {
    let id = item.get("id").and_then(Value::as_str)?.to_string();
    let label = match item.get("type").and_then(Value::as_str)? {
        "commandExecution" => {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("a command");
            let short: String = command.chars().take(48).collect();
            format!("Running {short}")
        }
        "reasoning" => "Working out what matters".to_string(),
        "webSearch" => "Searching the web".to_string(),
        "mcpToolCall" | "dynamicToolCall" => "Using a tool".to_string(),
        "agentMessage" => "Writing the plan".to_string(),
        // Everything else is noise for a progress list.
        _ => return None,
    };
    Some((id, label))
}

/// Turns a Codex error payload into something worth showing a developer.
fn describe_error(error: &Value) -> String {
    let message = plan::unwrap_error_body(
        error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Codex stopped without saying why"),
    );
    match error.get("codexErrorInfo").and_then(Value::as_str) {
        Some("usageLimitExceeded") => format!("Codex usage limit reached. {message}"),
        Some("unauthorized") => "Codex is not signed in. Sign in and try again.".to_string(),
        Some("contextWindowExceeded") => {
            "This repository was too large for one analysis. Try a narrower goal.".to_string()
        }
        _ => message,
    }
}

/// Runs one read-only planning turn end to end. Blocking: call it on a worker
/// thread so closing the popup cannot interrupt it.
pub fn run_analysis(
    client: &CodexClient,
    project_path: &str,
    request: &plan::PlanRequest,
    model: Option<&str>,
    on_event: impl FnMut(AnalysisEvent),
    is_cancelled: impl Fn() -> bool,
) -> Result<AnalysisOutcome> {
    let (text, thread_id) = run_read_only_turn(
        client,
        project_path,
        &request.prompt,
        request.schema(),
        model,
        on_event,
        is_cancelled,
        "Codex took too long to analyse this project.",
    )?;
    Ok(AnalysisOutcome {
        plan: request.parse(&text)?,
        thread_id,
    })
}

/// One turn that may only read, constrained to a JSON Schema. Shared by
/// planning and verification, which differ only in what they ask for.
#[allow(clippy::too_many_arguments)]
pub fn run_read_only_turn(
    client: &CodexClient,
    project_path: &str,
    prompt: &str,
    schema: Value,
    model: Option<&str>,
    mut on_event: impl FnMut(AnalysisEvent),
    is_cancelled: impl Fn() -> bool,
    timeout_message: &str,
) -> Result<(String, String)> {
    // Subscribe before starting anything so no notification is missed.
    let events = client.subscribe();

    let thread = client.request(
        "thread/start",
        json!({
            "cwd": project_path,
            "sandbox": "read-only",
            "approvalPolicy": "never",
        }),
        Duration::from_secs(60),
    )?;
    let thread_id = thread
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::invalid("Codex did not return a thread id"))?
        .to_string();

    let mut turn_params = json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": prompt }],
        "outputSchema": schema,
        "sandboxPolicy": { "type": "readOnly", "networkAccess": false },
        "approvalPolicy": "never",
        "cwd": project_path,
    });
    if let Some(model) = model.filter(|value| !value.trim().is_empty()) {
        turn_params["model"] = json!(model);
    }
    client.request("turn/start", turn_params, Duration::from_secs(60))?;

    let deadline = Instant::now() + ANALYSIS_TIMEOUT;
    let mut latest_message: Option<String> = None;

    loop {
        if is_cancelled() {
            let _ = client.request(
                "turn/interrupt",
                json!({ "threadId": thread_id }),
                Duration::from_secs(10),
            );
            return Err(Error::invalid("Analysis cancelled."));
        }
        if Instant::now() >= deadline {
            let _ = client.request(
                "turn/interrupt",
                json!({ "threadId": thread_id }),
                Duration::from_secs(10),
            );
            return Err(Error::invalid(timeout_message));
        }

        let Ok(Notification { method, params }) = events.recv_timeout(Duration::from_millis(500))
        else {
            // A quiet stretch is normal; only the deadline ends the wait.
            continue;
        };

        // Notifications for other threads (future phases) are not ours.
        if let Some(other) = params.get("threadId").and_then(Value::as_str) {
            if other != thread_id {
                continue;
            }
        }

        match method.as_str() {
            // Planning is read-only, so nothing here may be granted. The
            // client parks approvals for a human; refuse them immediately.
            APPROVAL_REQUESTED => client.decline_all_pending(),
            "item/started" => {
                if let Some((id, label)) = params.get("item").and_then(label_for) {
                    on_event(AnalysisEvent::StepStarted { id, label });
                }
            }
            "item/completed" => {
                let item = params.get("item");
                if let Some(id) = item.and_then(|i| i.get("id")).and_then(Value::as_str) {
                    on_event(AnalysisEvent::StepFinished { id: id.to_string() });
                }
                let is_message =
                    item.and_then(|i| i.get("type")).and_then(Value::as_str) == Some("agentMessage");
                if is_message {
                    if let Some(text) = item.and_then(|i| i.get("text")).and_then(Value::as_str) {
                        latest_message = Some(text.to_string());
                    }
                }
            }
            "error" => {
                let error = params.get("error").cloned().unwrap_or(Value::Null);
                return Err(Error::invalid(describe_error(&error)));
            }
            "turn/completed" => {
                let turn = params.get("turn").cloned().unwrap_or(Value::Null);
                if turn.get("status").and_then(Value::as_str) == Some("failed") {
                    let error = turn.get("error").cloned().unwrap_or(Value::Null);
                    return Err(Error::invalid(describe_error(&error)));
                }
                let text = latest_message
                    .ok_or_else(|| Error::invalid("Codex finished without answering."))?;
                return Ok((text, thread_id));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_limits_are_explained_rather_than_dumped() {
        let error = json!({
            "message": "You've hit your usage limit. Try again at Aug 20th.",
            "codexErrorInfo": "usageLimitExceeded"
        });
        let described = describe_error(&error);
        assert!(described.starts_with("Codex usage limit reached."));
        assert!(described.contains("Aug 20th"));
    }

    #[test]
    fn a_provider_json_body_is_reduced_to_its_sentence() {
        // Exactly what codex 0.132.0 returned when the configured model was too new.
        let error = json!({
            "message": r#"{"type":"error","status":400,"error":{"type":"invalid_request_error","message":"The 'gpt-5.6-sol' model requires a newer version of Codex."}}"#,
            "codexErrorInfo": "other"
        });
        let described = describe_error(&error);
        assert!(described.starts_with("The 'gpt-5.6-sol' model requires"));
        assert!(!described.contains('{'));
    }

    #[test]
    fn only_meaningful_items_become_progress_steps() {
        let command = json!({ "id": "a", "type": "commandExecution", "command": "ls -la" });
        assert_eq!(label_for(&command).unwrap().1, "Running ls -la");
        assert!(label_for(&json!({ "id": "b", "type": "userMessage" })).is_none());
    }
}
