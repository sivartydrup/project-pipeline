use crate::{
    AdapterError, AgentAdapter, CapabilityReport, EventKind, Health, NormalizedEvent,
    RecoveryReport, RecoveryState, Result, RunHandle, RunRequest,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use uuid::Uuid;

pub const TESTED_PI_VERSION: &str = "1.1.0";
const POLICY_SOURCE: &str = include_str!("pi_policy.ts");

struct Shared {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<String, mpsc::Sender<Value>>>,
    events: Mutex<mpsc::Receiver<Result<Value>>>,
    next_event: AtomicU64,
    session_id: Mutex<String>,
    policy_path: PathBuf,
}

/// One owned Pi RPC subprocess, bound to one isolated checkout and one model.
/// Dropping the adapter closes stdin and reaps the child.
#[derive(Clone)]
pub struct PiAdapter(Arc<Shared>);

impl PiAdapter {
    pub fn probe_executable(executable: &Path) -> CapabilityReport {
        let output = Self::base_command(executable).arg("--version").output();
        let (health, version, detail) = match output {
            Err(error) => (Health::Unavailable, None, error.to_string()),
            Ok(output) if !output.status.success() => {
                (Health::Unavailable, None, "Pi version probe failed".into())
            }
            Ok(output) => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if version == TESTED_PI_VERSION {
                    (
                        Health::Ready,
                        Some(version),
                        "tested Pi RPC version available".into(),
                    )
                } else {
                    (
                        Health::VersionMismatch,
                        Some(version),
                        "Pi version differs from tested protocol".into(),
                    )
                }
            }
        };
        let ready = health == Health::Ready;
        CapabilityReport {
            health,
            version,
            detail,
            can_resume: false,
            can_steer: ready,
            can_stop: ready,
            can_stream: ready,
            can_diff: false,
            can_reply_permissions: ready,
        }
    }

    fn base_command(executable: &Path) -> Command {
        if executable
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
        {
            let mut command = Command::new("node");
            command.arg(executable);
            command
        } else {
            Command::new(executable)
        }
    }

    pub fn launch(
        executable: &Path,
        checkout: &Path,
        run_id: &str,
        provider: &str,
        model: &str,
        environment: &[(&str, &str)],
    ) -> Result<Self> {
        if !executable.is_file() || !checkout.is_dir() {
            return Err(AdapterError::Unavailable(
                "Pi executable or checkout missing".into(),
            ));
        }
        if provider.trim().is_empty() || model.trim().is_empty() {
            return Err(AdapterError::Protocol(
                "explicit Pi provider and model required".into(),
            ));
        }
        if Uuid::parse_str(run_id).is_err() {
            return Err(AdapterError::Protocol("Pi run ID must be a UUID".into()));
        }
        let report = Self::probe_executable(executable);
        match report.health {
            Health::Ready => {}
            Health::VersionMismatch => {
                return Err(AdapterError::VersionMismatch {
                    expected: TESTED_PI_VERSION.into(),
                    found: report.version.unwrap_or_default(),
                });
            }
            Health::Unavailable => return Err(AdapterError::Unavailable(report.detail)),
        }
        let policy_path =
            std::env::temp_dir().join(format!("pipeline-pi-policy-{}.ts", Uuid::new_v4()));
        std::fs::write(&policy_path, POLICY_SOURCE)?;
        let mut command = Self::base_command(executable);
        command
            .current_dir(checkout)
            .args([
                "--mode",
                "rpc",
                "--provider",
                provider,
                "--model",
                model,
                "--no-extensions",
                "--no-mcp",
                "--no-skills",
                "--no-prompt-templates",
                "--no-context-files",
                "--no-approve",
                "--tools",
                "read,edit,write,powershell,bash",
                "--extension",
            ])
            .arg(&policy_path)
            .arg("--session-dir")
            .arg(std::env::temp_dir().join("project-pipeline-pi-sessions"))
            .arg("--session-id")
            .arg(run_id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in environment {
            command.env(key, value);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|error| {
            let _ = std::fs::remove_file(&policy_path);
            AdapterError::Unavailable(error.to_string())
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AdapterError::Protocol("Pi stdin unavailable".into()))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdapterError::Protocol("Pi stdout unavailable".into()))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| AdapterError::Protocol("Pi stderr unavailable".into()))?;
        let (event_tx, event_rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            events: Mutex::new(event_rx),
            next_event: AtomicU64::new(0),
            session_id: Mutex::new(String::new()),
            policy_path,
        });
        let reader_shared = Arc::downgrade(&shared);
        std::thread::spawn(move || {
            let mut decoder = crate::PiJsonlDecoder::new();
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => {
                        let _ = event_tx.send(decoder.finish().map(|_| json!({"type":"eof"})));
                        break;
                    }
                    Ok(n) => match decoder.push(&buf[..n]) {
                        Ok(records) => {
                            for record in records {
                                if record["type"] == "response"
                                    && let Some(id) = record["id"].as_str()
                                    && let Some(shared) = reader_shared.upgrade()
                                    && let Some(waiter) = shared.pending.lock().unwrap().remove(id)
                                {
                                    let _ = waiter.send(record);
                                    continue;
                                }
                                if event_tx.send(Ok(record)).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(error) => {
                            let _ = event_tx.send(Err(error));
                            break;
                        }
                    },
                    Err(error) => {
                        let _ = event_tx.send(Err(error.into()));
                        break;
                    }
                }
            }
        });
        std::thread::spawn(move || {
            let mut sink = std::io::sink();
            let _ = std::io::copy(&mut stderr, &mut sink);
        });
        let state = Self(shared);
        let response = state.command("get_state", json!({}))?;
        let session_id = response["data"]["sessionId"]
            .as_str()
            .ok_or_else(|| AdapterError::Protocol("Pi get_state omitted sessionId".into()))?
            .to_owned();
        *state.0.session_id.lock().unwrap() = session_id;
        let ready = state.next_event(Duration::from_secs(5))?;
        if !ready.is_some_and(|event| event.kind == EventKind::Connected) {
            return Err(AdapterError::Protocol(
                "Pi policy extension did not confirm startup".into(),
            ));
        }
        Ok(state)
    }

    pub fn command(&self, name: &str, fields: Value) -> Result<Value> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel();
        self.0.pending.lock().unwrap().insert(id.clone(), tx);
        let mut object = fields
            .as_object()
            .cloned()
            .ok_or_else(|| AdapterError::Protocol("RPC fields must be object".into()))?;
        object.insert("id".into(), json!(id));
        object.insert("type".into(), json!(name));
        let mut wire =
            serde_json::to_vec(&object).map_err(|e| AdapterError::Protocol(e.to_string()))?;
        wire.push(b'\n');
        let write = self.0.stdin.lock().unwrap().write_all(&wire);
        if let Err(error) = write {
            self.0.pending.lock().unwrap().remove(&id);
            return Err(error.into());
        }
        let response = match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(response) => response,
            Err(_) => {
                self.0.pending.lock().unwrap().remove(&id);
                return Err(AdapterError::Transport(format!(
                    "Pi {name} response timeout"
                )));
            }
        };
        if response["command"] != name || response["success"] != true {
            return Err(AdapterError::Protocol(
                response["error"]
                    .as_str()
                    .unwrap_or("Pi command failed")
                    .to_owned(),
            ));
        }
        Ok(response)
    }

    pub fn send_ui_response(&self, id: &str, confirmed: bool) -> Result<()> {
        let mut wire = serde_json::to_vec(
            &json!({"type":"extension_ui_response","id":id,"confirmed":confirmed}),
        )
        .map_err(|e| AdapterError::Protocol(e.to_string()))?;
        wire.push(b'\n');
        self.0.stdin.lock().unwrap().write_all(&wire)?;
        Ok(())
    }

    pub fn next_event(&self, timeout: Duration) -> Result<Option<NormalizedEvent>> {
        let received = self.0.events.lock().unwrap().recv_timeout(timeout);
        match received {
            Ok(Ok(value)) if value["type"] == "eof" => {
                Err(AdapterError::Unavailable("Pi process exited".into()))
            }
            Ok(Ok(value)) => {
                let n = self.0.next_event.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(normalize_pi_event(
                    &value,
                    &self.0.session_id.lock().unwrap(),
                    n,
                ))
            }
            Ok(Err(error)) => Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(AdapterError::Unavailable("Pi event stream closed".into()))
            }
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        if let Ok(child) = self.child.get_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(&self.policy_path);
    }
}

impl AgentAdapter for PiAdapter {
    fn probe(&self) -> CapabilityReport {
        let healthy = self
            .0
            .child
            .lock()
            .unwrap()
            .try_wait()
            .ok()
            .flatten()
            .is_none();
        CapabilityReport {
            health: if healthy {
                Health::Ready
            } else {
                Health::Unavailable
            },
            version: Some(TESTED_PI_VERSION.into()),
            detail: if healthy {
                "Pi RPC subprocess active"
            } else {
                "Pi RPC subprocess exited"
            }
            .into(),
            can_resume: false,
            can_steer: healthy,
            can_stop: healthy,
            can_stream: healthy,
            can_diff: false,
            can_reply_permissions: healthy,
        }
    }
    fn start(&self, request: RunRequest) -> Result<RunHandle> {
        Ok(RunHandle {
            run_id: request.run_id,
            session_id: self.0.session_id.lock().unwrap().clone(),
            checkout: request.checkout,
        })
    }
    fn events(
        &self,
        _run: &RunHandle,
        on_event: &mut dyn FnMut(NormalizedEvent) -> bool,
    ) -> Result<()> {
        loop {
            if let Some(event) = self.next_event(Duration::from_secs(1))?
                && !on_event(event)
            {
                return Ok(());
            }
        }
    }
    fn steer(&self, _run: &RunHandle, message: &str) -> Result<()> {
        self.command("prompt", json!({"message":message}))?;
        Ok(())
    }
    fn stop(&self, _run: &RunHandle) -> Result<()> {
        self.command("abort", json!({}))?;
        Ok(())
    }
    fn recover(&self, run: &RunHandle) -> Result<RecoveryReport> {
        let state = self.command("get_state", json!({}))?;
        if state["data"]["sessionId"] != run.session_id {
            return Err(AdapterError::Protocol("Pi session changed".into()));
        }
        Ok(RecoveryReport {
            session_id: run.session_id.clone(),
            state: if state["data"]["isStreaming"] == true {
                RecoveryState::Busy
            } else {
                RecoveryState::Idle
            },
            diff_files: 0,
        })
    }
    fn reply_permission(
        &self,
        _run: &RunHandle,
        permission_id: &str,
        allow_once: bool,
    ) -> Result<()> {
        self.send_ui_response(permission_id, allow_once)
    }
}

pub fn normalize_pi_event(
    value: &Value,
    session_id: &str,
    sequence: u64,
) -> Option<NormalizedEvent> {
    let kind = value["type"].as_str()?;
    let mut event = NormalizedEvent {
        external_id: Some(format!("pi:{sequence}")),
        session_id: session_id.into(),
        kind: EventKind::Unknown,
        summary: kind.into(),
        permission_id: None,
        permission_class: None,
        permission_target: None,
        permission_digest_material: None,
    };
    match kind {
        "extension_ui_request"
            if value["method"] == "notify"
                && value["message"] == "Project Pipeline policy gate ready" =>
        {
            event.kind = EventKind::Connected;
            event.summary = "Pi policy gate ready".into();
        }
        "extension_ui_request"
            if value["method"] == "notify"
                && value["message"] == "Project Pipeline model-call limit reached" =>
        {
            event.kind = EventKind::Error;
            event.summary = "Pi model-call limit reached before provider request".into();
        }
        "agent_start" | "turn_start" | "message_start" | "message_end" => {
            event.kind = EventKind::Progress
        }
        "agent_settled" => {
            event.kind = EventKind::Idle;
            event.summary = "Pi agent settled".into();
        }
        "tool_execution_start" | "tool_execution_end" => {
            event.kind = EventKind::Tool;
            event.summary = format!(
                "Pi {} {}",
                value["toolName"].as_str().unwrap_or("tool"),
                if kind == "tool_execution_start" {
                    "proposed"
                } else {
                    "finished"
                }
            );
        }
        "message_update" => {
            if value["assistantMessageEvent"]["type"] != "text_delta" {
                return None;
            }
            event.kind = EventKind::Progress;
            event.summary = value["assistantMessageEvent"]["delta"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(300)
                .collect();
        }
        "extension_ui_request"
            if value["method"] == "confirm" && value["title"] == "Project Pipeline action" =>
        {
            let input: Value = value["message"]
                .as_str()
                .and_then(|message| serde_json::from_str(message).ok())
                .unwrap_or(Value::Null);
            let tool = input["tool"].as_str().unwrap_or("unknown");
            let details = &input["input"];
            let target = match tool {
                "edit" | "write" | "read" => details["path"]
                    .as_str()
                    .or_else(|| details["file_path"].as_str())
                    .unwrap_or(""),
                "bash" | "powershell" => details["command"].as_str().unwrap_or(""),
                _ => "",
            };
            event.kind = EventKind::PermissionRequest;
            event.permission_id = value["id"].as_str().map(str::to_owned);
            event.permission_class = Some(tool.into());
            event.permission_target = Some(target.into());
            event.permission_digest_material = Some(input.to_string());
            event.summary = format!(
                "Pi requests {tool}: {}",
                target.chars().take(160).collect::<String>()
            );
        }
        "extension_error" => {
            event.kind = EventKind::Error;
            event.summary = "Pi extension error".into();
        }
        "auto_retry_end" if value["success"] == false => event.kind = EventKind::Error,
        "response" if value["success"] == false => event.kind = EventKind::Error,
        "response" => return None,
        _ => return None,
    }
    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn normalizes_tool_gate_before_execution() {
        let request = json!({"type":"extension_ui_request","id":"gate-1","method":"confirm",
            "title":"Project Pipeline action","message":json!({"tool":"powershell","input":{"command":"Get-Content src/lib.rs"}}).to_string()});
        let event = normalize_pi_event(&request, "session", 4).unwrap();
        assert_eq!(event.kind, EventKind::PermissionRequest);
        assert_eq!(event.permission_id.as_deref(), Some("gate-1"));
        assert_eq!(
            event.permission_target.as_deref(),
            Some("Get-Content src/lib.rs")
        );
        assert_eq!(event.external_id.as_deref(), Some("pi:4"));
        assert!(
            normalize_pi_event(
                &json!({"type":"agent_settled","aborted":false}),
                "session",
                5
            )
            .is_some()
        );
    }

    #[test]
    fn no_inference_owned_process_probe() {
        let Some(path) = std::env::var_os("PIPELINE_PI_CLI_JS") else {
            return;
        };
        let checkout = TempDir::new().unwrap();
        let run_id = Uuid::new_v4().to_string();
        let adapter = PiAdapter::launch(
            &PathBuf::from(path),
            checkout.path(),
            &run_id,
            "openrouter",
            "cohere/north-mini-code:free",
            &[],
        )
        .unwrap();
        let handle = adapter
            .start(RunRequest {
                run_id,
                checkout: checkout.path().to_path_buf(),
            })
            .unwrap();
        assert_eq!(adapter.probe().health, Health::Ready);
        assert_eq!(adapter.recover(&handle).unwrap().state, RecoveryState::Idle);
        let prompt_adapter = adapter.clone();
        let prompt = std::thread::spawn(move || {
            prompt_adapter.command("prompt", json!({"message":"/pipeline-policy-probe"}))
        });
        let gate = adapter.next_event(Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(gate.kind, EventKind::PermissionRequest);
        adapter
            .reply_permission(&handle, gate.permission_id.as_deref().unwrap(), false)
            .unwrap();
        assert_eq!(
            prompt.join().unwrap().unwrap()["data"]["disposition"],
            "handled"
        );
        adapter.stop(&handle).unwrap();
        drop(adapter);
        let resumed = PiAdapter::launch(
            &PathBuf::from(std::env::var_os("PIPELINE_PI_CLI_JS").unwrap()),
            checkout.path(),
            &handle.run_id,
            "openrouter",
            "cohere/north-mini-code:free",
            &[],
        )
        .unwrap();
        assert_eq!(resumed.recover(&handle).unwrap().state, RecoveryState::Idle);
    }
}
