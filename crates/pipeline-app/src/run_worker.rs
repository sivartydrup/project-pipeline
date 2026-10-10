use pipeline_adapters::{AgentAdapter, EventKind, NormalizedEvent, OpenCodeServer};
use pipeline_engine::{OpenCodePermissionOutcome, ProjectEngine, RunLimits};
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub enum RunCommand {
    Stop,
    ContinuePermission(String),
}

pub enum RunUpdate {
    Started(String),
    Event(String),
    Permission(String, String),
    PermissionResolved(String),
    Finished(String),
    Failed(String),
}

pub struct RunControl {
    pub commands: Sender<RunCommand>,
    pub updates: Receiver<RunUpdate>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for RunControl {
    fn drop(&mut self) {
        let _ = self.commands.send(RunCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct RunLaunch {
    pub database: PathBuf,
    pub evidence_dir: PathBuf,
    pub project_id: String,
    pub task_id: String,
    pub opencode: PathBuf,
    pub cli: PathBuf,
    pub provider: String,
    pub model: String,
    pub token_budget: i64,
}

struct BridgeProcess(Child);

impl Drop for BridgeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn launch(input: RunLaunch) -> RunControl {
    let (command_tx, command_rx) = mpsc::channel();
    let (update_tx, update_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        if let Err(error) = execute(input, &command_rx, &update_tx) {
            let _ = update_tx.send(RunUpdate::Failed(error));
        }
    });
    RunControl {
        commands: command_tx,
        updates: update_rx,
        worker: Some(worker),
    }
}

fn bridge(cli: &Path, database: &Path) -> Result<(BridgeProcess, SocketAddr), String> {
    let reservation = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let address = reservation.local_addr().map_err(|e| e.to_string())?;
    drop(reservation);
    let mut command = Command::new(cli);
    command
        .arg("serve")
        .arg(database)
        .arg(address.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let child = command.spawn().map_err(|e| e.to_string())?;
    let process = BridgeProcess(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_ok() {
            return Ok((process, address));
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("agent bridge did not start".into())
}

fn execute(
    input: RunLaunch,
    commands: &Receiver<RunCommand>,
    updates: &Sender<RunUpdate>,
) -> Result<(), String> {
    if input.provider.trim().is_empty() || input.model.trim().is_empty() {
        return Err("select an explicit provider and model".into());
    }
    if !(1..=160_000).contains(&input.token_budget) {
        return Err("observed token limit must be between 1 and 160,000".into());
    }
    if !input.opencode.is_file() || !input.cli.is_file() {
        return Err("OpenCode and pipeline-cli executable paths must exist".into());
    }
    let mut engine = ProjectEngine::open(&input.database).map_err(|e| e.to_string())?;
    let operations = [
        "project.get",
        "project.context",
        "task.list",
        "task.get",
        "task.update",
        "task.block",
        "task.submit",
        "decision.propose",
        "decision.get",
        "question.ask",
        "artifact.attach",
        "research.record",
        "test.record",
        "run.progress",
    ]
    .map(str::to_owned);
    let start = engine
        .prepare_agent_run(
            &input.project_id,
            &input.task_id,
            true,
            &operations,
            RunLimits {
                wall_seconds: 900,
                token_budget: input.token_budget,
                token_ttl_seconds: 900,
            },
        )
        .map_err(|e| e.to_string())?;
    let run_id = start.run.id.clone();
    engine
        .approve_agent_prompt(&run_id, &input.provider, &input.model)
        .map_err(|e| e.to_string())?;
    let _ = updates.send(RunUpdate::Started(run_id.clone()));
    let outcome = run_prepared(&mut engine, &input, &start, commands, updates);
    if outcome.is_err()
        && let Ok(run) = engine.list_task_run_reviews(&input.project_id)
        && let Some(run) = run.iter().find(|run| run.run_id == run_id)
        && matches!(
            run.state.as_str(),
            "starting" | "running" | "waiting_for_input" | "review"
        )
    {
        // Keep the worktree and audit trail for owner recovery.
        let current = engine.get_agent_run(&run_id).map_err(|e| e.to_string())?;
        let _ = engine.transition_agent_run(&run_id, current.revision, "failed");
    }
    outcome
}

fn run_prepared(
    engine: &mut ProjectEngine,
    input: &RunLaunch,
    start: &pipeline_engine::RunStart,
    commands: &Receiver<RunCommand>,
    updates: &Sender<RunUpdate>,
) -> Result<(), String> {
    let (_bridge, address) = bridge(&input.cli, &input.database)?;
    let permissions = serde_json::json!({"*":"deny","read":"allow","glob":"allow",
        "grep":"allow","edit":"allow","bash":"ask"});
    let permission_config = serde_json::json!({
        "permission": permissions,
        "agent": {"build": {"permission": permissions}},
        "autoupdate": false
    })
    .to_string();
    let server = OpenCodeServer::launch_with_environment(
        &input.opencode,
        Path::new(&start.packet.checkout_path),
        &[
            ("PIPELINE_AGENT_TOKEN", start.grant.token.as_str()),
            ("PIPELINE_AGENT_BRIDGE", &address.to_string()),
            ("OPENCODE_CONFIG_CONTENT", &permission_config),
        ],
    )
    .map_err(|e| e.to_string())?;
    let adapter = server.adapter().map_err(|e| e.to_string())?;
    adapter
        .assert_runtime_permissions(Path::new(&start.packet.checkout_path))
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let bridge_command = format!(
        "& '{}' call {}",
        input.cli.display().to_string().replace('\'', "''"),
        address
    );
    #[cfg(not(windows))]
    let bridge_command = format!("\"{}\" call {}", input.cli.display(), address);
    let handle = engine
        .create_opencode_run_session(&adapter, start)
        .map_err(|e| e.to_string())?;
    let (event_tx, event_rx) = mpsc::channel::<Result<NormalizedEvent, String>>();
    let stream_adapter = server.adapter().map_err(|e| e.to_string())?;
    let stream_handle = handle.clone();
    thread::spawn(move || {
        let result = stream_adapter.events(&stream_handle, &mut |event| {
            event_tx.send(Ok(event)).is_ok()
        });
        if let Err(error) = result {
            let _ = event_tx.send(Err(error.to_string()));
        }
    });
    let connected = event_rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "OpenCode event stream did not connect".to_owned())?
        .map_err(|error| format!("OpenCode event stream: {error}"))?;
    if connected.kind != EventKind::Connected {
        return Err("OpenCode event stream did not confirm connection".into());
    }
    engine
        .observe_opencode_event(&adapter, &handle, connected)
        .map_err(|e| e.to_string())?;
    engine
        .steer_opencode_run(
            &adapter,
            start,
            &handle,
            Some(&bridge_command),
            Some((&input.provider, &input.model)),
        )
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(900);
    let mut last_usage_check = Instant::now();
    let mut known_permissions: HashMap<String, String> = HashMap::new();
    let mut last_permission_check = Instant::now();
    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                RunCommand::Stop => {
                    engine
                        .stop_opencode_run(&adapter, &handle)
                        .map_err(|e| e.to_string())?;
                    let _ =
                        updates.send(RunUpdate::Finished("Run stopped; checkout retained".into()));
                    return Ok(());
                }
                RunCommand::ContinuePermission(id) => {
                    let outcome = engine
                        .resolve_pending_opencode_permission(&adapter, &handle, &id)
                        .map_err(|e| e.to_string())?;
                    if matches!(outcome, OpenCodePermissionOutcome::Pending(_)) {
                        let _ = updates.send(RunUpdate::Permission(
                            id.clone(),
                            "Owner decision still pending".into(),
                        ));
                    } else {
                        known_permissions.remove(&id);
                        let _ = updates.send(RunUpdate::PermissionResolved(id.clone()));
                    }
                    let _ = updates.send(RunUpdate::Event(format!("Permission {id}: {outcome:?}")));
                }
            }
        }
        match event_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(mut event)) => {
                if event.kind == EventKind::PermissionRequest {
                    for _ in 0..3 {
                        let prior = event.permission_target.clone();
                        let _ = adapter.enrich_bash_permission_target(&handle, &mut event);
                        if event.permission_target != prior {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                let observation = engine
                    .observe_opencode_event(&adapter, &handle, event)
                    .map_err(|e| e.to_string())?;
                if matches!(
                    observation.permission,
                    Some(OpenCodePermissionOutcome::Pending(_))
                ) && let Some(id) = observation.event.permission_id.as_ref()
                {
                    if let Some(OpenCodePermissionOutcome::Pending(request_id)) =
                        &observation.permission
                    {
                        known_permissions.insert(id.clone(), request_id.clone());
                    }
                    let _ = updates.send(RunUpdate::Permission(
                        id.clone(),
                        observation.event.summary.clone(),
                    ));
                }
                let _ = updates.send(RunUpdate::Event(format!(
                    "{:?}: {}",
                    observation.event.kind, observation.event.summary
                )));
            }
            Ok(Err(error)) => return Err(format!("OpenCode event stream: {error}")),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("OpenCode event stream ended".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if last_permission_check.elapsed() >= Duration::from_millis(500) {
            last_permission_check = Instant::now();
            for (id, request_id) in known_permissions.clone() {
                let request = engine
                    .get_action_request(&request_id)
                    .map_err(|e| e.to_string())?;
                if request.status == "pending" {
                    continue;
                }
                match engine.resolve_pending_opencode_permission(&adapter, &handle, &id) {
                    Ok(OpenCodePermissionOutcome::Pending(new_request_id)) => {
                        known_permissions.insert(id.clone(), new_request_id);
                        let _ = updates.send(RunUpdate::Permission(
                            id.clone(),
                            "Additional exact action review required".into(),
                        ));
                    }
                    Ok(outcome) => {
                        known_permissions.remove(&id);
                        let _ = updates.send(RunUpdate::PermissionResolved(id.clone()));
                        let _ =
                            updates.send(RunUpdate::Event(format!("Permission {id}: {outcome:?}")));
                    }
                    Err(error) => {
                        known_permissions.remove(&id);
                        let _ = updates.send(RunUpdate::PermissionResolved(id.clone()));
                        let _ = updates.send(RunUpdate::Event(format!(
                            "Permission {id} needs review: {error}"
                        )));
                    }
                }
            }
        }
        let plan = engine
            .load_plan(&input.project_id)
            .map_err(|e| e.to_string())?;
        if plan
            .active_tasks
            .iter()
            .any(|task| task.logical_id == input.task_id && task.status == "review")
        {
            adapter.stop(&handle).map_err(|e| e.to_string())?;
            match engine.capture_run_diff(&handle.run_id, &input.evidence_dir) {
                Ok(id) => {
                    let _ = updates.send(RunUpdate::Event(format!("Captured diff artifact {id}")));
                }
                Err(error) => {
                    let _ = updates.send(RunUpdate::Event(format!(
                        "Diff capture needs review: {error}"
                    )));
                }
            }
            let current = engine
                .get_agent_run(&handle.run_id)
                .map_err(|e| e.to_string())?;
            let reviewed = engine
                .transition_agent_run(&handle.run_id, current.revision, "review")
                .map_err(|e| e.to_string())?;
            engine
                .transition_agent_run(&handle.run_id, reviewed.revision, "completed")
                .map_err(|e| e.to_string())?;
            let _ = updates.send(RunUpdate::Finished(
                "Agent submitted task for owner review".into(),
            ));
            return Ok(());
        }
        if Instant::now() >= deadline {
            engine
                .stop_opencode_run(&adapter, &handle)
                .map_err(|e| e.to_string())?;
            return Err("run reached its 15 minute wall limit; checkout retained".into());
        }
        if last_usage_check.elapsed() >= Duration::from_secs(2) {
            last_usage_check = Instant::now();
            let usage = adapter
                .observed_token_usage(&handle)
                .map_err(|e| e.to_string())?;
            if usage >= start.packet.limits.token_budget as u64 {
                engine
                    .stop_opencode_run(&adapter, &handle)
                    .map_err(|e| e.to_string())?;
                return Err(format!(
                    "run reached observed token budget ({usage}); checkout retained"
                ));
            }
        }
    }
}
