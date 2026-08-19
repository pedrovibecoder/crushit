//! Running one task through the Codex app server with write access.
//!
//! Unlike planning, this turn may change the repository: the thread is started
//! with the `workspace-write` sandbox and an `on-request` approval policy, so
//! anything Codex wants beyond that comes back as a request a human answers.

use super::client::{CodexClient, Notification, APPROVAL_REQUESTED};
use crate::error::{Error, Result};
use crate::execution::ApprovalRequest;
use crate::models::Task;
use crate::run::{self, RunEvent, RunOutcome};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// A readable label plus the kind of work, for the activity list.
fn label_for(item: &Value) -> Option<(String, String, &'static str)> {
    let id = item.get("id").and_then(Value::as_str)?.to_string();
    let (label, kind) = match item.get("type").and_then(Value::as_str)? {
        "commandExecution" => {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("a command");
            (format!("Running {}", run::short(command, 44)), "command")
        }
        "fileChange" => {
            let files = changed_paths(item);
            let label = match files.len() {
                0 => "Editing files".to_string(),
                1 => format!("Editing {}", file_name(&files[0])),
                count => format!("Editing {count} files"),
            };
            (label, "file")
        }
        "reasoning" => ("Working out what to do".to_string(), "thinking"),
        "webSearch" => ("Searching the web".to_string(), "search"),
        "mcpToolCall" | "dynamicToolCall" => ("Using a tool".to_string(), "tool"),
        "agentMessage" => ("Writing up the change".to_string(), "message"),
        _ => return None,
    };
    Some((id, label, kind))
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn changed_paths(item: &Value) -> Vec<String> {
    item.get("changes")
        .and_then(Value::as_array)
        .map(|changes| {
            changes
                .iter()
                .filter_map(|change| change.get("path").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Describes a parked permission request in the terms the developer sees.
fn describe_approval(approval: &super::client::PendingApproval) -> ApprovalRequest {
    let title = match approval.method.as_str() {
        "item/fileChange/requestApproval" | "applyPatchApproval" => "Apply file changes?",
        "item/permissions/requestApproval" => "Grant permission?",
        _ => "Run command?",
    };
    ApprovalRequest {
        id: approval.id,
        title: title.to_string(),
        command: approval.command.clone(),
        reason: approval.reason.clone(),
    }
}

fn describe_error(error: &Value) -> String {
    let message = crate::plan::unwrap_error_body(
        error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Codex stopped without saying why"),
    );
    match error.get("codexErrorInfo").and_then(Value::as_str) {
        Some("usageLimitExceeded") => format!("Codex usage limit reached. {message}"),
        Some("unauthorized") => "Codex is not signed in. Sign in and try again.".to_string(),
        _ => message,
    }
}

/// Implements one task. Blocking: run it on a worker thread.
pub fn run_task(
    client: &CodexClient,
    project_path: &str,
    task: &Task,
    resume_thread: Option<&str>,
    model: Option<&str>,
    mut on_event: impl FnMut(RunEvent),
    is_cancelled: impl Fn() -> bool,
    is_blocked: impl Fn() -> bool,
) -> Result<RunOutcome> {
    let events = client.subscribe();

    // Resuming keeps the task's own conversation, so a continued run remembers
    // what the stopped one already did.
    let settings = json!({
        "cwd": project_path,
        "sandbox": "workspace-write",
        "approvalPolicy": "on-request",
    });
    let response = match resume_thread {
        Some(thread_id) => {
            let mut params = settings.clone();
            params["threadId"] = json!(thread_id);
            client.request("thread/resume", params, Duration::from_secs(60))?
        }
        None => client.request("thread/start", settings, Duration::from_secs(60))?,
    };
    let thread_id = response
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| Error::invalid("Codex did not return a thread id"))?
        .to_string();
    on_event(RunEvent::Thread(thread_id.clone()));

    let mut turn = json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": run::task_prompt(task) }],
        "sandboxPolicy": { "type": "workspaceWrite", "networkAccess": false },
        "approvalPolicy": "on-request",
        "cwd": project_path,
    });
    if let Some(model) = model.filter(|value| !value.trim().is_empty()) {
        turn["model"] = json!(model);
    }
    client.request("turn/start", turn, Duration::from_secs(60))?;

    let timeout = Duration::from_secs(run::RUN_TIMEOUT_SECONDS);
    let mut deadline = Instant::now() + timeout;
    let mut summary: Option<String> = None;

    loop {
        // Time spent waiting on a person is not time spent running.
        if is_blocked() {
            deadline = Instant::now() + timeout;
        }
        if is_cancelled() {
            client.decline_all_pending();
            let _ = client.request(
                "turn/interrupt",
                json!({ "threadId": thread_id }),
                Duration::from_secs(10),
            );
            return Ok(RunOutcome {
                thread_id,
                summary: None,
            });
        }
        if Instant::now() >= deadline {
            client.decline_all_pending();
            let _ = client.request(
                "turn/interrupt",
                json!({ "threadId": thread_id }),
                Duration::from_secs(10),
            );
            return Err(Error::invalid("Codex took too long on this task."));
        }

        let Ok(Notification { method, params }) = events.recv_timeout(Duration::from_millis(400))
        else {
            continue;
        };
        if let Some(other) = params.get("threadId").and_then(Value::as_str) {
            if other != thread_id {
                continue;
            }
        }

        match method.as_str() {
            APPROVAL_REQUESTED => {
                for approval in client.pending_approvals() {
                    on_event(RunEvent::Approval(describe_approval(&approval)));
                }
            }
            "item/started" => {
                if let Some((id, label, kind)) = params.get("item").and_then(label_for) {
                    on_event(RunEvent::Started { id, label, kind });
                }
            }
            "item/completed" => {
                let item = params.get("item");
                if let Some((id, label, kind)) = item.and_then(label_for) {
                    on_event(RunEvent::Started { id: id.clone(), label, kind });
                    on_event(RunEvent::Finished { id });
                }
                if let Some(item) = item {
                    for path in changed_paths(item) {
                        on_event(RunEvent::FileChanged(run::relative_to(project_path, &path)));
                    }
                    if item.get("type").and_then(Value::as_str) == Some("agentMessage") {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            summary = Some(text.to_string());
                            on_event(RunEvent::Message(text.to_string()));
                        }
                    }
                }
            }
            "error" => {
                client.decline_all_pending();
                return Err(Error::invalid(&describe_error(
                    &params.get("error").cloned().unwrap_or(Value::Null),
                )));
            }
            "turn/completed" => {
                client.decline_all_pending();
                let turn = params.get("turn").cloned().unwrap_or(Value::Null);
                if turn.get("status").and_then(Value::as_str) == Some("failed") {
                    return Err(Error::invalid(&describe_error(
                        &turn.get("error").cloned().unwrap_or(Value::Null),
                    )));
                }
                return Ok(RunOutcome { thread_id, summary });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::client::PendingApproval;

    #[test]
    fn a_file_change_names_the_file_it_touched() {
        let item = json!({
            "id": "a", "type": "fileChange",
            "changes": [{ "path": "src/services/invoice.ts", "kind": "update", "diff": "" }]
        });
        let (_, label, kind) = label_for(&item).unwrap();
        assert_eq!(label, "Editing invoice.ts");
        assert_eq!(kind, "file");
    }

    #[test]
    fn several_file_changes_are_counted_rather_than_listed() {
        let item = json!({
            "id": "a", "type": "fileChange",
            "changes": [
                { "path": "a.ts", "kind": "update", "diff": "" },
                { "path": "b.ts", "kind": "add", "diff": "" }
            ]
        });
        assert_eq!(label_for(&item).unwrap().1, "Editing 2 files");
        assert_eq!(changed_paths(&item), vec!["a.ts", "b.ts"]);
    }

    #[test]
    fn a_long_command_is_trimmed_for_the_activity_list() {
        let item = json!({ "id": "a", "type": "commandExecution", "command": "x".repeat(200) });
        let label = label_for(&item).unwrap().1;
        assert!(label.chars().count() <= "Running ".len() + 45);
    }

    #[test]
    fn approvals_are_titled_by_what_is_being_asked() {
        let command = PendingApproval {
            id: 1,
            method: "item/commandExecution/requestApproval".into(),
            thread_id: "t".into(),
            command: Some("npm install pdf-lib".into()),
            reason: Some("Required for PDF generation.".into()),
            cwd: None,
        };
        let described = describe_approval(&command);
        assert_eq!(described.title, "Run command?");
        assert_eq!(described.command.as_deref(), Some("npm install pdf-lib"));

        let patch = PendingApproval {
            method: "item/fileChange/requestApproval".into(),
            ..command
        };
        assert_eq!(describe_approval(&patch).title, "Apply file changes?");
    }

    #[test]
    fn noise_items_stay_out_of_the_activity_list() {
        assert!(label_for(&json!({ "id": "b", "type": "userMessage" })).is_none());
    }
}
