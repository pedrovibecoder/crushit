//! Checks the Claude Code integration against a real install.
//!
//! Ignored by default: these spawn the CLI and spend account quota.
//! Run with `cargo test --test claude_live -- --ignored --nocapture`.

use crushit_lib::agent;
use crushit_lib::claude::{detect, planning};
use crushit_lib::plan::{self, AnalysisEvent};

/// The categories a freshly migrated database hands the planner.
fn categories() -> Vec<String> {
    vec!["task".to_string(), "bug".to_string()]
}

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
        &plan::PlanRequest::goal(&goal, categories()),
        Some(&model),
        |event| {
            if let AnalysisEvent::StepStarted { label, .. } = event {
                println!("step: {label}");
                steps.push(label);
            }
        },
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

#[test]
#[ignore = "runs the real claude CLI, writes to BLITZIT_FIXTURE, and spends quota"]
fn a_task_run_edits_the_repository_and_reports_what_it_touched() {
    use crushit_lib::claude::execution;
    use crushit_lib::models::{AcceptanceCriterion, Task, TaskStatus};
    use crushit_lib::run::RunEvent;

    let binary = detect::find_binary(None).expect("claude on this machine");
    let fixture = std::env::var("BLITZIT_FIXTURE").expect("set BLITZIT_FIXTURE to a repo path");

    let task = Task {
        id: 1,
        project_id: 1,
        goal_id: None,
        title: "Add a health check route".into(),
        description: Some("Expose GET /health returning {\"status\":\"ok\"}.".into()),
        category: "task".into(),
        story_points: Some(3),
        status: TaskStatus::InProgress,
        position: 0,
        estimate_minutes: Some(10),
        is_ai_generated: false,
        created_at: 0,
        updated_at: 0,
        completed_at: None,
        criteria: vec![AcceptanceCriterion {
            id: 1,
            task_id: 1,
            text: "GET /health responds with status ok".into(),
            is_met: false,
            position: 0,
        }],
        files: vec!["src/server.js".into()],
        depends_on: vec![],
        focus_seconds: 0,
        focus_sessions: 0,
    };

    let mut changed: Vec<String> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    let mut thread = String::new();

    let outcome = execution::run_task(
        &binary,
        &fixture,
        &task,
        None,
        Some("sonnet"),
        |event| match event {
            RunEvent::Started { label, .. } => {
                println!("· {label}");
                labels.push(label);
            }
            RunEvent::FileChanged(path) => changed.push(path),
            RunEvent::Thread(id) => thread = id,
            _ => {}
        },
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );

    let result = outcome.expect("the run should finish");
    println!("session {}", result.thread_id);
    println!("changed: {changed:?}");

    assert!(!thread.is_empty(), "a session id should have been reported");
    assert!(!labels.is_empty(), "activity should have been reported");
    assert!(!changed.is_empty(), "the run should report the files it edited");
    for path in &changed {
        assert!(
            !path.starts_with('/'),
            "changed files must be shown relative to the repository, got {path}"
        );
    }

    // The point of execution is that the repository actually changed.
    let status = std::process::Command::new("git")
        .args(["-C", &fixture, "status", "--porcelain"])
        .output()
        .expect("git status");
    let dirty = String::from_utf8_lossy(&status.stdout).trim().to_string();
    assert!(!dirty.is_empty(), "expected edits in the working tree");
    println!("git status:\n{dirty}");
}

#[test]
#[ignore = "runs the real claude CLI and cancels it"]
fn cancelling_stops_the_run_promptly_even_while_the_agent_is_quiet() {
    use crushit_lib::claude::planning;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let binary = detect::find_binary(None).expect("claude on this machine");
    let fixture = std::env::var("BLITZIT_FIXTURE").expect("set BLITZIT_FIXTURE to a repo path");

    let cancelled = Arc::new(AtomicBool::new(false));
    {
        // Cancel while it is still thinking, which is when nothing is being
        // printed and a between-the-lines check would never fire.
        let cancelled = cancelled.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(5));
            cancelled.store(true, Ordering::SeqCst);
        });
    }

    let started = std::time::Instant::now();
    let outcome = planning::run_analysis(
        &binary,
        &fixture,
        &plan::PlanRequest::goal(
            "Rewrite this service to be event driven, and explain every trade-off in detail.",
            categories(),
        ),
        Some("sonnet"),
        |_| {},
        cancelled,
    );
    let elapsed = started.elapsed();
    println!("returned after {elapsed:?}: {outcome:?}", outcome = outcome.is_err());

    assert!(outcome.is_err(), "a cancelled run does not produce a plan");
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "cancelling should stop the run within seconds, took {elapsed:?}"
    );
}

#[test]
#[ignore = "runs the real claude CLI and spends quota"]
fn verification_judges_each_criterion_on_the_code_that_is_there() {
    use crushit_lib::claude::planning;
    use crushit_lib::models::{AcceptanceCriterion, Task, TaskStatus};
    use crushit_lib::verify;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    let binary = detect::find_binary(None).expect("claude on this machine");
    let fixture = std::env::var("BLITZIT_FIXTURE").expect("set BLITZIT_FIXTURE to a repo path");

    // One criterion the fixture satisfies, one it plainly does not.
    let criteria = [
        "GET /invoices/:id returns the invoice as JSON",
        "There is a unit test covering the invoice route",
    ];
    let task = Task {
        id: 1,
        project_id: 1,
        goal_id: None,
        title: "Invoice endpoint".into(),
        description: None,
        category: "task".into(),
        story_points: Some(3),
        status: TaskStatus::NeedsReview,
        position: 0,
        estimate_minutes: None,
        is_ai_generated: false,
        created_at: 0,
        updated_at: 0,
        completed_at: None,
        criteria: criteria
            .iter()
            .enumerate()
            .map(|(index, text)| AcceptanceCriterion {
                id: index as i64,
                task_id: 1,
                text: (*text).to_string(),
                is_met: false,
                position: index as i64,
            })
            .collect(),
        files: vec!["src/server.js".into()],
        depends_on: vec![],
        focus_seconds: 0,
        focus_sessions: 0,
    };

    let (text, _) = planning::run_read_only(
        &binary,
        &fixture,
        &format!("{}{}", verify::prompt(&task), verify::json_instruction()),
        Some("sonnet"),
        |_| {},
        Arc::new(AtomicBool::new(false)),
        "took too long",
    )
    .expect("verification should run");

    let result = verify::parse(&text, &task).expect("a readable verdict");
    for verdict in &result.criteria {
        println!(
            "{} {} — {}",
            if verdict.satisfied { "PASS" } else { "FAIL" },
            verdict.text,
            verdict.evidence
        );
    }

    assert_eq!(result.total(), 2, "one verdict per criterion");
    assert!(result.criteria[0].satisfied, "the route plainly exists");
    assert!(!result.criteria[1].satisfied, "there is no test in the fixture");
    assert!(!result.complete, "not every criterion passed");
    assert_eq!(result.recommendation(), "Keep task open.");
    assert!(
        !result.criteria[0].evidence.is_empty(),
        "a verdict without evidence is not worth showing"
    );
}
