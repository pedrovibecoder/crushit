//! Slack: the messages waiting on you, and a reply drafted for you to send.
//!
//! Everything here runs in the backend rather than the webview, because the
//! token must never reach a page. The app posts as *you* — a user token, not a
//! bot — so nothing is ever sent without an explicit press of Send, and the
//! command that sends is the only one that writes to Slack at all.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://slack.com/api";
const TIMEOUT: Duration = Duration::from_secs(20);

/// How far back a conversation is read for context and for the draft.
const HISTORY: usize = 12;

/// Who the token belongs to, so the app can say whose name it would post under.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlackAccount {
    pub user_id: String,
    pub user: String,
    pub team: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlackMessage {
    pub user: String,
    /// The display name, when Slack gave one; the id otherwise.
    pub author: String,
    pub text: String,
    /// Slack's own message id, which is also its timestamp.
    pub ts: String,
    /// True when this one is yours.
    pub is_mine: bool,
}

/// A conversation whose last word is not yours.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WaitingConversation {
    pub id: String,
    /// Who you are talking to, as you would name them.
    pub with: String,
    pub messages: Vec<SlackMessage>,
    /// Seconds since the message that is waiting.
    pub waiting_seconds: i64,
}

#[derive(Deserialize)]
struct AuthTest {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    team: String,
}

/// Slack answers 200 with `ok: false` for real failures, so the body decides.
fn check(body: &Value) -> Result<()> {
    if body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Ok(());
    }
    let error = body
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("slack refused the request");
    Err(Error::invalid(match error {
        "invalid_auth" | "not_authed" | "token_revoked" => {
            "That Slack token was refused. Paste a current one in Settings.".to_string()
        }
        "missing_scope" => {
            "The Slack token is missing a permission. It needs im:history, users:read \
             and chat:write."
                .to_string()
        }
        "ratelimited" => "Slack is rate limiting this token. Try again shortly.".to_string(),
        other => format!("Slack said: {other}"),
    }))
}

fn get(token: &str, method: &str, query: &[(&str, &str)]) -> Result<Value> {
    let mut request = ureq::get(&format!("{API}/{method}"))
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .header("Authorization", &format!("Bearer {token}"));
    for (key, value) in query {
        request = request.query(*key, *value);
    }
    let body: Value = request
        .call()
        .map_err(|error| Error::invalid(format!("could not reach Slack: {error}")))?
        .body_mut()
        .read_json()
        .map_err(|error| Error::invalid(format!("Slack sent something unreadable: {error}")))?;
    check(&body)?;
    Ok(body)
}

fn post(token: &str, method: &str, payload: Value) -> Result<Value> {
    let body: Value = ureq::post(&format!("{API}/{method}"))
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(payload)
        .map_err(|error| Error::invalid(format!("could not reach Slack: {error}")))?
        .body_mut()
        .read_json()
        .map_err(|error| Error::invalid(format!("Slack sent something unreadable: {error}")))?;
    check(&body)?;
    Ok(body)
}

/// Who the token belongs to. Also the check that it works at all.
pub fn account(token: &str) -> Result<SlackAccount> {
    let body = get(token, "auth.test", &[])?;
    let auth: AuthTest = serde_json::from_value(body)?;
    if !auth.ok {
        return Err(Error::invalid(
            auth.error.unwrap_or_else(|| "Slack refused the token".into()),
        ));
    }
    Ok(SlackAccount {
        user_id: auth.user_id,
        user: auth.user,
        team: auth.team,
    })
}

fn display_name(token: &str, user_id: &str) -> String {
    let profile = get(token, "users.info", &[("user", user_id)]).ok();
    profile
        .as_ref()
        .and_then(|body| body.get("user"))
        .and_then(|user| {
            user.get("profile")
                .and_then(|profile| {
                    profile
                        .get("display_name")
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty())
                        .or_else(|| profile.get("real_name").and_then(Value::as_str))
                })
                .or_else(|| user.get("name").and_then(Value::as_str))
        })
        .unwrap_or(user_id)
        .to_string()
}

/// Direct messages whose last message is not yours.
///
/// Deliberately only DMs for now: a message someone sent you directly is
/// unambiguously waiting on you, where a busy channel is not.
pub fn waiting(token: &str, me: &str) -> Result<Vec<WaitingConversation>> {
    let body = get(
        token,
        "users.conversations",
        &[("types", "im"), ("limit", "50"), ("exclude_archived", "true")],
    )?;
    let channels = body
        .get("channels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let now = crate::models::now();
    let mut waiting = Vec::new();
    for channel in channels {
        let Some(id) = channel.get("id").and_then(Value::as_str) else {
            continue;
        };
        // A DM you have never used has nothing waiting in it.
        if channel.get("is_user_deleted").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let messages = history(token, id, me)?;
        let Some(last) = messages.last() else { continue };
        if last.is_mine {
            continue;
        }
        let with = channel
            .get("user")
            .and_then(Value::as_str)
            .map(|user| display_name(token, user))
            .unwrap_or_else(|| "someone".to_string());
        let since = last.ts.split('.').next().and_then(|s| s.parse::<i64>().ok());
        waiting.push(WaitingConversation {
            id: id.to_string(),
            with,
            messages,
            waiting_seconds: since.map(|at| (now - at).max(0)).unwrap_or(0),
        });
    }
    // Longest wait first: that is the one costing someone the most.
    waiting.sort_by(|a, b| b.waiting_seconds.cmp(&a.waiting_seconds));
    Ok(waiting)
}

/// The tail of a conversation, oldest first.
pub fn history(token: &str, conversation: &str, me: &str) -> Result<Vec<SlackMessage>> {
    let limit = HISTORY.to_string();
    let body = get(
        token,
        "conversations.history",
        &[("channel", conversation), ("limit", &limit)],
    )?;
    let mut messages: Vec<SlackMessage> = body
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        // Joins, leaves and other housekeeping are not part of the conversation.
        .filter(|message| message.get("subtype").is_none())
        .filter_map(|message| {
            let user = message.get("user").and_then(Value::as_str)?.to_string();
            let text = message.get("text").and_then(Value::as_str)?.trim().to_string();
            if text.is_empty() {
                return None;
            }
            let is_mine = user == me;
            Some(SlackMessage {
                author: if is_mine { "You".to_string() } else { user.clone() },
                user,
                text,
                ts: message.get("ts").and_then(Value::as_str)?.to_string(),
                is_mine,
            })
        })
        .collect();
    // Slack returns newest first; a conversation reads the other way.
    messages.reverse();
    Ok(messages)
}

/// Sends a reply. The only call in this module that writes anything.
pub fn send(token: &str, conversation: &str, text: &str) -> Result<()> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::invalid("write something before sending"));
    }
    post(
        token,
        "chat.postMessage",
        json!({ "channel": conversation, "text": text, "as_user": true }),
    )?;
    Ok(())
}

// ------------------------------------------------------------------- drafting

/// The shape a drafted reply comes back in.
pub fn draft_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["reply"],
        "properties": { "reply": { "type": "string" } }
    })
}

/// What to ask the agent for. The conversation, plus what the developer is
/// actually working on, because that is usually what is being asked about.
pub fn draft_prompt(conversation: &WaitingConversation, tasks: &[crate::models::Task]) -> String {
    let transcript = conversation
        .messages
        .iter()
        .map(|message| {
            let who = if message.is_mine { "Me" } else { &conversation.with };
            format!("{who}: {}", message.text)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let work = if tasks.is_empty() {
        "(no tasks on the board)".to_string()
    } else {
        tasks
            .iter()
            .take(12)
            .map(|task| {
                let met = task.criteria.iter().filter(|c| c.is_met).count();
                format!(
                    "- {} [{}] {}/{} criteria met",
                    task.title,
                    task.status.as_str(),
                    met,
                    task.criteria.len()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "You are drafting a Slack reply that will be sent from my account, in my \
         name, to {with}. I will read it before it goes.\n\n\
         CONVERSATION (oldest first)\n{transcript}\n\n\
         WHAT I AM WORKING ON\n{work}\n\n\
         You may read this repository to answer accurately. Do not modify anything.\n\n\
         Rules:\n\
         - Answer the last message. Nothing else.\n\
         - Write as a person types in Slack: plain, direct, a couple of sentences. \
           No greeting unless the conversation has one, no sign-off, no bullet points \
           unless they genuinely help.\n\
         - Only state what you can support from the repository or the task list above. \
           If you cannot tell, say so plainly and say what you would need to check — \
           that is a better reply than a confident wrong one going out in my name.\n\
         - Never promise a date, commit to work, or agree to anything on my behalf. \
           Leave decisions to me.\n\
         - No emoji unless they used one first.",
        with = conversation.with,
    )
}

/// The same shape in prose, for an agent with no schema-enforcing mode.
pub fn draft_instruction() -> String {
    "\n\nReply with a single JSON object and nothing else — no prose, no code fence:\n\
     { \"reply\": string }"
        .to_string()
}

pub fn parse_draft(text: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Draft {
        #[serde(default)]
        reply: String,
    }
    let draft: Draft = crate::plan::parse_json_object(text)
        .map_err(|error| Error::invalid(format!("the agent returned an unreadable reply: {error}")))?;
    let reply = draft.reply.trim().to_string();
    if reply.is_empty() {
        return Err(Error::invalid("the agent had nothing to say"));
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_conversation() -> WaitingConversation {
        WaitingConversation {
            id: "D1".into(),
            with: "Priya".into(),
            waiting_seconds: 900,
            messages: vec![
                SlackMessage {
                    user: "U2".into(),
                    author: "Priya".into(),
                    text: "is the invoice PDF thing done?".into(),
                    ts: "1.0".into(),
                    is_mine: false,
                },
            ],
        }
    }

    #[test]
    fn the_draft_prompt_carries_the_conversation_and_the_board() {
        let prompt = draft_prompt(&a_conversation(), &[]);
        assert!(prompt.contains("is the invoice PDF thing done?"));
        assert!(prompt.contains("Priya"));
        assert!(prompt.contains("no tasks on the board"));
    }

    #[test]
    fn the_draft_prompt_refuses_to_commit_on_my_behalf() {
        let prompt = draft_prompt(&a_conversation(), &[]);
        assert!(prompt.contains("Never promise a date"));
        assert!(prompt.contains("Do not modify anything"));
        assert!(prompt.contains("I will read it before it goes"));
    }

    #[test]
    fn a_drafted_reply_is_pulled_out_of_the_answer() {
        assert_eq!(
            parse_draft(r#"{"reply":"Not yet — the download endpoint is left."}"#).unwrap(),
            "Not yet — the download endpoint is left."
        );
    }

    #[test]
    fn a_fenced_answer_still_parses() {
        let fenced = "```json\n{\"reply\":\"Nearly, one task to go.\"}\n```";
        assert_eq!(parse_draft(fenced).unwrap(), "Nearly, one task to go.");
    }

    #[test]
    fn an_empty_draft_is_an_error_rather_than_an_empty_message() {
        assert!(parse_draft(r#"{"reply":"   "}"#).is_err());
    }

    #[test]
    fn slack_failures_are_explained_rather_than_passed_through() {
        let refused = check(&json!({ "ok": false, "error": "invalid_auth" })).unwrap_err();
        assert!(refused.to_string().contains("Settings"), "{refused}");
        let scope = check(&json!({ "ok": false, "error": "missing_scope" })).unwrap_err();
        assert!(scope.to_string().contains("chat:write"), "{scope}");
    }

    #[test]
    fn a_successful_body_passes() {
        assert!(check(&json!({ "ok": true })).is_ok());
    }

    #[test]
    fn an_empty_reply_is_never_sent() {
        assert!(send("x", "D1", "   ").is_err());
    }
}
