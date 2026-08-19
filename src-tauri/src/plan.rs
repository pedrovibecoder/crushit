//! The plan a coding agent produces, and the prompt and schema that shape it.
//!
//! This is deliberately agent-neutral: Codex and Claude Code are asked the same
//! question, constrained by the same JSON Schema, and their answers are
//! sanitised the same way.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// More tasks than this stops being a plan and starts being a backlog.
pub const MAX_TASKS: usize = 8;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTask {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub relevant_files: Vec<String>,
    /// One-based positions of earlier tasks in this same plan.
    #[serde(default)]
    pub depends_on: Vec<i64>,
    #[serde(default)]
    pub estimate_minutes: i64,
    /// Relative size on the Fibonacci scale; 0 when the agent gave none.
    #[serde(default)]
    pub story_points: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    #[serde(default)]
    pub summary: String,
    /// What the repository already has, as evidence the analysis actually looked.
    #[serde(default)]
    pub existing: Vec<String>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<PlannedTask>,
}

/// What the UI shows while an analysis is in flight.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStep {
    pub id: String,
    pub label: String,
    pub done: bool,
}

#[derive(Debug)]
pub enum AnalysisEvent {
    /// A unit of work started, keyed so it can be completed later.
    StepStarted { id: String, label: String },
    StepFinished { id: String },
}

pub struct AnalysisOutcome {
    pub plan: Plan,
    /// The agent's own conversation handle, kept so later phases can resume it.
    pub thread_id: String,
}

/// The category slugs an agent may choose from, guarding against a caller that
/// somehow has none: an empty `enum` would make the schema unsatisfiable.
fn choices(categories: &[String]) -> Vec<String> {
    if categories.is_empty() {
        vec!["task".to_string()]
    } else {
        categories.to_vec()
    }
}

/// The JSON Schema the agent's final answer is constrained to. The categories
/// are whatever the developer keeps in Settings, not a fixed list.
pub fn output_schema(categories: &[String]) -> Value {
    let categories = choices(categories);
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "existing", "missing", "tasks"],
        "properties": {
            "summary": { "type": "string" },
            "existing": { "type": "array", "items": { "type": "string" } },
            "missing": { "type": "array", "items": { "type": "string" } },
            "tasks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "title", "description", "category", "acceptanceCriteria",
                        "relevantFiles", "dependsOn", "estimateMinutes", "storyPoints"
                    ],
                    "properties": {
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "category": { "type": "string", "enum": categories },
                        "acceptanceCriteria": { "type": "array", "items": { "type": "string" } },
                        "relevantFiles": { "type": "array", "items": { "type": "string" } },
                        "dependsOn": { "type": "array", "items": { "type": "integer" } },
                        "estimateMinutes": { "type": "integer" },
                        "storyPoints": { "type": "integer", "enum": crate::models::STORY_POINTS }
                    }
                }
            }
        }
    })
}

pub fn prompt(goal: &str, max_tasks: usize) -> String {
    let scale = story_points_text();
    format!(
        "You are planning development work inside an existing repository.\n\n\
         GOAL\n{goal}\n\n\
         Inspect the repository to work out what already exists that serves this goal, \
         and what is still missing. Read files and run read-only commands only; do not \
         modify anything.\n\n\
         Then produce an ordered implementation plan.\n\n\
         Rules:\n\
         - Ground `existing` in things you actually found. Name the file or module.\n\
         - `missing` is what the goal needs that is not there yet.\n\
         - At most {max_tasks} tasks. Each one should be a single sitting of work.\n\
         - `relevantFiles` are repository-relative paths, existing or to be created.\n\
         - `dependsOn` holds 1-based positions of earlier tasks in your own list.\n\
         - `acceptanceCriteria` are checkable statements, not restatements of the title.\n\
         - `estimateMinutes` is a realistic whole number of minutes.\n\
         - `storyPoints` sizes the task against the others on the Fibonacci \
           scale ({scale}): complexity and unknowns, not hours."
    )
}

/// The scale as it appears in a prompt.
fn story_points_text() -> String {
    crate::models::STORY_POINTS
        .iter()
        .map(|point| point.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// One question to put to an agent, and the shape its answer has to take.
///
/// Planning a goal and importing someone else's task list ask for the same
/// thing — an ordered list of tasks — so they travel through the same read-only
/// turn, the same schema, and the same sanitising. Only the question differs.
pub struct PlanRequest {
    pub prompt: String,
    pub categories: Vec<String>,
    pub max_tasks: usize,
}

impl PlanRequest {
    /// Plan a goal against the repository.
    pub fn goal(goal: &str, categories: Vec<String>) -> Self {
        Self {
            prompt: prompt(goal, MAX_TASKS),
            categories,
            max_tasks: MAX_TASKS,
        }
    }

    /// The JSON Schema an answer is constrained to, for agents that can.
    pub fn schema(&self) -> Value {
        output_schema(&self.categories)
    }

    /// The same shape spelled out in prose, for agents that cannot.
    pub fn spelled_out(&self) -> String {
        format!(
            "{}{}",
            self.prompt,
            json_instruction(&self.categories, self.max_tasks)
        )
    }

    pub fn parse(&self, text: &str) -> Result<Plan> {
        parse_plan(text, &self.categories, self.max_tasks)
    }
}

/// Strips a fenced code block if the model wrapped its JSON in one.
fn unwrap_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let body = rest.split_once('\n').map(|(_, body)| body).unwrap_or(rest);
    body.strip_suffix("```").unwrap_or(body).trim()
}

/// Finds the outermost balanced `{...}` span, for an answer that wrapped its
/// JSON in a sentence or two. String literals are respected so a brace inside
/// a description cannot end the span early.
fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let bytes = text.as_bytes();
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);

    for index in start..bytes.len() {
        let byte = bytes[index];
        if in_string {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=index]);
                }
            }
            _ => {}
        }
    }
    None
}

/// The shape to ask for when an agent has no schema-enforcing mode.
pub fn json_instruction(categories: &[String], max_tasks: usize) -> String {
    let scale = story_points_text();
    let allowed = choices(categories)
        .iter()
        .map(|slug| format!("\"{slug}\""))
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "\n\nReply with a single JSON object and nothing else — no prose, no code fence:\n\
         {{\n  \"summary\": string,\n  \"existing\": string[],\n  \"missing\": string[],\n  \
         \"tasks\": [{{ \"title\": string, \"description\": string, \"category\": {allowed}, \
         \"acceptanceCriteria\": string[], \"relevantFiles\": string[], \
         \"dependsOn\": number[], \"estimateMinutes\": number, \"storyPoints\": number }}]\n}}\n\
         `storyPoints` is one of {scale}.\n\
         Keep it compact: at most {max_tasks} tasks, at most 4 acceptance criteria each."
    )
}

/// Parses and sanitises a plan. The output schema makes the shape reliable;
/// this makes the *contents* safe to store and show.
pub fn parse_plan(text: &str, categories: &[String], max_tasks: usize) -> Result<Plan> {
    let unfenced = unwrap_fence(text);
    // Try the whole answer first, then the JSON hiding inside it.
    let plan: Plan = serde_json::from_str(unfenced)
        .or_else(|first| match extract_json_object(unfenced) {
            Some(object) => serde_json::from_str(object),
            None => Err(first),
        })
        .map_err(|error| Error::invalid(format!("the agent returned an unreadable plan: {error}")))?;
    sanitise(plan, categories, max_tasks)
}

/// Parses any JSON object out of an agent's answer, tolerating a code fence or
/// a sentence wrapped around it.
pub fn parse_json_object<T: serde::de::DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    let unfenced = unwrap_fence(text);
    serde_json::from_str(unfenced).or_else(|first| match extract_json_object(unfenced) {
        Some(object) => serde_json::from_str(object),
        None => Err(first),
    })
}

/// Same, for an agent that hands back an already-parsed object.
pub fn plan_from_value(value: &Value, categories: &[String], max_tasks: usize) -> Result<Plan> {
    let plan: Plan = serde_json::from_value(value.clone())
        .map_err(|error| Error::invalid(format!("the agent returned an unreadable plan: {error}")))?;
    sanitise(plan, categories, max_tasks)
}

fn sanitise(mut plan: Plan, categories: &[String], max_tasks: usize) -> Result<Plan> {
    let allowed = choices(categories);
    plan.summary = plan.summary.trim().to_string();
    plan.existing = clean_list(plan.existing);
    plan.missing = clean_list(plan.missing);
    plan.tasks.truncate(max_tasks);

    let mut tasks = Vec::with_capacity(plan.tasks.len());
    for (index, mut task) in plan.tasks.into_iter().enumerate() {
        task.title = task.title.trim().to_string();
        if task.title.is_empty() {
            continue;
        }
        task.description = task.description.trim().to_string();
        // A category the developer has since renamed away or never had falls
        // back to the first one rather than failing the whole plan.
        if !allowed.iter().any(|slug| *slug == task.category) {
            task.category = allowed[0].clone();
        }
        task.acceptance_criteria = clean_list(task.acceptance_criteria);
        task.relevant_files = clean_list(task.relevant_files);
        task.estimate_minutes = task.estimate_minutes.clamp(0, 480);
        task.story_points = crate::models::nearest_story_points(task.story_points).unwrap_or(0);
        // Only backward references survive, so a plan can never cycle.
        let position = index as i64 + 1;
        task.depends_on = task
            .depends_on
            .into_iter()
            .filter(|other| *other >= 1 && *other < position)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        tasks.push(task);
    }
    plan.tasks = tasks;

    if plan.tasks.is_empty() {
        return Err(Error::invalid("the agent did not produce any tasks for that goal."));
    }
    Ok(plan)
}

fn clean_list(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// Agents wrap provider failures in their own JSON. Dig out the sentence a
/// developer can act on rather than showing them a blob.
pub fn unwrap_error_body(message: &str) -> String {
    let mut current = message.trim().to_string();
    for _ in 0..3 {
        if !current.starts_with('{') {
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(&current) else {
            break;
        };
        let inner = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .or_else(|| value.get("message").and_then(Value::as_str));
        match inner {
            Some(text) if text.trim() != current => current = text.trim().to_string(),
            _ => break,
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the seeded categories table hands the planner.
    fn categories() -> Vec<String> {
        vec!["task".to_string(), "bug".to_string()]
    }

    const SAMPLE: &str = r#"{
      "summary": "Invoices exist but cannot be downloaded.",
      "existing": ["Invoice model in src/models/invoice.ts", "  "],
      "missing": ["PDF generation"],
      "tasks": [
        {
          "title": "  Create invoice PDF service  ",
          "description": "Render an invoice to a PDF buffer.",
          "category": "bug",
          "acceptanceCriteria": ["Generates a valid PDF", ""],
          "relevantFiles": ["src/services/invoice.ts"],
          "dependsOn": [],
          "estimateMinutes": 45
        },
        {
          "title": "Add download endpoint",
          "description": "Serve the PDF.",
          "category": "not-a-category",
          "acceptanceCriteria": [],
          "relevantFiles": [],
          "dependsOn": [1, 2, 9],
          "estimateMinutes": 10000
        }
      ]
    }"#;

    #[test]
    fn a_plan_is_trimmed_and_normalised() {
        let plan = parse_plan(SAMPLE, &categories(), MAX_TASKS).unwrap();
        assert_eq!(plan.tasks[0].title, "Create invoice PDF service");
        assert_eq!(plan.existing, vec!["Invoice model in src/models/invoice.ts"]);
        assert_eq!(plan.tasks[0].acceptance_criteria, vec!["Generates a valid PDF"]);
    }

    #[test]
    fn an_unknown_category_falls_back_rather_than_failing_the_plan() {
        assert_eq!(parse_plan(SAMPLE, &categories(), MAX_TASKS).unwrap().tasks[1].category, "task");
    }

    #[test]
    fn the_schema_offers_exactly_the_developers_own_categories() {
        let schema = output_schema(&["design".to_string(), "chore".to_string()]);
        assert_eq!(
            schema["properties"]["tasks"]["items"]["properties"]["category"]["enum"],
            serde_json::json!(["design", "chore"])
        );
    }

    #[test]
    fn dependencies_only_point_backwards_so_a_plan_cannot_cycle() {
        // Task 2 asked to depend on 1, itself, and a task that does not exist.
        assert_eq!(parse_plan(SAMPLE, &categories(), MAX_TASKS).unwrap().tasks[1].depends_on, vec![1]);
    }

    #[test]
    fn a_wild_estimate_is_clamped() {
        assert_eq!(parse_plan(SAMPLE, &categories(), MAX_TASKS).unwrap().tasks[1].estimate_minutes, 480);
    }

    #[test]
    fn json_wrapped_in_a_code_fence_still_parses() {
        assert!(parse_plan(&format!("```json\n{SAMPLE}\n```"), &categories(), MAX_TASKS).is_ok());
    }

    #[test]
    fn an_already_parsed_object_takes_the_same_path() {
        let value: Value = serde_json::from_str(SAMPLE).unwrap();
        let from_value = plan_from_value(&value, &categories(), MAX_TASKS).unwrap();
        assert_eq!(from_value.tasks[0].title, "Create invoice PDF service");
        assert_eq!(from_value.tasks[1].depends_on, vec![1]);
    }

    #[test]
    fn json_buried_in_prose_is_still_found() {
        let wrapped = format!("Here is the plan you asked for:\n\n{SAMPLE}\n\nHope that helps!");
        let plan = parse_plan(&wrapped, &categories(), MAX_TASKS).unwrap();
        assert_eq!(plan.tasks[0].title, "Create invoice PDF service");
    }

    #[test]
    fn a_brace_inside_a_string_does_not_end_the_object_early() {
        let text = r#"note {"summary":"uses {braces} inside","existing":[],"missing":[],"tasks":[{"title":"T","description":"","category":"backend","acceptanceCriteria":[],"relevantFiles":[],"dependsOn":[],"estimateMinutes":5}]} done"#;
        let plan = parse_plan(text, &categories(), MAX_TASKS).unwrap();
        assert_eq!(plan.summary, "uses {braces} inside");
    }

    #[test]
    fn text_with_no_json_at_all_is_an_error_not_a_panic() {
        assert!(parse_plan("I could not work that out, sorry.", &categories(), MAX_TASKS).is_err());
    }

    #[test]
    fn a_plan_with_no_usable_tasks_is_an_error() {
        assert!(parse_plan(
            r#"{"summary":"","existing":[],"missing":[],"tasks":[]}"#,
            &categories(),
            MAX_TASKS,
        ).is_err());
    }

    #[test]
    fn the_output_schema_forbids_stray_fields() {
        let schema = output_schema(&categories());
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["tasks"]["items"]["additionalProperties"], false);
    }

    #[test]
    fn a_provider_json_error_body_is_unwrapped_to_its_sentence() {
        let raw = r#"{"type":"error","status":400,"error":{"type":"invalid_request_error","message":"The 'gpt-5.6-sol' model requires a newer version of Codex."}}"#;
        let unwrapped = unwrap_error_body(raw);
        assert!(unwrapped.starts_with("The 'gpt-5.6-sol' model requires"));
        assert!(!unwrapped.contains('{'));
    }

    #[test]
    fn a_plain_message_survives_unwrapping_untouched() {
        assert_eq!(unwrap_error_body("  something broke  "), "something broke");
    }

    #[test]
    fn unparseable_json_is_left_alone_rather_than_lost() {
        assert_eq!(unwrap_error_body("{not json"), "{not json");
    }

    #[test]
    fn the_json_instruction_names_every_field_the_parser_needs() {
        let text = json_instruction(&categories(), MAX_TASKS);
        for field in [
            "summary", "existing", "missing", "tasks", "title", "description",
            "category", "acceptanceCriteria", "relevantFiles", "dependsOn",
            "estimateMinutes",
        ] {
            assert!(text.contains(field), "instruction is missing {field}");
        }
        // The cap the prompt states must match the one the parser enforces.
        assert!(text.contains(&MAX_TASKS.to_string()));
    }
}
