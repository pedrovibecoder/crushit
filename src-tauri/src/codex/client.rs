//! A JSON-RPC client for `codex app-server` over its stdio JSONL transport.
//!
//! One long-lived server process is shared by the app: starting it costs a
//! couple of seconds, and later phases reuse it to resume task threads.

use crate::error::{Error, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A server-to-client message. Params are left as JSON so callers can pick out
/// only the fields they need from a protocol with 60-plus notification types.
#[derive(Clone, Debug)]
pub struct Notification {
    pub method: String,
    pub params: Value,
}

type Pending = Arc<Mutex<HashMap<i64, Sender<std::result::Result<Value, String>>>>>;
type Subscribers = Arc<Mutex<Vec<Sender<Notification>>>>;

struct Session {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    next_id: AtomicI64,
}

#[derive(Default)]
pub struct CodexClient {
    session: Mutex<Option<Session>>,
    pending: Pending,
    subscribers: Subscribers,
}

fn write_message(stdin: &Arc<Mutex<ChildStdin>>, message: &Value) -> Result<()> {
    let mut guard = stdin.lock().unwrap_or_else(|e| e.into_inner());
    let line = format!("{message}\n");
    guard.write_all(line.as_bytes())?;
    guard.flush()?;
    Ok(())
}

impl CodexClient {
    /// True while a server process is alive and usable.
    pub fn is_running(&self) -> bool {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_mut() {
            Some(session) => match session.child.try_wait() {
                Ok(None) => true,
                // Reaped or errored: drop it so the next call starts a fresh one.
                _ => {
                    *guard = None;
                    false
                }
            },
            None => false,
        }
    }

    /// Notifications are fanned out to every live subscriber.
    pub fn subscribe(&self) -> Receiver<Notification> {
        let (tx, rx) = mpsc::channel();
        self.subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(tx);
        rx
    }

    /// Starts and handshakes the server if it is not already running.
    pub fn ensure_started(&self, binary: &Path, client_version: &str) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }

        let mut child = Command::new(binary)
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // The server logs progress chatter to stderr; it is not protocol.
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                Error::invalid(format!("could not start `codex app-server`: {error}"))
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::invalid("codex app-server produced no stdout"))?;
        let stdin = Arc::new(Mutex::new(
            child
                .stdin
                .take()
                .ok_or_else(|| Error::invalid("codex app-server accepted no stdin"))?,
        ));

        spawn_reader(stdout, self.pending.clone(), self.subscribers.clone(), stdin.clone());

        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(Session {
            child,
            stdin,
            next_id: AtomicI64::new(1),
        });

        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "blitzit",
                    "title": "Blitzit",
                    "version": client_version,
                }
            }),
            Duration::from_secs(30),
        )?;
        self.notify("initialized", json!({}))?;
        Ok(())
    }

    pub fn notify(&self, method: &str, params: impl Serialize) -> Result<()> {
        let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let session = guard
            .as_ref()
            .ok_or_else(|| Error::invalid("codex app-server is not running"))?;
        let message = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": serde_json::to_value(params)?,
        });
        write_message(&session.stdin, &message)
    }

    pub fn request(
        &self,
        method: &str,
        params: impl Serialize,
        timeout: Duration,
    ) -> Result<Value> {
        let (id, stdin) = {
            let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
            let session = guard
                .as_ref()
                .ok_or_else(|| Error::invalid("codex app-server is not running"))?;
            (
                session.next_id.fetch_add(1, Ordering::SeqCst),
                session.stdin.clone(),
            )
        };

        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, tx);

        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": serde_json::to_value(params)?,
        });
        if let Err(error) = write_message(&stdin, &message) {
            self.pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
            return Err(error);
        }

        match rx.recv_timeout(timeout) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(message)) => Err(Error::invalid(message)),
            Err(_) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                Err(Error::invalid(format!("Codex did not answer `{method}` in time")))
            }
        }
    }

    /// Stops the server. Safe to call when nothing is running.
    pub fn shutdown(&self) {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut session) = guard.take() {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Reads the JSONL stream: resolves replies, fans out notifications, and
/// answers server-initiated requests so the turn never stalls waiting on us.
fn spawn_reader(
    stdout: std::process::ChildStdout,
    pending: Pending,
    subscribers: Subscribers,
    stdin: Arc<Mutex<ChildStdin>>,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };

            let id = message.get("id").and_then(Value::as_i64);
            let method = message.get("method").and_then(Value::as_str);

            match (id, method) {
                // A reply to something we sent.
                (Some(id), None) => {
                    let sender = pending
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&id);
                    if let Some(sender) = sender {
                        let outcome = match message.get("error") {
                            Some(error) => Err(error
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("Codex returned an error")
                                .to_string()),
                            None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                        };
                        let _ = sender.send(outcome);
                    }
                }
                // A request from the server, which must be answered.
                (Some(id), Some(method)) => {
                    let response = answer_server_request(id, method);
                    let _ = write_message(&stdin, &response);
                }
                // A plain notification.
                (None, Some(method)) => {
                    let notification = Notification {
                        method: method.to_string(),
                        params: message.get("params").cloned().unwrap_or(Value::Null),
                    };
                    subscribers
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .retain(|sender| sender.send(notification.clone()).is_ok());
                }
                (None, None) => {}
            }
        }

        // The process ended: unblock anyone still waiting on a reply.
        for (_, sender) in pending.lock().unwrap_or_else(|e| e.into_inner()).drain() {
            let _ = sender.send(Err("Codex app-server stopped".to_string()));
        }
        subscribers.lock().unwrap_or_else(|e| e.into_inner()).clear();
    });
}

/// Blitzit never silently grants Codex more access than the turn was started
/// with, so approval requests are declined until a phase implements the UI.
fn answer_server_request(id: i64, method: &str) -> Value {
    match method {
        "item/commandExecution/requestApproval"
        | "item/fileChange/requestApproval"
        | "execCommandApproval"
        | "applyPatchApproval" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "decision": "decline" },
        }),
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("blitzit does not handle {method}") },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_requests_are_declined_not_ignored() {
        let response = answer_server_request(7, "item/fileChange/requestApproval");
        assert_eq!(response["id"], 7);
        assert_eq!(response["result"]["decision"], "decline");
    }

    #[test]
    fn unknown_server_requests_get_an_error_so_the_turn_never_stalls() {
        let response = answer_server_request(9, "something/new");
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn a_client_with_no_session_refuses_to_send() {
        let client = CodexClient::default();
        assert!(!client.is_running());
        assert!(client.notify("initialized", json!({})).is_err());
    }
}
