//! Checks the Claude Code integration against a real install.
//!
//! Ignored by default: these spawn the CLI and spend account quota.
//! Run with `cargo test --test claude_live -- --ignored --nocapture`.

use blitzit_lib::agent;
use blitzit_lib::claude::{detect, planning};
use blitzit_lib::plan::AnalysisEvent;

#[test]
#[ignore = "runs the real claude CLI"]
fn the_cli_is_found_and_reports_its_login_state() {
    let status = detect::status(None);
    println!("{status:#?}");
    assert!(status.installed, "expected Claude Code on this machine");
    assert!(status.version.is_some());
    assert_eq!(status.agent, agent::Agent::ClaudeCode);
}

#[test]
#[ignore = "runs the real claude CLI and spends quota"]
fn a_planning_run_produces_a_usable_plan() {
    let binary = detect::find_binary(None).expect("claude on this machine");
    let fixture = std::env::var("BLITZIT_FIXTURE").expect("set BLITZIT_FIXTURE to a repo path");
    let goal = std::env::var("BLITZIT_GOAL")
        .unwrap_or_else(|_| "Let customers download an invoice as a PDF.".to_string());
    let model = std::env::var("BLITZIT_MODEL").unwrap_or_else(|_| "sonnet".to_string());
    let started = std::time::Instant::now();

    // Compare before and after: the repository may already be dirty for
    // reasons that have nothing to do with us.
    let porcelain = |label: &str| -> String {
        let output = std::process::Command::new("git")
            .args(["-C", &fixture, "status", "--porcelain"])
            .output()
            .unwrap_or_else(|error| panic!("git status {label}: {error}"));
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    let before = porcelain("before");

    let mut steps: Vec<String> = Vec::new();
    let outcome = planning::run_analysis(
        &binary,
        &fixture,
        &goal,
        Some(&model),
        |event| {
            if let AnalysisEvent::StepStarted { label, .. } = event {
                println!("step: {label}");
                steps.push(label);
            }
        },
        || false,
    );

    // The promise is that planning cannot change the repository.
    let after = porcelain("after");
    assert_eq!(
        before, after,
        "analysis changed the working tree\nbefore:\n{before}\nafter:\n{after}"
    );

    match outcome {
        Ok(result) => {
            println!("took {:?}", started.elapsed());
            println!("session {}", result.thread_id);
            println!("summary: {}", result.plan.summary);
            for (index, task) in result.plan.tasks.iter().enumerate() {
                println!(
                    "  {}. {} [{}] {}m deps={:?} criteria={}",
                    index + 1,
                    task.title,
                    task.category,
                    task.estimate_minutes,
                    task.depends_on,
                    task.acceptance_criteria.len()
                );
            }
            assert!(!result.plan.tasks.is_empty(), "a plan must contain tasks");
            assert!(!steps.is_empty(), "progress steps should have been reported");
            for task in &result.plan.tasks {
                assert!(!task.title.is_empty());
                // Sanitising must have normalised every category.
                assert!(matches!(
                    task.category.as_str(),
                    "frontend" | "backend" | "database" | "security"
                        | "testing" | "devops" | "refactor" | "bug"
                ));
            }
        }
        Err(error) => {
            let message = error.to_string();
            println!("analysis failed: {message}");
            assert!(!message.contains("{\"type\""), "raw JSON leaked: {message}");
            panic!("expected a plan: {message}");
        }
    }
}
