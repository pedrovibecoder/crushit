//! Turning a handful of picked tasks into something an agent can act on.
//!
//! The list already knows what the work is; what it cannot say is what the
//! work *amounts to* — which parts of the repository it lands in, what order it
//! wants to be done in, and what someone should be careful of. That is a
//! reading job, so it is handed to the agent with the repository in front of
//! it, and what comes back is prose rather than a schema: this is written to be
//! read by a person about to start, and pasted to one who is not.

use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{json, Value};

/// Long enough to be worth reading, short enough to be read.
const MAX_WORDS: usize = 400;

#[derive(Deserialize, Debug, Default)]
struct Answer {
    #[serde(default)]
    summary: String,
}

pub fn output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary"],
        "properties": { "summary": { "type": "string" } }
    })
}

/// `context` is the same text the developer is looking at, so what the agent
/// was asked about and what the screen shows can never drift apart.
pub fn prompt(project: &str, context: &str) -> String {
    format!(
        "You are briefing an engineer who is about to start on the tasks below \
         in this repository ({project}).\n\n\
         {context}\n\
         Read the repository for how work like this is already done — the \
         conventions, the surrounding code, the tests. Read only; do not modify \
         anything.\n\n\
         Write the brief as prose an engineer can act on:\n\
         - What these tasks add up to, in a sentence or two. Say the thing \
           itself, not \"this set of tasks aims to\".\n\
         - Where in this repository the work lands: the files and modules that \
           will have to change, named as they actually are.\n\
         - The order to do them in, and which ones block which.\n\
         - What to be careful of: what this touches that is easy to break, and \
           anything the tasks assume but do not say.\n\n\
         Ground every claim in what is actually in the repository. Where the \
         tasks are unclear, say what is unclear rather than deciding for them. \
         Under {MAX_WORDS} words. No headings ceremony, no restating the task \
         list back."
    )
}

/// The same shape in prose, for an agent with no schema-enforcing mode.
pub fn json_instruction() -> String {
    "\n\nReply with a single JSON object and nothing else — no prose outside it, \
     no code fence:\n{ \"summary\": string }\n\
     Newlines inside the string are fine."
        .to_string()
}

/// Pulls the brief out of the agent's reply.
///
/// A reply that is simply the prose, with no JSON around it, is taken as the
/// brief: the answer is readable either way, and refusing it over its wrapper
/// would throw away a perfectly good one.
pub fn parse(text: &str) -> Result<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(Error::invalid("the agent returned an empty brief"));
    }
    match crate::plan::parse_json_object::<Answer>(trimmed) {
        Ok(answer) if !answer.summary.trim().is_empty() => Ok(answer.summary.trim().to_string()),
        _ if trimmed.starts_with('{') => {
            Err(Error::invalid("the agent returned an unreadable brief"))
        }
        _ => Ok(trimmed.to_string()),
    }
}

/// Same, for an agent that hands back an already-parsed object.
pub fn from_value(value: &Value) -> Result<String> {
    let answer: Answer = serde_json::from_value(value.clone())
        .map_err(|error| Error::invalid(format!("the agent returned an unreadable brief: {error}")))?;
    let summary = answer.summary.trim();
    if summary.is_empty() {
        return Err(Error::invalid("the agent returned an empty brief"));
    }
    Ok(summary.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_reply_is_unwrapped() {
        let brief = parse(r#"{"summary": "Adds the project number end to end."}"#).unwrap();
        assert_eq!(brief, "Adds the project number end to end.");
    }

    #[test]
    fn prose_with_no_json_around_it_is_taken_as_the_brief() {
        let brief = parse("  Adds the project number end to end.  ").unwrap();
        assert_eq!(brief, "Adds the project number end to end.");
    }

    #[test]
    fn an_empty_answer_is_refused_rather_than_shown() {
        assert!(parse("   ").is_err());
        assert!(parse(r#"{"summary": "  "}"#).is_err());
    }

    #[test]
    fn the_prompt_carries_the_context_the_developer_can_see() {
        let prompt = prompt("my-app", "# Context: my-app\n\n## 1. Add the field\n");
        assert!(prompt.contains("## 1. Add the field"));
        assert!(prompt.contains("my-app"));
    }
}
