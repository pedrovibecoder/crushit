//! Proposing acceptance criteria for a task the developer wrote by hand.
//!
//! A task typed in a hurry rarely says when it is finished. This asks the agent
//! for the checkable statements that would settle it — the same list the
//! verification pass later reads — so writing them is not the thing that stops
//! a task from being written at all.

use crate::error::{Error, Result};
use crate::models::Task;
use serde::Deserialize;
use serde_json::{json, Value};

/// More than this stops being a definition of done and starts being the work.
pub const MAX_CRITERIA: usize = 8;

/// A criterion long enough to be a paragraph is not checkable.
const MAX_LENGTH: usize = 160;

#[derive(Deserialize, Debug, Default)]
pub struct Suggestion {
    #[serde(default)]
    pub criteria: Vec<String>,
}

pub fn output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["criteria"],
        "properties": {
            "criteria": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

pub fn prompt(task: &Task) -> String {
    let description = task
        .description
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("(none given)");
    let existing = if task.criteria.is_empty() {
        "The task has no acceptance criteria yet.".to_string()
    } else {
        format!(
            "It already has these, which you must not repeat or reword:\n{}",
            task.criteria
                .iter()
                .map(|criterion| format!("- {}", criterion.text))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    format!(
        "You are writing the acceptance criteria for one task in an existing \
         repository.\n\n\
         TASK\n{title}\n\n\
         DESCRIPTION\n{description}\n\n\
         {existing}\n\n\
         Look at the repository for how work like this is already done — the \
         conventions, the tests, the surrounding code — and read only; do not \
         modify anything.\n\n\
         Rules:\n\
         - Each criterion is one checkable statement about the finished work. \
           Someone reading it should be able to say yes or no.\n\
         - Write what must be true, not what to do. \"Expired token shows an \
           error\", not \"handle expired tokens\".\n\
         - Cover the failure cases and the tests, not only the happy path.\n\
         - Ground them in this repository where you can, but do not invent \
           requirements the task does not imply.\n\
         - At most {MAX_CRITERIA}, fewer when the task is small.\n\
         - No numbering, no leading dashes, no restating the task title.",
        title = task.title,
    )
}

/// The same shape in prose, for an agent with no schema-enforcing mode.
pub fn json_instruction() -> String {
    format!(
        "\n\nReply with a single JSON object and nothing else — no prose, no code \
         fence:\n{{ \"criteria\": string[] }}\n\
         At most {MAX_CRITERIA} entries."
    )
}

/// Parses and tidies a suggestion. Anything unusable is dropped rather than
/// shown, because a criterion nobody can check is worse than one fewer.
pub fn parse(text: &str) -> Result<Vec<String>> {
    let suggestion: Suggestion = crate::plan::parse_json_object(text).map_err(|error| {
        Error::invalid(format!("the agent returned unreadable criteria: {error}"))
    })?;
    Ok(clean(suggestion.criteria))
}

/// Same, for an agent that hands back an already-parsed object.
pub fn from_value(value: &Value) -> Result<Vec<String>> {
    let suggestion: Suggestion = serde_json::from_value(value.clone()).map_err(|error| {
        Error::invalid(format!("the agent returned unreadable criteria: {error}"))
    })?;
    Ok(clean(suggestion.criteria))
}

fn clean(raw: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    raw.into_iter()
        .map(|criterion| {
            // Models like to number their lists however firmly they are asked not to.
            criterion
                .trim()
                .trim_start_matches(|character: char| {
                    character.is_ascii_digit() || matches!(character, '.' | ')' | '-' | '*' | ' ')
                })
                .trim()
                .to_string()
        })
        .filter(|criterion| !criterion.is_empty() && criterion.len() <= MAX_LENGTH)
        .filter(|criterion| seen.insert(criterion.to_lowercase()))
        .take(MAX_CRITERIA)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AcceptanceCriterion, TaskStatus};

    fn a_task(criteria: Vec<&str>) -> Task {
        Task {
            id: 1,
            project_id: 1,
            goal_id: None,
            title: "Add password reset".into(),
            description: Some("Let a customer set a new password by email.".into()),
            category: "task".into(),
            story_points: Some(3),
            planned_for: "2026-08-19".into(),
            status: TaskStatus::Ready,
            position: 0,
            estimate_minutes: Some(45),
            is_ai_generated: false,
            created_at: 0,
            updated_at: 0,
            completed_at: None,
            criteria: criteria
                .into_iter()
                .enumerate()
                .map(|(index, text)| AcceptanceCriterion {
                    id: index as i64 + 1,
                    task_id: 1,
                    text: text.to_string(),
                    is_met: false,
                    position: index as i64,
                })
                .collect(),
            files: Vec::new(),
            depends_on: Vec::new(),
            focus_seconds: 0,
            focus_sessions: 0,
        }
    }

    #[test]
    fn the_prompt_carries_the_task_and_forbids_changing_anything() {
        let text = prompt(&a_task(vec![]));
        assert!(text.contains("Add password reset"));
        assert!(text.contains("Let a customer set a new password"));
        assert!(text.contains("do not modify"));
        assert!(text.contains("no acceptance criteria yet"));
    }

    #[test]
    fn criteria_already_written_are_shown_so_they_are_not_repeated() {
        let text = prompt(&a_task(vec!["Reset email is sent"]));
        assert!(text.contains("Reset email is sent"));
        assert!(text.contains("must not repeat"));
    }

    #[test]
    fn a_numbered_list_is_stripped_back_to_the_statements() {
        let parsed = parse(r#"{"criteria":["1. User can request a reset","- Reset email is sent"]}"#)
            .unwrap();
        assert_eq!(parsed, vec!["User can request a reset", "Reset email is sent"]);
    }

    #[test]
    fn blank_repeated_and_rambling_entries_are_dropped() {
        let long = "x".repeat(MAX_LENGTH + 1);
        let raw = format!(
            r#"{{"criteria":["Password can be changed","  ","password can be changed","{long}"]}}"#
        );
        assert_eq!(parse(&raw).unwrap(), vec!["Password can be changed"]);
    }

    #[test]
    fn a_long_list_is_cut_to_something_a_person_would_read() {
        let many: Vec<String> = (0..20).map(|index| format!("\"Criterion {index}\"")).collect();
        let raw = format!(r#"{{"criteria":[{}]}}"#, many.join(","));
        assert_eq!(parse(&raw).unwrap().len(), MAX_CRITERIA);
    }

    #[test]
    fn json_wrapped_in_prose_or_a_fence_still_parses() {
        let fenced = "```json\n{\"criteria\":[\"Tests cover failure cases\"]}\n```";
        assert_eq!(parse(fenced).unwrap(), vec!["Tests cover failure cases"]);
    }

    #[test]
    fn an_answer_with_no_json_is_an_error_rather_than_an_empty_list() {
        assert!(parse("I would need to see the code first.").is_err());
    }
}
