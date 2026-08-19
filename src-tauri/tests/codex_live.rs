//! Checks the app-server integration against a real Codex install.
//!
//! Ignored by default: these spawn a process and talk to the developer's own
//! Codex. Run with `cargo test -- --ignored --nocapture`.

use crushit_lib::codex::client::CodexClient;
use crushit_lib::codex::detect;
use serde_json::json;
use std::time::Duration;

/// The categories a freshly migrated database hands the planner.
fn categories() -> Vec<String> {
    vec!["task".to_string(), "bug".to_string()]
}

fn client_or_skip() -> Option<(CodexClient, std::path::PathBuf)> {
    let binary = detect::find_binary(None)?;
    Some((CodexClient::default(), binary))
}

#[test]
#[ignore = "spawns the real codex app-server"]
fn the_cli_is_found_and_reports_its_login_state() {
    let status = detect::status(None);
    println!("{status:#?}");
    assert!(status.installed, "expected a Codex CLI on this machine");
    assert!(status.version.is_some());
}

#[test]
#[ignore = "spawns the real codex app-server"]
fn a_thread_can_be_started_read_only() {
    let Some((client, binary)) = client_or_skip() else {
        panic!("no codex binary found");
    };
    client
        .ensure_started(&binary, "0.1.0-test")
        .expect("app-server should start and handshake");
    assert!(client.is_running());

    let response = client
        .request(
            "thread/start",
            json!({
                "cwd": env!("CARGO_MANIFEST_DIR"),
                "sandbox": "read-only",
                "approvalPolicy": "never",
            }),
            Duration::from_secs(60),
        )
        .expect("thread/start should succeed");

    let thread_id = response["thread"]["id"].as_str().expect("a thread id");
    assert!(!thread_id.is_empty());
    println!("thread {thread_id}");

    // The sandbox the thread reports back must be the read-only one we asked for.
    let sandbox = &response["sandbox"]["type"];
    assert_eq!(sandbox, "readOnly", "planning must never get write access");

    client.shutdown();
    assert!(!client.is_running());
}

#[test]
#[ignore = "spawns the real codex app-server"]
fn the_model_list_comes_back_populated() {
    let Some((client, binary)) = client_or_skip() else {
        panic!("no codex binary found");
    };
    client.ensure_started(&binary, "0.1.0-test").unwrap();
    let response = client
        .request("model/list", json!({}), Duration::from_secs(30))
        .expect("model/list should succeed");
    let models = response["data"].as_array().expect("a model array");
    assert!(!models.is_empty());
    println!("{} models, first: {}", models.len(), models[0]["id"]);
    client.shutdown();
}

#[test]
#[ignore = "spawns the real codex app-server and starts a model turn"]
fn a_planning_turn_runs_end_to_end() {
    use crushit_lib::codex::planning;
    use crushit_lib::plan::AnalysisEvent;

    let Some((client, binary)) = client_or_skip() else {
        panic!("no codex binary found");
    };
    client.ensure_started(&binary, "0.1.0-test").unwrap();

    let mut steps: Vec<String> = Vec::new();
    let outcome = planning::run_analysis(
        &client,
        env!("CARGO_MANIFEST_DIR"),
        &crushit_lib::plan::PlanRequest::goal(
            "Add a command that reports how many tasks are overdue.",
            categories(),
        ),
        None,
        |event| {
            if let AnalysisEvent::StepStarted { label, .. } = event {
                println!("step: {label}");
                steps.push(label);
            }
        },
        || false,
    );
    client.shutdown();

    match outcome {
        Ok(result) => {
            println!("thread {}", result.thread_id);
            println!("{} tasks", result.plan.tasks.len());
            assert!(!result.plan.tasks.is_empty());
            for task in &result.plan.tasks {
                assert!(!task.title.is_empty());
            }
        }
        Err(error) => {
            // A quota or model problem still exercises the whole path; what
            // matters is that it arrives as a sentence a developer can act on.
            let message = error.to_string();
            println!("analysis failed: {message}");
            assert!(!message.is_empty());
            assert!(
                !message.contains("{\"type\""),
                "raw provider JSON leaked into the message: {message}"
            );
        }
    }
}

#[test]
#[ignore = "spawns the real codex app-server"]
fn an_execution_thread_is_started_in_workspace_write() {
    let Some((client, binary)) = client_or_skip() else {
        panic!("no codex binary found");
    };
    client.ensure_started(&binary, "0.1.0-test").unwrap();

    let response = client
        .request(
            "thread/start",
            json!({
                "cwd": env!("CARGO_MANIFEST_DIR"),
                "sandbox": "workspace-write",
                "approvalPolicy": "on-request",
            }),
            Duration::from_secs(60),
        )
        .expect("thread/start should succeed");

    // Execution needs write access, and only inside the workspace.
    assert_eq!(response["sandbox"]["type"], "workspaceWrite");
    assert_eq!(
        response["sandbox"]["networkAccess"], false,
        "the workspace sandbox should not also open the network"
    );
    // Approvals must come to us rather than being auto-granted.
    assert_eq!(response["approvalPolicy"], "on-request");

    client.shutdown();
}
