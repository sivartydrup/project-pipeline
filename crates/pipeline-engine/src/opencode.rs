use crate::{ActionClass, ActionSpec, EngineError, PolicyOutcome, ProjectEngine, Result, RunStart};
use pipeline_adapters::{
    AgentAdapter, EventKind, NormalizedEvent, OpenCodeAdapter, RecoveryReport, RunHandle,
    RunRequest,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum OpenCodePermissionOutcome {
    Pending(String),
    Denied,
    AllowedOnce,
}

#[derive(Debug, Clone)]
pub struct OpenCodeObservation {
    pub event: NormalizedEvent,
    pub sequence: Option<i64>,
    pub permission: Option<OpenCodePermissionOutcome>,
}

fn permission_action(event: &NormalizedEvent) -> Result<ActionSpec> {
    if event.kind != EventKind::PermissionRequest {
        return Err(EngineError::RunPreflight("not a permission event".into()));
    }
    let target = event
        .permission_target
        .as_ref()
        .ok_or_else(|| EngineError::RunPreflight("permission target missing".into()))?;
    let command_spec = event
        .permission_digest_material
        .as_ref()
        .ok_or_else(|| EngineError::RunPreflight("permission details missing".into()))?;
    Ok(ActionSpec {
        class: ActionClass::UnknownExternal,
        target: target.clone(),
        command_spec: command_spec.clone(),
        effect_summary: event.summary.clone(),
    })
}

impl ProjectEngine {
    pub fn start_opencode_run(
        &mut self,
        adapter: &OpenCodeAdapter,
        start: &RunStart,
    ) -> Result<RunHandle> {
        if start.run.state != "starting" {
            return Err(EngineError::RunPreflight("run is not starting".into()));
        }
        let handle = adapter.start(RunRequest {
            run_id: start.run.id.clone(),
            checkout: Path::new(&start.packet.checkout_path).to_path_buf(),
        })?;
        if let Err(error) =
            self.store
                .attach_adapter_session(&start.run.id, start.run.revision, &handle.session_id)
        {
            let _ = adapter.stop(&handle);
            let _ = adapter.delete_session(&handle);
            return Err(error.into());
        }
        self.store
            .transition_run(&start.run.id, start.run.revision, "running")?;
        let prompt = serde_json::to_string(&start.packet)
            .map_err(|e| EngineError::RunPreflight(e.to_string()))?;
        let prompt = format!(
            "Execute this approved Project Pipeline task packet. Respect its stop conditions and use the scoped project bridge for durable updates.\n{prompt}"
        );
        if let Err(error) = adapter.steer(&handle, &prompt) {
            let _ = adapter.stop(&handle);
            let run = self.store.get_run(&start.run.id)?;
            self.store.transition_run(&run.id, run.revision, "failed")?;
            return Err(error.into());
        }
        Ok(handle)
    }

    pub fn observe_opencode_event(
        &mut self,
        adapter: &OpenCodeAdapter,
        run: &RunHandle,
        event: NormalizedEvent,
    ) -> Result<OpenCodeObservation> {
        let event_id = event.external_id.clone().unwrap_or_else(|| {
            format!(
                "synthetic:{:x}",
                Sha256::digest(format!("{}:{}", event.session_id, event.summary).as_bytes())
            )
        });
        let kind = format!("{:?}", event.kind).to_lowercase();
        let sequence = self.store.append_adapter_event(
            &run.run_id,
            &run.session_id,
            &event_id,
            &kind,
            &event.summary,
        )?;
        let mut permission = None;
        if sequence.is_some() && event.kind == EventKind::PermissionRequest {
            let action = permission_action(&event)?;
            match self.evaluate_action(&run.run_id, &action)? {
                PolicyOutcome::Pending(request) => {
                    permission = Some(OpenCodePermissionOutcome::Pending(request.id));
                    let current = self.store.get_run(&run.run_id)?;
                    if current.state == "running" {
                        self.store.transition_run(
                            &run.run_id,
                            current.revision,
                            "waiting_for_input",
                        )?;
                    }
                }
                PolicyOutcome::Denied(_) => {
                    if let Some(id) = &event.permission_id {
                        adapter.reply_permission(run, id, false)?;
                    }
                    permission = Some(OpenCodePermissionOutcome::Denied);
                }
                PolicyOutcome::Allowed => {
                    if let Some(id) = &event.permission_id {
                        adapter.reply_permission(run, id, true)?;
                    }
                    permission = Some(OpenCodePermissionOutcome::AllowedOnce);
                }
            }
        }
        Ok(OpenCodeObservation {
            event,
            sequence,
            permission,
        })
    }

    pub fn resolve_pending_opencode_permission(
        &mut self,
        adapter: &OpenCodeAdapter,
        run: &RunHandle,
        permission_id: &str,
    ) -> Result<OpenCodePermissionOutcome> {
        let event = adapter
            .pending_permissions(run)?
            .into_iter()
            .find(|event| event.permission_id.as_deref() == Some(permission_id))
            .ok_or_else(|| EngineError::RunPreflight("permission no longer pending".into()))?;
        match self.evaluate_action(&run.run_id, &permission_action(&event)?)? {
            PolicyOutcome::Pending(request) => Ok(OpenCodePermissionOutcome::Pending(request.id)),
            PolicyOutcome::Denied(_) => {
                adapter.reply_permission(run, permission_id, false)?;
                Ok(OpenCodePermissionOutcome::Denied)
            }
            PolicyOutcome::Allowed => {
                adapter.reply_permission(run, permission_id, true)?;
                let current = self.store.get_run(&run.run_id)?;
                if current.state == "waiting_for_input" {
                    self.store
                        .transition_run(&run.run_id, current.revision, "running")?;
                }
                Ok(OpenCodePermissionOutcome::AllowedOnce)
            }
        }
    }

    pub fn stream_opencode_events(
        &mut self,
        adapter: &OpenCodeAdapter,
        run: &RunHandle,
        max_events: usize,
    ) -> Result<Vec<OpenCodeObservation>> {
        let mut observations = Vec::new();
        let mut failure = None;
        adapter.stream_events(run, |event| {
            match self.observe_opencode_event(adapter, run, event) {
                Ok(observation) => observations.push(observation),
                Err(error) => {
                    failure = Some(error);
                    return false;
                }
            }
            observations.len() < max_events
        })?;
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(observations)
    }

    pub fn stop_opencode_run(&mut self, adapter: &OpenCodeAdapter, run: &RunHandle) -> Result<()> {
        adapter.stop(run)?;
        let current = self.store.get_run(&run.run_id)?;
        self.store
            .transition_run(&run.run_id, current.revision, "cancelled")?;
        Ok(())
    }

    pub fn recover_opencode_run(
        &mut self,
        adapter: &OpenCodeAdapter,
        run_id: &str,
    ) -> Result<RecoveryReport> {
        let run = self.store.get_run(run_id)?;
        let session_id = self
            .store
            .external_session_id(run_id)?
            .ok_or_else(|| EngineError::RunPreflight("run has no OpenCode session".into()))?;
        let handle = RunHandle {
            run_id: run_id.into(),
            session_id: session_id.clone(),
            checkout: PathBuf::from(run.checkout_path),
        };
        let report = adapter.recover(&handle)?;
        let event_id = format!("recovery:{session_id}:{:?}", report.state);
        let _ = self.store.append_adapter_event(
            run_id,
            &session_id,
            &event_id,
            "recovery",
            &format!(
                "OpenCode session {:?}; {} diff files",
                report.state, report.diff_files
            ),
        )?;
        Ok(report)
    }
}
