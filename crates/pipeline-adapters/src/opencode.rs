use crate::{
    AdapterError, AgentAdapter, CapabilityReport, EventKind, Health, NormalizedEvent,
    RecoveryReport, RecoveryState, Result, RunHandle, RunRequest,
};
use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use url::Url;
use uuid::Uuid;

const TESTED_VERSION: &str = "1.18.30";

/// A server owned by the application. Dropping it stops the helper process.
pub struct OpenCodeServer {
    child: Child,
    endpoint: String,
    password: String,
}

impl OpenCodeServer {
    pub fn launch(executable: &Path, checkout: &Path) -> Result<Self> {
        #[cfg(windows)]
        if executable.extension().and_then(|value| value.to_str()) != Some("exe") {
            return Err(AdapterError::Protocol(
                "Windows OpenCode server requires the native .exe, not a shell shim".into(),
            ));
        }
        let checkout = std::fs::canonicalize(checkout)?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let password = Uuid::new_v4().simple().to_string();
        let mut command = Command::new(executable);
        command
            .args([
                "serve",
                "--hostname",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ])
            .current_dir(checkout)
            .env("OPENCODE_SERVER_PASSWORD", &password)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let child = command.spawn()?;
        let server = Self {
            child,
            endpoint: format!("http://127.0.0.1:{port}/"),
            password,
        };
        let adapter = server.adapter()?;
        let until = Instant::now() + Duration::from_secs(15);
        while Instant::now() < until {
            if adapter.probe().health == Health::Ready {
                return Ok(server);
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Err(AdapterError::Unavailable(
            "OpenCode did not become ready within 15 seconds".into(),
        ))
    }

    pub fn adapter(&self) -> Result<OpenCodeAdapter> {
        OpenCodeAdapter::connect(&self.endpoint, Some(self.password.clone()), TESTED_VERSION)
    }
}

impl Drop for OpenCodeServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct OpenCodeAdapter {
    base: Url,
    password: Option<String>,
    expected_version: String,
    http: ureq::Agent,
}

impl OpenCodeAdapter {
    pub fn connect(
        endpoint: &str,
        password: Option<String>,
        expected_version: &str,
    ) -> Result<Self> {
        let base = Url::parse(endpoint).map_err(|e| AdapterError::Protocol(e.to_string()))?;
        let local = match base.host_str().and_then(|host| host.parse::<IpAddr>().ok()) {
            Some(ip) => ip.is_loopback(),
            None => base.host_str() == Some("localhost"),
        };
        if base.scheme() != "http" || !local || base.port().is_none() {
            return Err(AdapterError::Protocol(
                "OpenCode endpoint must be explicit loopback HTTP".into(),
            ));
        }
        let http = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .timeout_read(Duration::from_secs(10))
            .timeout_write(Duration::from_secs(10))
            .build();
        Ok(Self {
            base,
            password,
            expected_version: expected_version.into(),
            http,
        })
    }

    fn url(&self, path: &str, directory: Option<&Path>) -> Result<String> {
        let mut url = self
            .base
            .join(path)
            .map_err(|e| AdapterError::Protocol(e.to_string()))?;
        if let Some(directory) = directory {
            url.query_pairs_mut()
                .append_pair("directory", &directory.to_string_lossy());
        }
        Ok(url.to_string())
    }

    fn request(&self, method: &str, path: &str, directory: Option<&Path>) -> Result<ureq::Request> {
        let url = self.url(path, directory)?;
        let mut request = self.http.request(method, &url);
        if let Some(password) = &self.password {
            let value =
                base64::engine::general_purpose::STANDARD.encode(format!("opencode:{password}"));
            request = request.set("Authorization", &format!("Basic {value}"));
        }
        Ok(request)
    }

    fn get_json(&self, path: &str, directory: Option<&Path>) -> Result<Value> {
        self.request("GET", path, directory)?
            .call()
            .map_err(map_http)?
            .into_json()
            .map_err(|e| AdapterError::Protocol(e.to_string()))
    }

    fn post_json(&self, path: &str, directory: &Path, body: Value) -> Result<Value> {
        let response = self
            .request("POST", path, Some(directory))?
            .send_json(body)
            .map_err(map_http)?;
        if response.status() == 204 {
            return Ok(Value::Null);
        }
        response
            .into_json()
            .map_err(|e| AdapterError::Protocol(e.to_string()))
    }

    fn require_ready(&self) -> Result<()> {
        let report = self.probe();
        match report.health {
            Health::Ready => Ok(()),
            Health::VersionMismatch => Err(AdapterError::VersionMismatch {
                expected: self.expected_version.clone(),
                found: report.version.unwrap_or_default(),
            }),
            Health::Unavailable => Err(AdapterError::Unavailable(report.detail)),
        }
    }

    pub fn stream_events(
        &self,
        run: &RunHandle,
        mut on_event: impl FnMut(NormalizedEvent) -> bool,
    ) -> Result<()> {
        self.require_ready()?;
        let response = self
            .request("GET", "global/event", None)?
            .set("Accept", "text/event-stream")
            .call()
            .map_err(map_http)?;
        let mut reader = BufReader::new(response.into_reader());
        let mut decoder = SseDecoder::default();
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => return Err(AdapterError::Unavailable("event stream closed".into())),
                Ok(_) => {
                    if line.len() > 64 * 1024 {
                        return Err(AdapterError::Protocol("SSE line too large".into()));
                    }
                    if let Some(event) = decoder.push_line(&line)?
                        && let Some(event) = normalize_event(&event, &run.session_id)
                        && !on_event(event)
                    {
                        return Ok(());
                    }
                }
                Err(error) => return Err(AdapterError::Unavailable(error.to_string())),
            }
        }
    }

    pub fn normalize_event(value: &Value, session_id: &str) -> Option<NormalizedEvent> {
        normalize_event(value, session_id)
    }

    pub fn create_session(&self, run_id: &str, checkout: &Path) -> Result<RunHandle> {
        self.require_ready()?;
        let checkout = std::fs::canonicalize(checkout)?;
        let session = self.post_json(
            "session",
            &checkout,
            json!({"title":format!("Project Pipeline run {run_id}")}),
        )?;
        let session_id = session["id"]
            .as_str()
            .filter(|id| id.starts_with("ses"))
            .ok_or_else(|| AdapterError::Protocol("session creation returned no ID".into()))?
            .to_owned();
        Ok(RunHandle {
            run_id: run_id.into(),
            session_id,
            checkout,
        })
    }

    pub fn pending_permissions(&self, run: &RunHandle) -> Result<Vec<NormalizedEvent>> {
        let pending = self.get_json("permission", Some(&run.checkout))?;
        let items = pending
            .as_array()
            .ok_or_else(|| AdapterError::Protocol("permissions response is not an array".into()))?;
        let mut events: Vec<_> = items
            .iter()
            .filter_map(|item| {
                normalize_event(
                    &json!({"type":"permission.asked","properties":item}),
                    &run.session_id,
                )
            })
            .collect();
        let newer = self.get_json(
            &format!("api/session/{}/permission", run.session_id),
            Some(&run.checkout),
        )?;
        let newer = newer["data"].as_array().ok_or_else(|| {
            AdapterError::Protocol("v2 permissions response is not an array".into())
        })?;
        events.extend(newer.iter().filter_map(|item| {
            normalize_event(
                &json!({"type":"permission.v2.asked","properties":item}),
                &run.session_id,
            )
        }));
        Ok(events)
    }

    pub fn delete_session(&self, run: &RunHandle) -> Result<()> {
        self.request(
            "DELETE",
            &format!("session/{}", run.session_id),
            Some(&run.checkout),
        )?
        .call()
        .map_err(map_http)?;
        Ok(())
    }
}

impl AgentAdapter for OpenCodeAdapter {
    fn probe(&self) -> CapabilityReport {
        let result = self.get_json("global/health", None);
        let (health, version, detail) = match result {
            Ok(value) => {
                let version = value["version"].as_str().map(str::to_owned);
                if value["healthy"] != true {
                    (
                        Health::Unavailable,
                        version,
                        "health endpoint reported unhealthy".into(),
                    )
                } else if version.as_deref() != Some(&self.expected_version) {
                    (
                        Health::VersionMismatch,
                        version,
                        "server version differs from tested protocol".into(),
                    )
                } else {
                    (
                        Health::Ready,
                        version,
                        "tested OpenCode protocol available".into(),
                    )
                }
            }
            Err(error) => (Health::Unavailable, None, error.to_string()),
        };
        let ready = health == Health::Ready;
        CapabilityReport {
            health,
            version,
            detail,
            can_resume: ready,
            can_steer: ready,
            can_stop: ready,
            can_stream: ready,
            can_diff: ready,
            can_reply_permissions: ready,
        }
    }

    fn start(&self, request: RunRequest) -> Result<RunHandle> {
        self.create_session(&request.run_id, &request.checkout)
    }

    fn events(
        &self,
        run: &RunHandle,
        on_event: &mut dyn FnMut(NormalizedEvent) -> bool,
    ) -> Result<()> {
        self.stream_events(run, on_event)
    }

    fn steer(&self, run: &RunHandle, message: &str) -> Result<()> {
        self.require_ready()?;
        if message.trim().is_empty() {
            return Err(AdapterError::Protocol("empty prompt".into()));
        }
        self.post_json(
            &format!("session/{}/prompt_async", run.session_id),
            &run.checkout,
            json!({"parts":[{"type":"text","text":message}]}),
        )?;
        Ok(())
    }

    fn stop(&self, run: &RunHandle) -> Result<()> {
        self.post_json(
            &format!("session/{}/abort", run.session_id),
            &run.checkout,
            json!({}),
        )?;
        Ok(())
    }

    fn recover(&self, run: &RunHandle) -> Result<RecoveryReport> {
        if self.probe().health == Health::Unavailable {
            return Ok(RecoveryReport {
                session_id: run.session_id.clone(),
                state: RecoveryState::Disconnected,
                diff_files: 0,
            });
        }
        self.require_ready()?;
        let session =
            match self.get_json(&format!("session/{}", run.session_id), Some(&run.checkout)) {
                Ok(session) => session,
                Err(AdapterError::Transport(status)) if status.starts_with("404") => {
                    return Ok(RecoveryReport {
                        session_id: run.session_id.clone(),
                        state: RecoveryState::Missing,
                        diff_files: 0,
                    });
                }
                Err(error) => return Err(error),
            };
        if session["id"] != run.session_id {
            return Err(AdapterError::Protocol(
                "recovered session ID mismatch".into(),
            ));
        }
        let status = self.get_json("session/status", Some(&run.checkout))?;
        let busy = status[&run.session_id]["type"] == "busy";
        let diff = self.get_json(
            &format!("session/{}/diff", run.session_id),
            Some(&run.checkout),
        )?;
        let diff_files = diff
            .as_array()
            .ok_or_else(|| AdapterError::Protocol("session diff is not an array".into()))?
            .len();
        Ok(RecoveryReport {
            session_id: run.session_id.clone(),
            state: if busy {
                RecoveryState::Busy
            } else {
                RecoveryState::Idle
            },
            diff_files,
        })
    }

    fn reply_permission(
        &self,
        run: &RunHandle,
        permission_id: &str,
        allow_once: bool,
    ) -> Result<()> {
        self.require_ready()?;
        if !permission_id.starts_with("per") {
            return Err(AdapterError::Protocol("invalid permission ID".into()));
        }
        let pending = self.get_json("permission", Some(&run.checkout))?;
        let belongs = pending.as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["id"] == permission_id && item["sessionID"] == run.session_id)
        });
        if !belongs {
            let newer = self.get_json(
                &format!("api/session/{}/permission", run.session_id),
                Some(&run.checkout),
            )?;
            let newer_belongs = newer["data"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["id"] == permission_id));
            if newer_belongs {
                self.post_json(
                    &format!(
                        "api/session/{}/permission/{permission_id}/reply",
                        run.session_id
                    ),
                    &run.checkout,
                    json!({"reply":if allow_once {"once"} else {"reject"}}),
                )?;
                return Ok(());
            }
            return Err(AdapterError::Protocol(
                "permission is not pending for this session".into(),
            ));
        }
        self.post_json(
            &format!("permission/{permission_id}/reply"),
            &run.checkout,
            json!({"reply":if allow_once {"once"} else {"reject"}}),
        )?;
        Ok(())
    }
}

fn map_http(error: ureq::Error) -> AdapterError {
    match error {
        ureq::Error::Status(status, _) => AdapterError::Transport(format!("{status} response")),
        ureq::Error::Transport(error) => AdapterError::Unavailable(error.to_string()),
    }
}

#[derive(Default)]
pub struct SseDecoder {
    data: String,
}

impl SseDecoder {
    pub fn push_line(&mut self, line: &str) -> Result<Option<Value>> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if self.data.is_empty() {
                return Ok(None);
            }
            let data = std::mem::take(&mut self.data);
            return serde_json::from_str(data.trim_end_matches('\n'))
                .map(Some)
                .map_err(|e| AdapterError::Protocol(format!("invalid SSE JSON: {e}")));
        }
        if let Some(data) = line.strip_prefix("data:") {
            if self.data.len() + data.len() > 1024 * 1024 {
                return Err(AdapterError::Protocol("SSE event too large".into()));
            }
            self.data.push_str(data.trim_start());
            self.data.push('\n');
        }
        Ok(None)
    }
}

fn normalize_event(value: &Value, session_id: &str) -> Option<NormalizedEvent> {
    let payload = value.get("payload").unwrap_or(value);
    let kind = payload["type"].as_str()?;
    let properties = &payload["properties"];
    let event_session = properties["sessionID"]
        .as_str()
        .or_else(|| properties["info"]["sessionID"].as_str());
    if event_session != Some(session_id) && kind != "server.connected" {
        return None;
    }
    let external_id = payload["id"]
        .as_str()
        .or_else(|| value["id"].as_str())
        .map(str::to_owned);
    let mut event = NormalizedEvent {
        external_id,
        session_id: session_id.into(),
        kind: EventKind::Unknown,
        summary: kind.chars().take(100).collect(),
        permission_id: None,
        permission_class: None,
        permission_target: None,
        permission_digest_material: None,
    };
    match kind {
        "server.connected" => event.kind = EventKind::Connected,
        "session.status" | "message.updated" | "message.part.updated" => {
            event.kind = EventKind::Progress;
        }
        "session.idle" => event.kind = EventKind::Idle,
        "session.error" => event.kind = EventKind::Error,
        "session.diff" | "file.edited" => event.kind = EventKind::Diff,
        "permission.asked" | "permission.updated" | "permission.v2.asked" => {
            event.kind = EventKind::PermissionRequest;
            let request = if properties.get("request").is_some() {
                &properties["request"]
            } else {
                properties
            };
            event.permission_id = request["id"].as_str().map(str::to_owned);
            event.permission_class = Some("unknown_external".into());
            let resources = if kind == "permission.v2.asked" {
                &request["resources"]
            } else {
                &request["patterns"]
            };
            let target = resources
                .as_array()
                .map(|patterns| {
                    patterns
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|target| !target.is_empty())
                .unwrap_or_else(|| {
                    request["permission"]
                        .as_str()
                        .or_else(|| request["action"].as_str())
                        .unwrap_or("unknown")
                        .into()
                });
            event.permission_target = Some(format!(
                "{}: {target}",
                request["id"].as_str().unwrap_or("unknown")
            ));
            event.permission_digest_material = Some(format!(
                "{:x}",
                Sha256::digest(request.to_string().as_bytes())
            ));
            event.summary = format!(
                "OpenCode requests {} permission",
                request["permission"]
                    .as_str()
                    .or_else(|| request["action"].as_str())
                    .unwrap_or("unknown")
            );
        }
        "permission.replied" => event.kind = EventKind::Progress,
        name if name.starts_with("tool.") => event.kind = EventKind::Tool,
        _ => {}
    }
    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::{Arc, Mutex};

    #[test]
    fn sse_frames_and_permission_versions_normalize_without_approval() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push_line(": heartbeat\n").unwrap().is_none());
        let payload = json!({"payload":{"id":"evt_1","type":"permission.v2.asked",
            "properties":{"id":"per_1","sessionID":"ses_1","action":"bash",
            "resources":["git push"],"metadata":{"secret":"must not persist"}}}});
        let frame = format!("data: {}\n", payload);
        assert!(decoder.push_line(&frame).unwrap().is_none());
        let parsed = decoder.push_line("\n").unwrap().unwrap();
        let event = OpenCodeAdapter::normalize_event(&parsed, "ses_1").unwrap();
        assert_eq!(event.kind, EventKind::PermissionRequest);
        assert_eq!(event.external_id.as_deref(), Some("evt_1"));
        assert_eq!(event.permission_id.as_deref(), Some("per_1"));
        assert_eq!(event.permission_target.as_deref(), Some("per_1: git push"));
        assert!(!event.summary.contains("secret"));
        assert!(
            !event
                .permission_digest_material
                .as_deref()
                .unwrap()
                .contains("secret")
        );
        assert!(OpenCodeAdapter::normalize_event(&parsed, "ses_other").is_none());
        let old = json!({"type":"permission.asked","properties":{"id":"per_2",
            "sessionID":"ses_1","permission":"edit","patterns":["src/main.rs"]}});
        assert_eq!(
            OpenCodeAdapter::normalize_event(&old, "ses_1")
                .unwrap()
                .kind,
            EventKind::PermissionRequest
        );
    }

    #[test]
    fn loopback_only_and_outage_are_explicit() {
        assert!(
            OpenCodeAdapter::connect("https://example.com:443/", None, TESTED_VERSION).is_err()
        );
        let adapter =
            OpenCodeAdapter::connect("http://127.0.0.1:1/", None, TESTED_VERSION).unwrap();
        assert_eq!(adapter.probe().health, Health::Unavailable);
    }

    #[test]
    fn protocol_fixture_covers_probe_start_steer_stop_recovery_and_permissions() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_worker = Arc::clone(&seen);
        let worker = std::thread::spawn(move || {
            for _ in 0..32 {
                let Some(mut request) = server.recv_timeout(Duration::from_secs(5)).unwrap() else {
                    break;
                };
                let path = request.url().split('?').next().unwrap().to_owned();
                let mut body = String::new();
                let length = request.body_length().unwrap_or(0);
                request
                    .as_reader()
                    .take(length as u64)
                    .read_to_string(&mut body)
                    .unwrap();
                seen_worker.lock().unwrap().push((path.clone(), body));
                let (status, response) = match path.as_str() {
                    "/global/health" => (200, json!({"healthy":true,"version":TESTED_VERSION})),
                    "/session" => (200, json!({"id":"ses_fixture"})),
                    "/session/ses_fixture/prompt_async" => (204, Value::Null),
                    "/session/ses_fixture" => (200, json!({"id":"ses_fixture"})),
                    "/session/status" => (200, json!({"ses_fixture":{"type":"busy"}})),
                    "/session/ses_fixture/diff" => (200, json!([{"file":"a.rs"}])),
                    "/permission" => (
                        200,
                        json!([{"id":"per_fixture","sessionID":"ses_fixture",
                        "permission":"bash","patterns":["git push"]}]),
                    ),
                    "/api/session/ses_fixture/permission" => (200, json!({"data":[]})),
                    "/permission/per_fixture/reply" => (200, json!(true)),
                    "/session/ses_fixture/abort" => (200, json!(true)),
                    _ => (404, json!({"error":"unknown route"})),
                };
                let reply = tiny_http::Response::from_string(response.to_string())
                    .with_status_code(status)
                    .with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    );
                request.respond(reply).unwrap();
                if path == "/session/ses_fixture/abort" {
                    break;
                }
            }
        });
        let endpoint = format!("http://{address}/");
        let adapter = OpenCodeAdapter::connect(&endpoint, None, TESTED_VERSION).unwrap();
        assert_eq!(adapter.probe().health, Health::Ready);
        let mismatched = OpenCodeAdapter::connect(&endpoint, None, "0.0.0").unwrap();
        assert_eq!(mismatched.probe().health, Health::VersionMismatch);
        let checkout = tempfile::tempdir().unwrap();
        let handle = adapter
            .start(RunRequest {
                run_id: "run_fixture".into(),
                checkout: checkout.path().to_path_buf(),
            })
            .unwrap();
        adapter.steer(&handle, "fixture prompt").unwrap();
        adapter.steer(&handle, "fixture steering").unwrap();
        let recovery = adapter.recover(&handle).unwrap();
        assert_eq!(recovery.state, RecoveryState::Busy);
        assert_eq!(recovery.diff_files, 1);
        let permissions = adapter.pending_permissions(&handle).unwrap();
        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].kind, EventKind::PermissionRequest);
        adapter
            .reply_permission(&handle, "per_fixture", false)
            .unwrap();
        adapter.stop(&handle).unwrap();
        worker.join().unwrap();
        let routes = seen.lock().unwrap();
        assert!(
            routes
                .iter()
                .any(|(path, body)| path.ends_with("prompt_async")
                    && body.contains("fixture prompt"))
        );
        assert!(
            routes
                .iter()
                .any(|(path, body)| path.ends_with("reply") && body.contains("reject"))
        );
        assert!(!routes.iter().any(|(_, body)| body.contains("always")));
    }

    #[test]
    #[ignore = "requires a local OpenCode server; does not start model inference"]
    fn live_windows_session_smoke_without_model_inference() {
        let endpoint = std::env::var("OPENCODE_TEST_ENDPOINT").unwrap();
        let adapter = OpenCodeAdapter::connect(&endpoint, None, TESTED_VERSION).unwrap();
        let report = adapter.probe();
        assert_eq!(report.health, Health::Ready);
        let checkout = tempfile::tempdir().unwrap();
        let run = adapter
            .create_session("run_live_smoke", checkout.path())
            .unwrap();
        let recovery = adapter.recover(&run).unwrap();
        assert_eq!(recovery.state, RecoveryState::Idle);
        adapter.stop(&run).unwrap();
        let stream_adapter = OpenCodeAdapter::connect(&endpoint, None, TESTED_VERSION).unwrap();
        let stream_run = run.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let stream = std::thread::spawn(move || {
            let mut saw_delete = false;
            stream_adapter
                .stream_events(&stream_run, |event| {
                    if event.kind == EventKind::Connected {
                        sender.send(()).unwrap();
                    }
                    if event.summary == "session.deleted" {
                        saw_delete = true;
                        return false;
                    }
                    true
                })
                .unwrap();
            saw_delete
        });
        receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        adapter.delete_session(&run).unwrap();
        assert!(stream.join().unwrap());
        eprintln!(
            "CAPABILITY_REPORT={} RUN_LOG={}",
            serde_json::to_string(&report).unwrap(),
            json!({"session_created":true,"recovery":"idle","diff_files":recovery.diff_files,
                "aborted":true,"deleted":true,"sse_deleted_event":true,"model_inference":false})
        );
    }

    #[test]
    #[ignore = "requires installed OpenCode executable; does not start model inference"]
    fn live_owned_server_uses_password_and_stops_on_drop() {
        let executable = std::env::var("OPENCODE_TEST_EXECUTABLE").unwrap();
        let checkout = tempfile::tempdir().unwrap();
        let server = OpenCodeServer::launch(Path::new(&executable), checkout.path()).unwrap();
        let adapter = server.adapter().unwrap();
        assert_eq!(adapter.probe().health, Health::Ready);
        let unauthenticated =
            OpenCodeAdapter::connect(&server.endpoint, None, TESTED_VERSION).unwrap();
        assert_eq!(unauthenticated.probe().health, Health::Unavailable);
        let authenticated = server.adapter().unwrap();
        drop(server);
        assert_eq!(authenticated.probe().health, Health::Unavailable);
    }
}
