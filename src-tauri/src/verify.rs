//! Checking finished work against a task's own acceptance criteria.
//!
//! Verification is a second read-only pass: the agent is asked to look at the
//! repository as it stands and say, with evidence, which criteria are met. It
//! never marks the task complete — that stays the developer's decision.

use crate::error::{Error, Result};
use crate::models::Task;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CriterionVerdict {
    pub text: String,
    pub satisfied: bool,
    /// What in the repository shows this, or what is missing.
    #[serde(default)]
    pub evidence: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub criteria: Vec<CriterionVerdict>,
    /// True when the agent believes the work is done.
    #[serde(default)]
    pub complete: bool,
}

impl Verification {
    pub fn satisfied(&self) -> usize {
        self.criteria.iter().filter(|verdict| verdict.satisfied).count()
    }

    pub fn total(&self) -> usize {
        self.criteria.len()
    }

    /// The line the PRD shows under the verdict list.
    pub fn recommendation(&self) -> &'static str {
        if self.complete && self.satisfied() == self.total() {
            "Ready to complete."
        } else {
            "Keep task open."
        }
    }
}

pub fn output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "criteria", "complete"],
        "properties": {
            "summary": { "type": "string" },
            "complete": { "type": "boolean" },
            "criteria": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["text", "satisfied", "evidence"],
                    "properties": {
                        "text": { "type": "string" },
                        "satisfied": { "type": "boolean" },
                        "evidence": { "type": "string" }
                    }
                }
            }
        }
    })
}

pub fn prompt(task: &Task) -> String {
    let mut prompt = format!(
        "Review the current repository against the acceptance criteria for one task.\n\n\
         TASK\n{}",
        task.title
    );
    if let Some(description) = task.description.as_deref().filter(|text| !text.is_empty()) {
        prompt.push_str(&format!("\n\n{description}"));
    }

    prompt.push_str("\n\nCRITERIA");
    for criterion in &task.criteria {
        prompt.push_str(&format!("\n- {}", criterion.text));
    }

    prompt.push_str(
        "\n\nDo not modify anything. Read the code as it stands now.\n\n\
         For every criterion, say whether it is satisfied and give the evidence: \
         name the file, function, or test you found, or state plainly what is missing. \
         Do not assume work is done because the task says so.\n\
         Set `complete` only if every criterion is satisfied.\n\
         Return one verdict per criterion, in the order given.",
    );
    prompt
}

/// The shape to ask for when an agent has no schema-enforcing mode.
pub fn json_instruction() -> String {
    "\n\nReply with a single JSON object and nothing else — no prose, no code fence:\n\
     {\n  \"summary\": string,\n  \"complete\": boolean,\n  \
     \"criteria\": [{ \"text\": string, \"satisfied\": boolean, \"evidence\": string }]\n}"
        .to_string()
}

/// Parses a verdict set and lines it back up with the task's own criteria, so
/// a reordered or invented list cannot misreport the result.
pub fn parse(text: &str, task: &Task) -> Result<Verification> {
    let mut verification: Verification = crate::plan::parse_json_object(text)
        .map_err(|error| Error::invalid(format!("the agent returned an unreadable verdict: {error}")))?;

    verification.summary = verification.summary.trim().to_string();

    let reported = std::mem::take(&mut verification.criteria);
    verification.criteria = task
        .criteria
        .iter()
        .enumerate()
        .map(|(index, criterion)| {
            // Match on text first; fall back to position when the agent
            // paraphrased the criterion rather than quoting it.
            let found = reported
                .iter()
                .find(|verdict| verdict.text.trim() == criterion.text.trim())
                .or_else(|| reported.get(index));
            CriterionVerdict {
                text: criterion.text.clone(),
                satisfied: found.map(|verdict| verdict.satisfied).unwrap_or(false),
                evidence: found
                    .map(|verdict| verdict.evidence.trim().to_string())
                    .unwrap_or_else(|| "No evidence reported.".to_string()),
            }
        })
        .collect();

    // A claim of completeness only stands if every criterion actually passed.
    verification.complete =
        verification.complete && verification.satisfied() == verification.total();
    Ok(verification)
}

// ------------------------------------------------------------------- state

use crate::plan::AnalysisStep;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationStatus {
    #[default]
    Idle,
    Running,
    Ready,
    Failed,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerificationSnapshot {
    pub status: VerificationStatus,
    pub task_id: Option<i64>,
    pub task_title: String,
    pub steps: Vec<AnalysisStep>,
    pub result: Option<Verification>,
    pub satisfied: usize,
    pub total: usize,
    pub recommendation: String,
    pub error: Option<String>,
    pub started_at: i64,
}

#[derive(Default)]
struct Inner {
    snapshot: VerificationSnapshot,
}

/// Backend-owned, like analysis and execution, so closing the popup cannot
/// interrupt a verification in flight.
#[derive(Default)]
pub struct VerificationState {
    inner: Mutex<Inner>,
    cancelled: Arc<AtomicBool>,
}

impl VerificationState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> VerificationSnapshot {
        self.lock().snapshot.clone()
    }

    pub fn is_running(&self) -> bool {
        self.lock().snapshot.status == VerificationStatus::Running
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn begin(&self, task_id: i64, title: String) -> VerificationSnapshot {
        self.cancelled.store(false, Ordering::SeqCst);
        let mut inner = self.lock();
        inner.snapshot = VerificationSnapshot {
            status: VerificationStatus::Running,
            task_id: Some(task_id),
            task_title: title,
            started_at: crate::models::now(),
            ..Default::default()
        };
        inner.snapshot.clone()
    }

    pub fn step_started(&self, id: String, label: String) -> VerificationSnapshot {
        let mut inner = self.lock();
        let steps = &mut inner.snapshot.steps;
        if let Some(existing) = steps.iter_mut().find(|step| step.id == id) {
            existing.label = label;
        } else {
            steps.push(AnalysisStep { id, label, done: false });
            if steps.len() > 10 {
                steps.remove(0);
            }
        }
        inner.snapshot.clone()
    }

    pub fn step_finished(&self, id: &str) -> VerificationSnapshot {
        let mut inner = self.lock();
        if let Some(step) = inner.snapshot.steps.iter_mut().find(|step| step.id == id) {
            step.done = true;
        }
        inner.snapshot.clone()
    }

    pub fn succeeded(&self, result: Verification) -> VerificationSnapshot {
        let mut inner = self.lock();
        inner.snapshot.status = VerificationStatus::Ready;
        inner.snapshot.satisfied = result.satisfied();
        inner.snapshot.total = result.total();
        inner.snapshot.recommendation = result.recommendation().to_string();
        inner.snapshot.result = Some(result);
        inner.snapshot.error = None;
        for step in &mut inner.snapshot.steps {
            step.done = true;
        }
        inner.snapshot.clone()
    }

    pub fn failed(&self, error: String) -> VerificationSnapshot {
        let mut inner = self.lock();
        inner.snapshot.status = VerificationStatus::Failed;
        inner.snapshot.error = Some(error);
        inner.snapshot.clone()
    }

    pub fn clear(&self) -> VerificationSnapshot {
        self.cancelled.store(false, Ordering::SeqCst);
        let mut inner = self.lock();
        *inner = Inner::default();
        inner.snapshot.clone()
    }

    /// Backstop, mirroring the other long-running states.
    pub fn fail_if_stalled(&self, max_seconds: i64) -> Option<VerificationSnapshot> {
        let mut inner = self.lock();
        if inner.snapshot.status != VerificationStatus::Running {
            return None;
        }
        if crate::models::now() - inner.snapshot.started_at < max_seconds {
            return None;
        }
        inner.snapshot.status = VerificationStatus::Failed;
        inner.snapshot.error = Some("Verification stopped responding and was abandoned.".into());
        Some(inner.snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AcceptanceCriterion, TaskCategory, TaskStatus};

    fn a_task(criteria: &[&str]) -> Task {
        Task {
            id: 1,
            project_id: 1,
            goal_id: None,
            title: "Create invoice PDF service".into(),
            description: None,
            category: TaskCategory::Backend,
            status: TaskStatus::NeedsReview,
            position: 0,
            estimate_minutes: None,
            is_ai_generated: true,
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
            files: vec![],
            depends_on: vec![],
            focus_seconds: 0,
            focus_sessions: 0,
        }
    }

    #[test]
    fn verdicts_are_matched_back_to_the_task_criteria() {
        let task = a_task(&["Generates a PDF", "Has a unit test"]);
        let answer = r#"{
          "summary": "Mostly there.",
          "complete": false,
          "criteria": [
            { "text": "Has a unit test", "satisfied": false, "evidence": "No test file found." },
            { "text": "Generates a PDF", "satisfied": true, "evidence": "src/pdf.ts renders one." }
          ]
        }"#;
        let result = parse(answer, &task).unwrap();
        // Order follows the task, not the answer.
        assert_eq!(result.criteria[0].text, "Generates a PDF");
        assert!(result.criteria[0].satisfied);
        assert_eq!(result.criteria[1].text, "Has a unit test");
        assert!(!result.criteria[1].satisfied);
        assert_eq!(result.satisfied(), 1);
        assert_eq!(result.total(), 2);
    }

    #[test]
    fn a_paraphrased_criterion_falls_back_to_its_position() {
        let task = a_task(&["Generates a PDF"]);
        let answer = r#"{"summary":"","complete":true,
          "criteria":[{"text":"PDF generation works","satisfied":true,"evidence":"src/pdf.ts"}]}"#;
        let result = parse(answer, &task).unwrap();
        assert_eq!(result.criteria[0].text, "Generates a PDF");
        assert!(result.criteria[0].satisfied);
    }

    #[test]
    fn a_criterion_the_agent_ignored_counts_as_unmet() {
        let task = a_task(&["Generates a PDF", "Has a unit test"]);
        let answer = r#"{"summary":"","complete":true,
          "criteria":[{"text":"Generates a PDF","satisfied":true,"evidence":"src/pdf.ts"}]}"#;
        let result = parse(answer, &task).unwrap();
        assert!(!result.criteria[1].satisfied);
        assert_eq!(result.criteria[1].evidence, "No evidence reported.");
    }

    #[test]
    fn completeness_is_not_taken_on_the_agents_word() {
        let task = a_task(&["Generates a PDF", "Has a unit test"]);
        let answer = r#"{"summary":"","complete":true,"criteria":[
          {"text":"Generates a PDF","satisfied":true,"evidence":"x"},
          {"text":"Has a unit test","satisfied":false,"evidence":"none"}]}"#;
        let result = parse(answer, &task).unwrap();
        assert!(!result.complete, "a failing criterion cannot be complete");
        assert_eq!(result.recommendation(), "Keep task open.");
    }

    #[test]
    fn everything_satisfied_reads_as_ready() {
        let task = a_task(&["Generates a PDF"]);
        let answer = r#"{"summary":"","complete":true,
          "criteria":[{"text":"Generates a PDF","satisfied":true,"evidence":"src/pdf.ts"}]}"#;
        assert_eq!(parse(answer, &task).unwrap().recommendation(), "Ready to complete.");
    }

    #[test]
    fn a_task_with_no_criteria_cannot_be_verified_into_a_pass() {
        let task = a_task(&[]);
        let result = parse(r#"{"summary":"","complete":true,"criteria":[]}"#, &task).unwrap();
        assert_eq!(result.total(), 0);
    }

    #[test]
    fn a_finished_verification_reports_its_tally() {
        let state = VerificationState::default();
        state.begin(1, "PDF service".into());
        let task = a_task(&["Generates a PDF", "Has a unit test"]);
        let result = parse(
            r#"{"summary":"","complete":false,"criteria":[
                {"text":"Generates a PDF","satisfied":true,"evidence":"src/pdf.ts"},
                {"text":"Has a unit test","satisfied":false,"evidence":"none"}]}"#,
            &task,
        )
        .unwrap();
        let snapshot = state.succeeded(result);
        assert_eq!(snapshot.status, VerificationStatus::Ready);
        assert_eq!((snapshot.satisfied, snapshot.total), (1, 2));
        assert_eq!(snapshot.recommendation, "Keep task open.");
    }

    #[test]
    fn a_wedged_verification_is_abandoned_rather_than_left_running() {
        let state = VerificationState::default();
        state.begin(1, "PDF service".into());
        assert!(state.fail_if_stalled(3600).is_none());
        assert!(state.fail_if_stalled(0).is_some());
        assert!(!state.is_running());
    }

    #[test]
    fn the_prompt_forbids_modification_and_demands_evidence() {
        let prompt = prompt(&a_task(&["Generates a PDF"]));
        assert!(prompt.contains("Do not modify"));
        assert!(prompt.contains("evidence"));
        assert!(prompt.contains("Generates a PDF"));
    }
}
